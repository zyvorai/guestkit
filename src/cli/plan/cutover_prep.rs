// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline cutover prep plans: SELinux relabel, Windows sysprep, BitLocker escrow.
//!
//! None of these decrypt a volume or run sysprep.exe while the guest is
//! offline. They write reviewable FixPlan operations + host-side artifacts.

use super::types::*;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn base_plan(vm: &str, profile: &str, description: &str, tags: &[&str]) -> FixPlan {
    let mut plan = FixPlan::new(vm.to_string(), profile.to_string());
    plan.version = "1".to_string();
    plan.overall_risk = "medium".to_string();
    plan.estimated_duration = "seconds".to_string();
    plan.metadata.author = "guestkit".to_string();
    plan.metadata.review_required = true;
    plan.metadata.reversible = true;
    plan.metadata.description = Some(description.to_string());
    plan.metadata.tags = tags.iter().map(|s| (*s).to_string()).collect();
    plan
}

fn write_op(id: &str, path: &str, content: &str, desc: &str) -> Operation {
    Operation {
        id: id.into(),
        op_type: OperationType::FileWrite(FileWrite {
            path: path.into(),
            content: content.to_string(),
            mode: Some("0644".into()),
        }),
        priority: Priority::High,
        description: desc.into(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![],
        validation: None,
        undo: Some(UndoInfo::Command {
            command: format!("rm -f {path}"),
        }),
    }
}

/// Offline `touch /.autorelabel` so the next boot relabels.
pub fn selinux_relabel_plan(vm: &str) -> FixPlan {
    let mut plan = base_plan(
        vm,
        "selinux-relabel",
        "Schedule SELinux autorelabel on next boot (/.autorelabel)",
        &["linux", "selinux", "offline"],
    );
    plan.overall_risk = "low".to_string();
    plan.metadata.review_required = false;
    plan.add_operation(write_op(
        "autorelabel",
        "/.autorelabel",
        "",
        "Create /.autorelabel (SELinux full relabel on next boot)",
    ));
    plan
}

/// Offline Windows generalize *prep*: drop unattend + a first-boot flag.
/// Does not execute sysprep.exe against the offline hive.
pub fn windows_sysprep_plan(vm: &str, hostname: Option<&str>, run_on_firstboot: bool) -> FixPlan {
    let name = hostname
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or("*");
    let mut plan = base_plan(
        vm,
        "windows-sysprep",
        &format!(
            "Offline sysprep prepare (unattend ComputerName={name}; first-boot generalize={run_on_firstboot})"
        ),
        &["windows", "sysprep", "offline"],
    );
    plan.overall_risk = "high".to_string();

    let unattend = unattend_xml(name);
    let setupcomplete = setupcomplete_cmd();
    plan.add_operation(write_op(
        "unattend",
        "/Windows/System32/Sysprep/unattend.xml",
        &unattend,
        "Write Sysprep unattend.xml (specialize + persist drivers)",
    ));
    plan.add_operation(write_op(
        "setupcomplete",
        "/Windows/Setup/Scripts/SetupComplete.cmd",
        &setupcomplete,
        "Write SetupComplete.cmd — runs sysprep /generalize if flag file exists",
    ));
    if run_on_firstboot {
        plan.add_operation(write_op(
            "sysprep-flag",
            "/GuestKit/run-sysprep.flag",
            "1\n",
            "Arm first-boot sysprep /generalize /oobe /quit",
        ));
    }
    plan.add_operation(write_op(
        "sysprep-readme",
        "/GuestKit/SYSPREP.txt",
        "GuestKit staged an offline sysprep unattend. sysprep.exe itself runs at first boot only if /GuestKit/run-sysprep.flag exists.\n",
        "Operator note inside the guest",
    ));
    plan
}

fn unattend_xml(computer_name: &str) -> String {
    format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
<unattend xmlns="urn:schemas-microsoft-com:unattend">
  <settings pass="generalize">
    <component name="Microsoft-Windows-PnpSysprep" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
      <PersistAllDeviceInstalls>true</PersistAllDeviceInstalls>
    </component>
  </settings>
  <settings pass="specialize">
    <component name="Microsoft-Windows-Shell-Setup" processorArchitecture="amd64" publicKeyToken="31bf3856ad364e35" language="neutral" versionScope="nonSxS">
      <ComputerName>{computer_name}</ComputerName>
      <CopyProfile>false</CopyProfile>
    </component>
  </settings>
</unattend>
"#
    )
}

