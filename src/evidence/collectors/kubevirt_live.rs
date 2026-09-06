// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! KubeVirt / virtio identity signals from a live Linux guest.

use crate::evidence::snapshot::{KubevirtEvidence, VirtioDiskEntry};
use std::path::Path;
use std::process::Command;

pub fn collect_kubevirt_live() -> KubevirtEvidence {
    let channel_path = guestkit_agent_protocol::VIRTIO_DEVICE_PATH;
    let virtio_channel_present = Path::new(channel_path).exists();
    let hostname = std::fs::read_to_string("/etc/hostname")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "localhost".into());
    let agent_service_active = Command::new("systemctl")
        .args(["is-active", "zyvor-guest-agent"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false)
        || Command::new("systemctl")
            .args(["is-active", "guestkit-agent"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
            .unwrap_or(false);
    KubevirtEvidence {
        hostname,
        virtio_channel_present,
        virtio_channel_path: channel_path.into(),
        cloud_init_config_present: Path::new("/etc/cloud").exists(),
        config_drive_mounted: Path::new("/mnt/config-drive").exists()
            || std::fs::read_to_string("/proc/mounts")
                .map(|m| m.contains("config-2") || m.contains("cidata"))
                .unwrap_or(false),
        agent_service_active,
        guestkit_version: crate::VERSION.into(),
        virtio_disks: collect_virtio_disks(),
    }
}

fn collect_virtio_disks() -> Vec<VirtioDiskEntry> {
    let mut disks = Vec::new();
    if let Ok(out) = Command::new("lsblk")
        .args(["-J", "-o", "NAME,SERIAL,MOUNTPOINT,TYPE"])
        .output()
    {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                walk_lsblk_disks(
                    json.get("blockdevices").and_then(|v| v.as_array()),
                    &mut disks,
                );
            }
        }
    }
    disks
}

fn walk_lsblk_disks(devices: Option<&Vec<serde_json::Value>>, out: &mut Vec<VirtioDiskEntry>) {
    let Some(devices) = devices else {
        return;
    };
    for dev in devices {
        let dtype = dev.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if dtype == "disk" {
            let name = dev.get("name").and_then(|v| v.as_str()).unwrap_or("");
            if name.starts_with("vd") || name.starts_with("sd") {
                let serial = dev
                    .get("serial")
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();
                let mut mountpoints = Vec::new();
                if let Some(mp) = dev.get("mountpoint").and_then(|v| v.as_str()) {
                    if !mp.is_empty() {
                        mountpoints.push(mp.to_string());
                    }
                }
                if let Some(children) = dev.get("children").and_then(|v| v.as_array()) {
                    for child in children {
                        if let Some(mp) = child.get("mountpoint").and_then(|v| v.as_str()) {
                            if !mp.is_empty() {
                                mountpoints.push(mp.to_string());
                            }
                        }
                    }
                }
                out.push(VirtioDiskEntry {
                    device: format!("/dev/{name}"),
                    serial,
                    mountpoints,
                });
            }
        }
        if let Some(children) = dev.get("children").and_then(|v| v.as_array()) {
            walk_lsblk_disks(Some(children), out);
        }
    }
}
