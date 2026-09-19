// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Payloads h2kvm used to apply after `run_migrate_repair`.
//!
//! Network files, users, hostname, services, first-boot scripts, cloud-init,
//! Active Directory rejoin, and Windows license reactivation are plan
//! operations. GuestKit writes them onto the offline disk. h2kvm does not
//! mount the image again.

use crate::cli::plan::types::{
    CommandExec, FileWrite, FixPlan, Operation, OperationType, Priority, RegistryEdit,
    ServiceOperation,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

/// One file to write into the guest (netplan, ifcfg, NetworkManager, …).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkFile {
    pub path: String,
    pub content: String,
}

/// A user to create on first boot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct UserSpec {
    pub name: String,
    /// Linux crypt hash (`chpasswd -e`).
    #[serde(default)]
    pub password_hash: Option<String>,
    /// Plaintext password (`chpasswd`, or Windows `New-LocalUser`).
    #[serde(default)]
    pub password: Option<String>,
    #[serde(default)]
    pub groups: Vec<String>,
    #[serde(default)]
    pub ssh_keys: Vec<String>,
    /// `linux` (default) or `windows`.
    #[serde(default)]
    pub os: String,
}

/// Offline Active Directory rejoin. Stages `Add-Computer` on first boot.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AdRejoin {
    pub domain: String,
    #[serde(default)]
    pub ou: Option<String>,
    /// Account allowed to join the domain. The password is not stored in the plan.
    #[serde(default)]
    pub username: Option<String>,
}

/// Extra work applied with migration repair.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InjectPayload {
    #[serde(default)]
    pub hostname: Option<String>,
    #[serde(default)]
    pub network_files: Vec<NetworkFile>,
    #[serde(default)]
    pub users: Vec<UserSpec>,
    #[serde(default)]
    pub services: Vec<String>,
    #[serde(default)]
    pub firstboot: Vec<String>,
    #[serde(default)]
    pub cloud_init_user_data: Option<String>,
    #[serde(default)]
    pub ad_rejoin: Option<AdRejoin>,
    /// KMS host. Stages `slmgr /skms` and `slmgr /ato` on first boot.
    #[serde(default)]
    pub license_kms: Option<String>,
    #[serde(default)]
    pub enable_rdp: bool,
}

impl InjectPayload {
    pub fn is_empty(&self) -> bool {
        self.hostname.is_none()
            && self.network_files.is_empty()
            && self.users.is_empty()
            && self.services.is_empty()
            && self.firstboot.is_empty()
            && self.cloud_init_user_data.is_none()
            && self.ad_rejoin.is_none()
            && self.license_kms.is_none()
            && !self.enable_rdp
    }
}