fn setupcomplete_cmd() -> String {
    r#"@echo off
if exist C:\GuestKit\run-sysprep.flag (
  C:\Windows\System32\Sysprep\sysprep.exe /generalize /oobe /quit /unattend:C:\Windows\System32\Sysprep\unattend.xml
  del /f /q C:\GuestKit\run-sysprep.flag
)
"#
    .to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BitLockerEscrow {
    pub kind: String,
    pub image: String,
    pub key_sha256: String,
    pub key_bytes: usize,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_preview: Option<String>,
    pub guest_marker: String,
    pub note: String,
}

/// Host-side escrow of a recovery password / BEK. Never writes the secret
/// into the guest disk.
pub fn bitlocker_escrow(
    image: &Path,
    key_file: &Path,
    include_secret: bool,
    host_output: &Path,
) -> Result<(BitLockerEscrow, FixPlan)> {
    let key = std::fs::read(key_file)
        .with_context(|| format!("read recovery key {}", key_file.display()))?;
    anyhow::ensure!(!key.is_empty(), "recovery key file is empty");
    let digest = sha256_hex(&key);

    let preview = if include_secret {
        Some(String::from_utf8_lossy(&key).trim().to_string())
    } else {
        None
    };

    let record = BitLockerEscrow {
        kind: "guestkit.bitlocker-escrow".into(),
        image: image.display().to_string(),
        key_sha256: digest.clone(),
        key_bytes: key.len(),
        key_preview: preview,
        guest_marker: "/GuestKit/BITLOCKER-ESCROW.txt".into(),
        note: "Attach this recovery password at the BitLocker prompt on first boot of new hardware. GuestKit does not decrypt the volume offline.".into(),
    };

    if let Some(dir) = host_output.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)?;
        }
    }
    std::fs::write(host_output, serde_json::to_string_pretty(&record)?)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(host_output, std::fs::Permissions::from_mode(0o600));
    }

    let mut plan = base_plan(
        &image.display().to_string(),
        "bitlocker-escrow",
        "BitLocker recovery key escrowed on the host; guest only gets an operator marker",
        &["windows", "bitlocker", "offline"],
    );
    plan.overall_risk = "high".to_string();
    plan.add_operation(write_op(
        "bitlocker-marker",
        "/GuestKit/BITLOCKER-ESCROW.txt",
        &format!(
            "A BitLocker recovery key was escrowed on the migration host.\n\
             Host record: {}\n\
             Key SHA-256: {}\n\
             Do not power this disk on new hardware without that key.\n",
            host_output.display(),
            digest
        ),
        "Write guest-side marker (no secret)",
    ));
    Ok((record, plan))
}

pub fn default_escrow_path(image: &Path) -> PathBuf {
    let mut p = image.to_path_buf();
    p.set_extension("bitlocker-escrow.json");
    p
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selinux_plan_writes_autorelabel() {
        let p = selinux_relabel_plan("disk.qcow2");
        assert_eq!(p.profile, "selinux-relabel");
        match &p.operations[0].op_type {
            OperationType::FileWrite(fw) => assert_eq!(fw.path, "/.autorelabel"),
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn sysprep_plan_arms_flag_when_requested() {
        let p = windows_sysprep_plan("win.qcow2", Some("WEB01"), true);
        assert!(p.operations.iter().any(|o| o.id == "sysprep-flag"));
        let unattend = p.operations.iter().find(|o| o.id == "unattend").unwrap();
        match &unattend.op_type {
            OperationType::FileWrite(fw) => {
                assert!(fw.content.contains("<ComputerName>WEB01</ComputerName>"))
            }
            _ => panic!("expected file write"),
        }
    }

    #[test]
    fn escrow_hashes_key_and_omits_secret_by_default() {
        let tmp = tempfile::tempdir().unwrap();
        let key = tmp.path().join("key.txt");
        std::fs::write(&key, "123456-789012-ABCDEF").unwrap();
        let image = tmp.path().join("win.qcow2");
        std::fs::write(&image, b"fake").unwrap();
        let out = tmp.path().join("escrow.json");
        let (rec, plan) = bitlocker_escrow(&image, &key, false, &out).unwrap();
        assert_eq!(rec.key_bytes, 20);
        assert!(rec.key_preview.is_none());
        assert_eq!(rec.key_sha256.len(), 64);
        assert!(out.exists());
        assert!(plan.operations.iter().any(|o| o.id == "bitlocker-marker"));
        let raw = std::fs::read_to_string(&out).unwrap();
        assert!(!raw.contains("123456-789012-ABCDEF"));
    }
}
