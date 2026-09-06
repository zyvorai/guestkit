// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Storage operations: rescan, TRIM, and (opt-in) filesystem expansion.

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::process::Command;

/// Rescan storage buses so newly attached/resized disks appear.
pub fn rescan() -> Result<Value> {
    if cfg!(windows) {
        let output = Command::new("powershell")
            .args([
                "-NoProfile",
                "-NonInteractive",
                "-Command",
                "'rescan' | diskpart",
            ])
            .output()
            .context("diskpart rescan")?;
        if !output.status.success() {
            bail!("diskpart rescan failed");
        }
        return Ok(json!({ "message": "disk rescan complete" }));
    }

    let mut hosts = 0usize;
    if let Ok(entries) = std::fs::read_dir("/sys/class/scsi_host") {
        for entry in entries.flatten() {
            let scan = entry.path().join("scan");
            if std::fs::write(&scan, "- - -").is_ok() {
                hosts += 1;
            }
        }
    }
    let mut devices = 0usize;
    if let Ok(entries) = std::fs::read_dir("/sys/block") {
        for entry in entries.flatten() {
            let rescan = entry.path().join("device/rescan");
            if std::fs::write(&rescan, "1").is_ok() {
                devices += 1;
            }
        }
    }
    Ok(json!({
        "message": format!("rescanned {hosts} SCSI host(s), {devices} block device(s)"),
        "scsi_hosts": hosts,
        "block_devices": devices,
    }))
}

/// TRIM/discard mounted filesystems.
pub fn trim(params: &Value) -> Result<Value> {
    if cfg!(windows) {
        let drive = params.get("mount").and_then(Value::as_str).unwrap_or("C:");
        let output = Command::new("defrag")
            .args([drive, "/L"])
            .output()
            .context("defrag /L")?;
        if !output.status.success() {
            bail!(
                "defrag /L failed: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }
        return Ok(json!({ "message": format!("TRIM complete on {drive}") }));
    }

    let output = match params.get("mount").and_then(Value::as_str) {
        Some(mount) => Command::new("fstrim").arg("-v").arg(mount).output(),
        None => Command::new("fstrim").args(["-va"]).output(),
    }
    .context("fstrim")?;
    if !output.status.success() {
        bail!("fstrim failed: {}", String::from_utf8_lossy(&output.stderr));
    }
    Ok(json!({
        "message": String::from_utf8_lossy(&output.stdout).trim(),
    }))
}

/// Expand a filesystem after its backing disk grew. Policy-gated
/// (`storage_ops.expand`, off by default), dry-run first, and routed
/// through the privileged helper when present.
pub fn expand(params: &Value) -> Result<Value> {
    let mount = params
        .get("mount")
        .and_then(Value::as_str)
        .context("missing required param: mount")?;
    let dry_run = params
        .get("dry_run")
        .and_then(Value::as_bool)
        .unwrap_or(true);

    if cfg!(windows) {
        bail!("use Windows Storage APIs / diskpart extend — not yet automated");
    }

    // Identify device + fstype for the mount.
    let findmnt = Command::new("findmnt")
        .args(["-n", "-o", "SOURCE,FSTYPE", mount])
        .output()
        .context("findmnt")?;
    if !findmnt.status.success() {
        bail!("mount point {mount} not found");
    }
    let out = String::from_utf8_lossy(&findmnt.stdout);
    let mut parts = out.split_whitespace();
    let source = parts.next().unwrap_or_default().to_string();
    let fstype = parts.next().unwrap_or_default().to_string();

    let grow_cmd = match fstype.as_str() {
        "xfs" => format!("xfs_growfs {mount}"),
        "ext2" | "ext3" | "ext4" => format!("resize2fs {source}"),
        "btrfs" => format!("btrfs filesystem resize max {mount}"),
        other => bail!("unsupported filesystem for expansion: {other}"),
    };
    // LVM logical volumes grow the LV and the filesystem together.
    let full_cmd = if source.starts_with("/dev/mapper/") || source.contains("/dev/dm-") {
        format!("lvextend -l +100%FREE -r {source}")
    } else {
        grow_cmd
    };

    if dry_run {
        return Ok(json!({
            "dry_run": true,
            "mount": mount,
            "device": source,
            "fstype": fstype,
            "planned_command": full_cmd,
        }));
    }

    // Prefer the privileged helper.
    if crate::agent::executor_ipc::executor_available() {
        if let Ok(result) = crate::agent::executor_ipc::call_executor(
            "expand_filesystem",
            json!({ "command": full_cmd }),
        ) {
            return Ok(json!({ "message": result }));
        }
    }
    let status = Command::new("sh").arg("-c").arg(&full_cmd).status()?;
    if !status.success() {
        bail!("expansion command failed: {full_cmd}");
    }
    crate::agent::audit::audit("storage.expand", "ok", mount);
    Ok(json!({ "message": format!("expanded {mount} via {full_cmd}") }))
}