fn push(plan: &mut FixPlan, description: &str, op_type: OperationType) {
    let n = plan.operations.len() + 1;
    plan.operations.push(Operation {
        id: format!("inject-{n:03}"),
        op_type,
        priority: Priority::Medium,
        description: description.into(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![],
        validation: None,
        undo: None,
    });
}

fn file_write(path: &str, content: &str, mode: Option<&str>) -> OperationType {
    OperationType::FileWrite(FileWrite {
        path: path.into(),
        content: content.into(),
        mode: mode.map(str::to_string),
    })
}

fn registry(key: &str, value: &str, data: serde_json::Value, data_type: &str) -> OperationType {
    OperationType::RegistryEdit(RegistryEdit {
        key: key.into(),
        value: value.into(),
        current_data: json!(null),
        new_data: data,
        data_type: data_type.into(),
    })
}

/// Append inject operations. No-op when the payload is empty.
pub fn append_inject(plan: &mut FixPlan, payload: &InjectPayload) {
    if payload.is_empty() {
        return;
    }

    if let Some(host) = payload.hostname.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        push(plan, "Set hostname", file_write("/etc/hostname", host, Some("0644")));
    }

    for file in &payload.network_files {
        let path = file.path.trim();
        if path.is_empty() || !path.starts_with('/') {
            continue;
        }
        push(
            plan,
            &format!("Write network config {path}"),
            file_write(path, &file.content, Some("0644")),
        );
    }

    for user in &payload.users {
        let name = user.name.trim();
        if name.is_empty() || name.contains(['/', ' ', '\n']) {
            continue;
        }
        if user.os.eq_ignore_ascii_case("windows") {
            let password_line = match user.password.as_deref().filter(|s| !s.is_empty()) {
                Some(pw) => format!(
                    "  $pw = ConvertTo-SecureString '{pw}' -AsPlainText -Force\n\
                     New-LocalUser -Name '{name}' -Password $pw\n"
                ),
                None => format!("  New-LocalUser -Name '{name}' -NoPassword\n"),
            };
            let script = format!(
                "$ErrorActionPreference='Stop'\n\
                 if (-not (Get-LocalUser -Name '{name}' -ErrorAction SilentlyContinue)) {{\n\
                 {password_line}\
                 }}\n"
            );
            let path = format!("/Windows/Temp/h2kvm-user-{name}.ps1");
            push(plan, &format!("Stage Windows user {name}"), file_write(&path, &script, None));
        } else {
            let groups = if user.groups.is_empty() {
                String::new()
            } else {
                format!(" -G {}", user.groups.join(","))
            };
            let mut script = format!(
                "#!/bin/sh\nid {name} >/dev/null 2>&1 || useradd -m{groups} {name}\n"
            );
            if let Some(hash) = user.password_hash.as_deref().filter(|s| !s.is_empty()) {
                script.push_str(&format!("printf '%s:%s\\n' {name} '{hash}' | chpasswd -e\n"));
            } else if let Some(pw) = user.password.as_deref().filter(|s| !s.is_empty()) {
                script.push_str(&format!("printf '%s:%s\\n' {name} '{pw}' | chpasswd\n"));
            }
            if !user.ssh_keys.is_empty() {
                script.push_str(&format!(
                    "install -d -m 700 -o {name} /home/{name}/.ssh\n\
                     cat > /home/{name}/.ssh/authorized_keys <<'EOF'\n{}\nEOF\n\
                     chown {name}:{name} /home/{name}/.ssh/authorized_keys\n\
                     chmod 600 /home/{name}/.ssh/authorized_keys\n",
                    user.ssh_keys.join("\n")
                ));
            }
            let path = format!("/usr/local/sbin/h2kvm-user-{name}");
            push(plan, &format!("Stage Linux user {name}"), file_write(&path, &script, Some("0755")));
            push(
                plan,
                &format!("Run user script for {name} on first boot"),
                OperationType::CommandExec(CommandExec {
                    command: path,
                    expected_exit: 0,
                    timeout: Some(60),
                    interpreter: Some("sh".into()),
                }),
            );
        }
    }

    for svc in &payload.services {
        let service = svc.trim();
        if service.is_empty() {
            continue;
        }
        push(
            plan,
            &format!("Enable {service}"),
            OperationType::ServiceOperation(ServiceOperation {
                service: service.into(),
                state: Some("enabled".into()),
                start: false,
                restart: false,
            }),
        );
    }

    for (i, body) in payload.firstboot.iter().enumerate() {
        let path = format!("/usr/local/sbin/h2kvm-firstboot-{i}");
        let script = if body.starts_with("#!") {
            body.clone()
        } else {
            format!("#!/bin/sh\n{body}\n")
        };
        push(plan, &format!("Stage first-boot script {i}"), file_write(&path, &script, Some("0755")));
        push(
            plan,
            &format!("Run first-boot script {i}"),
            OperationType::CommandExec(CommandExec {
                command: path,
                expected_exit: 0,
                timeout: Some(120),
                interpreter: Some("sh".into()),
            }),
        );
    }

    if let Some(data) = payload.cloud_init_user_data.as_deref().filter(|s| !s.is_empty()) {
        push(
            plan,
            "Write cloud-init user-data",
            file_write("/var/lib/cloud/seed/nocloud/user-data", data, Some("0644")),
        );
    }

    if let Some(ad) = &payload.ad_rejoin {
        let domain = ad.domain.trim();
        if !domain.is_empty() {
            let ou = ad.ou.as_deref().unwrap_or("");
            let user = ad.username.as_deref().unwrap_or("");
            let ps = format!(
                "$ErrorActionPreference='Stop'\n\
                 $cred = Get-Credential -UserName '{user}' -Message 'Join {domain}'\n\
                 Add-Computer -DomainName '{domain}' -OUPath '{ou}' -Credential $cred -Restart\n"
            );
            push(
                plan,
                "Stage Active Directory rejoin",
                file_write("/Windows/Temp/h2kvm-ad-rejoin.ps1", &ps, None),
            );
            push(
                plan,
                "RunOnce Active Directory rejoin",
                registry(
                    r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
                    "h2kvm-ad-rejoin",
                    json!("powershell -NoProfile -File C:\\Windows\\Temp\\h2kvm-ad-rejoin.ps1"),
                    "sz",
                ),
            );
        }
    }

    if let Some(kms) = payload.license_kms.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let ps = format!(
            "cscript //nologo C:\\Windows\\System32\\slmgr.vbs /skms {kms}\n\
             cscript //nologo C:\\Windows\\System32\\slmgr.vbs /ato\n"
        );
        push(
            plan,
            "Stage Windows license reactivation",
            file_write("/Windows/Temp/h2kvm-license.cmd", &ps, None),
        );
        push(
            plan,
            "RunOnce Windows license reactivation",
            registry(
                r"HKLM\SOFTWARE\Microsoft\Windows\CurrentVersion\RunOnce",
                "h2kvm-license",
                json!("C:\\Windows\\Temp\\h2kvm-license.cmd"),
                "sz",
            ),
        );
    }

    if payload.enable_rdp {
        push(
            plan,
            "Enable Remote Desktop",
            registry(
                r"HKLM\SYSTEM\CurrentControlSet\Control\Terminal Server",
                "fDenyTSConnections",
                json!(0),
                "dword",
            ),
        );
    }
}

/// Commands a live SSH fix should run. The caller executes them on the guest.
pub fn live_fix_commands(update_grub: bool, regen_initramfs: bool, remove_vmware_tools: bool) -> Vec<String> {
    let mut cmds = Vec::new();
    if remove_vmware_tools {
        cmds.push(
            "if command -v apt-get >/dev/null; then apt-get -y remove --purge open-vm-tools || true; \
             elif command -v dnf >/dev/null; then dnf -y remove open-vm-tools || true; \
             elif command -v yum >/dev/null; then yum -y remove open-vm-tools || true; fi"
                .into(),
        );
    }
    if regen_initramfs {
        cmds.push(
            "if command -v dracut >/dev/null; then dracut -f; \
             elif command -v update-initramfs >/dev/null; then update-initramfs -u; fi"
                .into(),
        );
    }
    if update_grub {
        cmds.push(
            "if command -v grub2-mkconfig >/dev/null; then grub2-mkconfig -o /boot/grub2/grub.cfg; \
             elif command -v update-grub >/dev/null; then update-grub; fi"
                .into(),
        );
    }
    cmds
}
