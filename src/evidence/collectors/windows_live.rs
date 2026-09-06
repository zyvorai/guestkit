// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows live evidence (WMI + Event Log when agent runs on Windows).

use crate::evidence::snapshot::WindowsEvidence;

#[cfg(target_os = "windows")]
pub fn collect_windows_live() -> Option<WindowsEvidence> {
    let live = crate::collectors::windows_live::collect_windows_live();
    let mut evidence = WindowsEvidence::default();
    if let Some(ev) = live {
        evidence.product_name = ev.os_caption;
        evidence.version = ev.os_version;
        evidence.systemroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
        evidence.rdp_enabled = ev.rdp_enabled;
        evidence.pending_reboot = ev.pending_reboot;
        evidence.services_count = ev.services_count;
        evidence.event_logs.log_count = ev.event_log_channels.len();
        evidence.event_logs.forensic = Some(crate::evidence::snapshot::WindowsForensicProfile {
            service_failures: ev.critical_events_24h as u32,
            failed_logons: ev.error_events_24h as u32,
            ..Default::default()
        });
        evidence.av_edr = ev.av_products.clone();
    } else {
        evidence.systemroot = std::env::var("SystemRoot").unwrap_or_else(|_| "C:\\Windows".into());
    }

    // Schema-v4 migration evidence (live SCM / BitLocker / VSS / pnputil /
    // NIC config / BCD / licensing probes).
    use crate::collectors::windows_live as probes;
    evidence.virtio_drivers = probes::collect_virtio_drivers();
    evidence.bitlocker = probes::collect_bitlocker_state();
    if evidence
        .bitlocker
        .as_ref()
        .map(|b| b.any_protected)
        .unwrap_or(false)
    {
        evidence.bitlocker_detected = true;
    }
    evidence.vss = probes::collect_vss_health();
    evidence.ghost_nics = probes::collect_ghost_nics();
    evidence.static_nic_configs = probes::collect_nic_configs()
        .into_iter()
        .filter(|nic| !nic.dhcp)
        .collect();
    evidence.driver_signature_enforcement = probes::collect_driver_signature_enforcement();
    evidence.esp_present = probes::collect_esp_present();
    evidence.activation = probes::collect_activation_info();

    Some(evidence)
}

#[cfg(not(target_os = "windows"))]
pub fn collect_windows_live() -> Option<WindowsEvidence> {
    None
}
