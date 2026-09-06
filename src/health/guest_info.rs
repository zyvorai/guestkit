// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Build GuestInfo from live evidence.

use crate::evidence::snapshot::EvidenceSnapshot;
use guestkit_agent_protocol::{
    GuestIdentity, GuestInfo, GuestOsInfo, GuestVirtualizationInfo, ServiceHealth,
};

pub fn build_guest_info(evidence: &EvidenceSnapshot) -> GuestInfo {
    let qga_installed = evidence
        .vm_tools
        .detected
        .iter()
        .any(|t| t.contains("qemu-guest-agent") || t.contains("qemu-ga"));
    let qga_running = evidence
        .kubevirt
        .as_ref()
        .map(|k| k.agent_service_active || k.virtio_channel_present)
        .unwrap_or(false);

    let hardware = evidence.hardware.as_ref();

    GuestInfo {
        hostname: evidence.os.hostname.clone(),
        os: GuestOsInfo {
            family: if evidence.os.os_type.is_empty() {
                "linux".into()
            } else {
                evidence.os.os_type.clone()
            },
            id: evidence.os.os_type.clone(),
            version: evidence.os.version.clone(),
            kernel: read_kernel_version(),
            architecture: evidence.os.architecture.clone(),
        },
        virtualization: GuestVirtualizationInfo {
            detected: evidence
                .systemd
                .as_ref()
                .and_then(|s| s.runtime.as_ref())
                .and_then(|r| r.manager.as_ref())
                .map(|m| m.virtualization.clone())
                .unwrap_or_else(|| "kvm".into()),
            qga_installed,
            qga_running,
            zyvor_agent_version: crate::VERSION.to_string(),
        },
        identity: GuestIdentity {
            machine_id: hardware.map(|h| h.machine_id.clone()).unwrap_or_default(),
            dmi_uuid: hardware.map(|h| h.dmi_uuid.clone()).unwrap_or_default(),
            zeus_vm_uid: hardware.and_then(|h| h.zeus_vm_uid.clone()),
        },
    }
}

pub fn build_service_health(unit_name: &str, evidence: &EvidenceSnapshot) -> Option<ServiceHealth> {
    let runtime = evidence.systemd.as_ref().and_then(|s| s.runtime.as_ref())?;
    let unit = runtime.units.iter().find(|u| u.name == unit_name)?;
    let journal = crate::journal::live::collect_journal_slice(&unit.name, 20);
    Some(ServiceHealth {
        name: unit.name.clone(),
        state: unit.active_state.clone(),
        sub_state: unit.sub_state.clone(),
        main_pid: unit.main_pid,
        exit_code: unit.exec_main_status,
        restart_count: unit.n_restarts,
        last_failure: journal.last_error.as_ref().map(|e| e.message.clone()),
        journal_cursor: journal.cursor.clone(),
        actions: vec![
            "view_logs".into(),
            "restart_unit".into(),
            "collect_service_bundle".into(),
        ],
    })
}

fn read_kernel_version() -> String {
    std::process::Command::new("uname")
        .arg("-r")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
        .unwrap_or_default()
}
