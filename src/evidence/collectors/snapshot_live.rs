// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Snapshot / backup quiesce readiness from a live guest.

use crate::evidence::snapshot::SnapshotReadinessEvidence;
use std::path::Path;
use std::process::Command;

pub fn collect_snapshot_readiness_live() -> SnapshotReadinessEvidence {
    let fs_frozen = Command::new("fsfreeze").arg("--help").output().is_ok()
        && std::fs::read_to_string("/proc/mounts")
            .map(|m| m.contains("fsfreeze"))
            .unwrap_or(false);

    let guest_agent_connected = Path::new(guestkit_agent_protocol::VIRTIO_DEVICE_PATH).exists();
    let quiesce_supported = Path::new("/usr/sbin/fsfreeze").exists()
        || Path::new("/sbin/fsfreeze").exists()
        || Path::new("/usr/bin/fsfreeze").exists();

    let agent_active = Command::new("systemctl")
        .args(["is-active", "guestkit-agent"])
        .output()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "active")
        .unwrap_or(false);

    let mut notes = Vec::new();
    if !agent_active {
        notes.push("guestkit-agent systemd unit is not active".into());
    }
    if !quiesce_supported {
        notes.push("fsfreeze binary not found — freeze/thaw may be unavailable".into());
    }

    SnapshotReadinessEvidence {
        fs_frozen,
        guest_agent_connected,
        quiesce_supported,
        fstrim_recommended: Path::new("/usr/sbin/fstrim").exists(),
        notes,
    }
}
