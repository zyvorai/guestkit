// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows WMI/Event Log collectors (live guest).

use guestkit_agent_protocol::{
    GuestHealth, GuestHealthComponents, HealthLevel, NetworkHealth, SecurityHealthSummary,
};
use serde::{Deserialize, Serialize};

#[cfg(target_os = "windows")]
use std::process::Command;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WindowsLiveEvidence {
    pub os_caption: String,
    pub os_version: String,
    pub last_boot: String,
    pub services_count: usize,
    pub running_services: usize,
    pub stopped_services: usize,
    pub event_log_channels: Vec<String>,
    pub critical_events_24h: usize,
    pub error_events_24h: usize,
    pub rdp_sessions: usize,
    pub rdp_enabled: bool,
    pub pending_updates: bool,
    pub pending_reboot: bool,
    pub firewall_enabled: bool,
    pub av_products: Vec<String>,
    pub ip_addresses: Vec<String>,
    pub default_gateway: Option<String>,
}

pub fn collect_windows_live() -> Option<WindowsLiveEvidence> {
    #[cfg(target_os = "windows")]
    {
        Some(collect_windows_live_inner())
    }
    #[cfg(not(target_os = "windows"))]
    {
        None
    }
}

#[cfg(target_os = "windows")]
fn collect_windows_live_inner() -> WindowsLiveEvidence {
    let os_caption = wmi_string("Win32_OperatingSystem", "Caption").unwrap_or_default();
    let os_version = wmi_string("Win32_OperatingSystem", "Version").unwrap_or_default();
    let last_boot = wmi_string("Win32_OperatingSystem", "LastBootUpTime").unwrap_or_default();
    let services_count = powershell_usize("(Get-Service | Measure-Object).Count").unwrap_or(0);
    let running_services =
        powershell_usize("(Get-Service | Where-Object Status -eq Running | Measure-Object).Count")
            .unwrap_or(0);
    let stopped_services = services_count.saturating_sub(running_services);
    let critical_events_24h = powershell_usize(
        "(Get-WinEvent -FilterHashtable @{LogName='System'; Level=1; StartTime=(Get-Date).AddHours(-24)} -ErrorAction SilentlyContinue | Measure-Object).Count",
    )
    .unwrap_or(0);
    let error_events_24h = powershell_usize(
        "(Get-WinEvent -FilterHashtable @{LogName='System'; Level=2; StartTime=(Get-Date).AddHours(-24)} -ErrorAction SilentlyContinue | Measure-Object).Count",
    )
    .unwrap_or(0);
    let pending_reboot = powershell_bool(
        "Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired'",
    )
    .unwrap_or(false);
    let pending_updates = powershell_bool(
        "(Get-CimInstance -Namespace root/Microsoft/Windows/WindowsUpdate -ClassName MSFT_WUSettings -ErrorAction SilentlyContinue) -ne $null",
    )
    .unwrap_or(false);
    let rdp_sessions =
        powershell_usize("(quser 2>$null | Measure-Object -Line).Lines").unwrap_or(0);
    let rdp_enabled = powershell_bool(
        "(Get-ItemProperty 'HKLM:\\System\\CurrentControlSet\\Control\\Terminal Server').fDenyTSConnections -eq 0",
    )
    .unwrap_or(false);
    let firewall_enabled = powershell_bool(
        "(Get-NetFirewallProfile -ErrorAction SilentlyContinue | Where-Object Enabled -eq $true | Measure-Object).Count -gt 0",
    )
    .unwrap_or(false);
    let av_products = powershell_lines(
        "Get-CimInstance -Namespace root/SecurityCenter2 -ClassName AntiVirusProduct -ErrorAction SilentlyContinue | Select-Object -ExpandProperty displayName",
    );
    let ip_addresses = powershell_lines(
        "Get-NetIPAddress -AddressFamily IPv4 -ErrorAction SilentlyContinue | Where-Object { $_.IPAddress -notlike '127.*' } | Select-Object -ExpandProperty IPAddress",
    );
    let default_gateway = powershell_string(
        "(Get-NetRoute -DestinationPrefix '0.0.0.0/0' -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty NextHop)",
    );

    WindowsLiveEvidence {
        os_caption,
        os_version,
        last_boot,
        services_count,
        running_services,
        stopped_services,
        event_log_channels: vec!["System".into(), "Application".into()],
        critical_events_24h,
        error_events_24h,
        rdp_sessions,
        rdp_enabled,
        pending_updates,
        pending_reboot,
        firewall_enabled,
        av_products,
        ip_addresses,
        default_gateway,
    }
}

#[cfg(target_os = "windows")]
fn wmi_string(class: &str, property: &str) -> Option<String> {
    powershell_string(&format!(
        "(Get-CimInstance -ClassName {class} -ErrorAction SilentlyContinue | Select-Object -First 1 -ExpandProperty {property})",
        class = class,
        property = property
    ))
}

#[cfg(target_os = "windows")]
fn powershell_usize(script: &str) -> Option<usize> {
    powershell_string(script)?.parse().ok()
}

#[cfg(target_os = "windows")]
fn powershell_bool(script: &str) -> Option<bool> {
    let raw = powershell_string(script)?;
    match raw.to_lowercase().as_str() {
        "true" | "1" => Some(true),
        "false" | "0" => Some(false),
        _ => raw.parse::<usize>().map(|n| n != 0).ok(),
    }
}

#[cfg(target_os = "windows")]
fn powershell_string(script: &str) -> Option<String> {
    let output = Command::new("powershell")
        .args([
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
    if text.is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(target_os = "windows")]
fn powershell_lines(script: &str) -> Vec<String> {
    powershell_string(script)
        .map(|s| {
            s.lines()
                .map(|l| l.trim().to_string())
                .filter(|l| !l.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

/// Pending-reboot detection via the standard registry markers.
/// Returns (pending, reasons).
#[cfg(target_os = "windows")]
pub fn collect_pending_reboot() -> (bool, Vec<String>) {
    let mut reasons = Vec::new();
    let checks = [
        (
            "Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\Component Based Servicing\\RebootPending'",
            "component-based servicing reboot pending",
        ),
        (
            "Test-Path 'HKLM:\\SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\WindowsUpdate\\Auto Update\\RebootRequired'",
            "Windows Update reboot required",
        ),
        (
            "[bool](Get-ItemProperty 'HKLM:\\SYSTEM\\CurrentControlSet\\Control\\Session Manager' -Name PendingFileRenameOperations -ErrorAction SilentlyContinue)",
            "pending file rename operations",
        ),
    ];
    for (script, reason) in checks {
        if powershell_bool(script).unwrap_or(false) {
            reasons.push(reason.to_string());
        }
    }
    (!reasons.is_empty(), reasons)
}

#[cfg(not(target_os = "windows"))]
pub fn collect_pending_reboot() -> (bool, Vec<String>) {
    (false, Vec::new())
}

/// Automatic-start services that are not running — the Windows analogue of
/// systemd failed units for the heartbeat's `critical_services_failed`.
#[cfg(target_os = "windows")]
pub fn failed_auto_services() -> Vec<String> {
    powershell_lines(
        "Get-CimInstance Win32_Service -Filter \"StartMode='Auto' and State<>'Running' and DelayedAutoStart=0\" \
         -ErrorAction SilentlyContinue | Where-Object { $_.ExitCode -ne 0 } | Select-Object -ExpandProperty Name",
    )
}

#[cfg(not(target_os = "windows"))]
pub fn failed_auto_services() -> Vec<String> {
    Vec::new()
}

// --- Migration-evidence probes (schema v4) ---

use crate::evidence::snapshot::{
    ActivationInfo, BitLockerState, BitLockerVolume, GhostNicEntry, VssHealth, WindowsDriverEntry,
    WindowsNicConfig,
};

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
const VIRTIO_DRIVER_NAMES: &[&str] = &[
    "viostor", "vioscsi", "netkvm", "vioser", "balloon", "viorng",
];

/// VirtIO driver presence + start mode from the live SCM.
#[cfg(target_os = "windows")]
pub fn collect_virtio_drivers() -> Vec<WindowsDriverEntry> {
    let filter = VIRTIO_DRIVER_NAMES
        .iter()
        .map(|n| format!("Name='{n}'"))
        .collect::<Vec<_>>()
        .join(" or ");
    let json = powershell_string(&format!(
        "Get-CimInstance Win32_SystemDriver -Filter \"{filter}\" -ErrorAction SilentlyContinue | \
         Select-Object Name,StartMode,State | ConvertTo-Json -Compress"
    ))
    .unwrap_or_default();
    parse_virtio_driver_json(&json)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_virtio_driver_json(json: &str) -> Vec<WindowsDriverEntry> {
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "Name")]
        name: String,
        #[serde(rename = "StartMode", default)]
        start_mode: Option<String>,
    }
    let rows: Vec<Row> = if json.trim_start().starts_with('[') {
        serde_json::from_str(json).unwrap_or_default()
    } else if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str::<Row>(json)
            .map(|r| vec![r])
            .unwrap_or_default()
    };
    VIRTIO_DRIVER_NAMES
        .iter()
        .map(|name| {
            let row = rows.iter().find(|r| r.name.eq_ignore_ascii_case(name));
            match row {
                Some(r) => {
                    let start = r
                        .start_mode
                        .clone()
                        .unwrap_or_default()
                        .to_ascii_lowercase();
                    WindowsDriverEntry {
                        name: name.to_string(),
                        version: None,
                        boot_critical: start == "boot",
                        start_type: start,
                        present: true,
                        sys_present: None,
                    }
                }
                None => WindowsDriverEntry {
                    name: name.to_string(),
                    ..Default::default()
                },
            }
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn collect_bitlocker_state() -> Option<BitLockerState> {
    let json = powershell_string(
        "Get-BitLockerVolume -ErrorAction SilentlyContinue | \
         Select-Object MountPoint,ProtectionStatus | ConvertTo-Json -Compress",
    )?;
    parse_bitlocker_json(&json)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_bitlocker_json(json: &str) -> Option<BitLockerState> {
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "MountPoint", default)]
        mount_point: String,
        // 0=Off, 1=On; PowerShell may render the enum as a string.
        #[serde(rename = "ProtectionStatus", default)]
        protection_status: serde_json::Value,
    }
    let rows: Vec<Row> = if json.trim_start().starts_with('[') {
        serde_json::from_str(json).ok()?
    } else {
        vec![serde_json::from_str(json).ok()?]
    };
    let volumes: Vec<BitLockerVolume> = rows
        .into_iter()
        .map(|r| {
            let protection = match &r.protection_status {
                serde_json::Value::Number(n) if n.as_u64() == Some(1) => "on",
                serde_json::Value::String(s) if s.eq_ignore_ascii_case("on") => "on",
                _ => "off",
            };
            BitLockerVolume {
                mount_point: r.mount_point,
                protection: protection.to_string(),
                source: Some("Get-BitLockerVolume".into()),
            }
        })
        .collect();
    Some(BitLockerState {
        any_protected: volumes.iter().any(|v| v.protection == "on"),
        offline_uncertain: false,
        volumes,
    })
}

#[cfg(target_os = "windows")]
pub fn collect_vss_health() -> Option<VssHealth> {
    let out = Command::new("vssadmin")
        .args(["list", "writers"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_vss_writers(&String::from_utf8_lossy(&out.stdout)))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_vss_writers(text: &str) -> VssHealth {
    let mut writers_total = 0usize;
    let mut writers_failed = Vec::new();
    let mut current: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(name) = line.strip_prefix("Writer name:") {
            writers_total += 1;
            current = Some(name.trim().trim_matches('\'').to_string());
        } else if let Some(state) = line.strip_prefix("State:") {
            let stable = state.contains("Stable");
            if !stable {
                if let Some(name) = current.take() {
                    writers_failed.push(name);
                }
            }
        }
    }
    VssHealth {
        writers_total,
        healthy: writers_failed.is_empty() && writers_total > 0,
        writers_failed,
        offline_inferred: false,
    }
}

#[cfg(target_os = "windows")]
pub fn collect_ghost_nics() -> Vec<GhostNicEntry> {
    let out = Command::new("pnputil")
        .args(["/enum-devices", "/class", "Net", "/disconnected"])
        .output();
    match out {
        Ok(o) if o.status.success() => parse_pnputil_devices(&String::from_utf8_lossy(&o.stdout)),
        _ => Vec::new(),
    }
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_pnputil_devices(text: &str) -> Vec<GhostNicEntry> {
    let mut entries = Vec::new();
    let mut instance: Option<String> = None;
    for line in text.lines() {
        let line = line.trim();
        if let Some(id) = line.strip_prefix("Instance ID:") {
            instance = Some(id.trim().to_string());
        } else if let Some(desc) = line.strip_prefix("Device Description:") {
            if let Some(id) = instance.take() {
                entries.push(GhostNicEntry {
                    instance_id: id,
                    description: desc.trim().to_string(),
                });
            }
        }
    }
    entries
}

#[cfg(target_os = "windows")]
pub fn collect_nic_configs() -> Vec<WindowsNicConfig> {
    let json = powershell_string(
        "Get-CimInstance Win32_NetworkAdapterConfiguration -Filter 'IPEnabled=true' \
         -ErrorAction SilentlyContinue | \
         Select-Object Description,MACAddress,IPAddress,DefaultIPGateway,DNSServerSearchOrder,DHCPEnabled | \
         ConvertTo-Json -Compress",
    )
    .unwrap_or_default();
    parse_nic_config_json(&json)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_nic_config_json(json: &str) -> Vec<WindowsNicConfig> {
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "Description", default)]
        description: String,
        #[serde(rename = "MACAddress", default)]
        mac: Option<String>,
        #[serde(rename = "IPAddress", default)]
        ips: Option<Vec<String>>,
        #[serde(rename = "DefaultIPGateway", default)]
        gateways: Option<Vec<String>>,
        #[serde(rename = "DNSServerSearchOrder", default)]
        dns: Option<Vec<String>>,
        #[serde(rename = "DHCPEnabled", default)]
        dhcp: bool,
    }
    let rows: Vec<Row> = if json.trim_start().starts_with('[') {
        serde_json::from_str(json).unwrap_or_default()
    } else if json.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str::<Row>(json)
            .map(|r| vec![r])
            .unwrap_or_default()
    };
    rows.into_iter()
        .map(|r| WindowsNicConfig {
            name: r.description,
            mac: r.mac.unwrap_or_default(),
            ip_addresses: r.ips.unwrap_or_default(),
            gateway: r.gateways.and_then(|g| g.into_iter().next()),
            dns: r.dns.unwrap_or_default(),
            dhcp: r.dhcp,
        })
        .collect()
}

#[cfg(target_os = "windows")]
pub fn collect_driver_signature_enforcement() -> Option<bool> {
    let out = Command::new("bcdedit")
        .args(["/enum", "{current}"])
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    Some(parse_signature_enforcement(&String::from_utf8_lossy(
        &out.stdout,
    )))
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_signature_enforcement(bcd_text: &str) -> bool {
    let weakened = bcd_text.lines().any(|l| {
        let l = l.to_ascii_lowercase();
        (l.contains("testsigning") || l.contains("nointegritychecks")) && l.contains("yes")
    });
    !weakened
}

#[cfg(target_os = "windows")]
pub fn collect_activation_info() -> Option<ActivationInfo> {
    let json = powershell_string(
        "Get-CimInstance SoftwareLicensingProduct -Filter \
         \"PartialProductKey IS NOT NULL and ApplicationID='55c92734-d682-4d71-983e-d6ec3f16059f'\" \
         -ErrorAction SilentlyContinue | Select-Object -First 1 LicenseStatus,ProductKeyChannel | \
         ConvertTo-Json -Compress",
    )?;
    parse_activation_json(&json)
}

#[cfg_attr(not(target_os = "windows"), allow(dead_code))]
fn parse_activation_json(json: &str) -> Option<ActivationInfo> {
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "LicenseStatus", default)]
        status: u32,
        #[serde(rename = "ProductKeyChannel", default)]
        channel: Option<String>,
    }
    let row: Row = serde_json::from_str(json).ok()?;
    Some(ActivationInfo {
        licensed: row.status == 1,
        channel: row.channel.unwrap_or_default(),
        product_id: None,
        edition_id: None,
    })
}

pub fn build_windows_guest_health(hostname: &str) -> GuestHealth {
    let evidence = collect_windows_live();
    let (level, score, reasons, components, journal_hints) = if let Some(ev) = &evidence {
        let mut reasons = Vec::new();
        let mut score = 90u8;
        if ev.pending_reboot {
            score = score.saturating_sub(15);
            reasons.push("Pending Windows reboot".into());
        }
        if ev.pending_updates {
            score = score.saturating_sub(5);
            reasons.push("Windows Update activity detected".into());
        }
        if ev.critical_events_24h > 0 {
            score = score.saturating_sub(25);
            reasons.push(format!(
                "{} critical System events in last 24h",
                ev.critical_events_24h
            ));
        }
        if ev.error_events_24h > 5 {
            score = score.saturating_sub(10);
            reasons.push(format!(
                "{} error System events in last 24h",
                ev.error_events_24h
            ));
        }
        if !ev.firewall_enabled {
            score = score.saturating_sub(10);
            reasons.push("Windows Firewall disabled on a profile".into());
        }
        let level = if score >= 80 {
            HealthLevel::Healthy
        } else if score >= 50 {
            HealthLevel::Degraded
        } else {
            HealthLevel::Unhealthy
        };
        let security_level = if ev.firewall_enabled && ev.av_products.is_empty() {
            HealthLevel::Degraded
        } else if ev.firewall_enabled {
            HealthLevel::Healthy
        } else {
            HealthLevel::Unhealthy
        };
        let components = GuestHealthComponents {
            boot: HealthLevel::Healthy,
            systemd: if ev.stopped_services > 5 {
                HealthLevel::Degraded
            } else {
                HealthLevel::Healthy
            },
            network: if ev.ip_addresses.is_empty() {
                HealthLevel::Degraded
            } else {
                HealthLevel::Healthy
            },
            dns: HealthLevel::Unknown,
            storage: HealthLevel::Unknown,
            security: security_level,
            agent: HealthLevel::Healthy,
        };
        let journal_hints = if ev.critical_events_24h > 0 {
            vec![format!(
                "system:critical_events_24h={}",
                ev.critical_events_24h
            )]
        } else {
            vec![]
        };
        (level, score, reasons, components, journal_hints)
    } else {
        (
            HealthLevel::Unknown,
            50,
            vec![],
            GuestHealthComponents::default(),
            vec![],
        )
    };

    let network = evidence
        .as_ref()
        .map(|ev| NetworkHealth {
            default_route: ev.default_gateway.is_some() || !ev.ip_addresses.is_empty(),
            dns_working: true,
            dns_error: None,
            interfaces_up: ev.ip_addresses.len(),
            cluster_dns_reachable: false,
        })
        .unwrap_or_default();

    let security = evidence
        .as_ref()
        .map(|ev| SecurityHealthSummary {
            pending_security_updates: ev.pending_updates,
            firewall_enabled: ev.firewall_enabled,
            selinux: ev
                .av_products
                .first()
                .cloned()
                .unwrap_or_else(|| "none".into()),
        })
        .unwrap_or_default();

    GuestHealth {
        vm_hostname: hostname.to_string(),
        guest_health: level,
        boot_state: evidence
            .as_ref()
            .map(|e| e.last_boot.clone())
            .unwrap_or_else(|| "running".into()),
        systemd_state: format!(
            "windows-services:{} running",
            evidence.as_ref().map(|e| e.running_services).unwrap_or(0)
        ),
        failed_units: evidence
            .as_ref()
            .map(|e| e.critical_events_24h + e.error_events_24h)
            .unwrap_or(0),
        critical_services: vec![],
        network,
        storage: Default::default(),
        security,
        recommendations: vec![],
        collected_at: chrono::Utc::now().to_rfc3339(),
        agent_version: crate::VERSION.to_string(),
        score,
        reasons,
        components,
        journal_hints,
    }
}

#[cfg(target_os = "windows")]
pub fn collect_esp_present() -> Option<bool> {
    powershell_bool(
        "[bool](Get-Partition -ErrorAction SilentlyContinue | \
         Where-Object { $_.GptType -eq '{c12a7328-f81f-11d2-ba4b-00a0c93ec93b}' })",
    )
}

#[cfg(test)]
mod migration_probe_tests {
    use super::*;

    #[test]
    fn virtio_json_marks_missing_and_boot_critical() {
        let json = r#"[{"Name":"viostor","StartMode":"Boot","State":"Running"},
                       {"Name":"netkvm","StartMode":"Auto","State":"Running"}]"#;
        let drivers = parse_virtio_driver_json(json);
        let by = |n: &str| drivers.iter().find(|d| d.name == n).unwrap();
        assert!(by("viostor").present && by("viostor").boot_critical);
        assert!(by("netkvm").present && !by("netkvm").boot_critical);
        assert!(!by("vioscsi").present);
    }

    #[test]
    fn vss_writer_parse() {
        let text = "\
Writer name: 'System Writer'\n   State: [1] Stable\n\
Writer name: 'SqlServerWriter'\n   State: [8] Failed\n";
        let health = parse_vss_writers(text);
        assert_eq!(health.writers_total, 2);
        assert_eq!(health.writers_failed, vec!["SqlServerWriter"]);
        assert!(!health.healthy);
    }

    #[test]
    fn pnputil_ghost_nic_parse() {
        let text = "\
Instance ID:  PCI\\VEN_15AD&DEV_07B0\\000000\n\
Device Description:  vmxnet3 Ethernet Adapter\n\
Status:  Disconnected\n";
        let nics = parse_pnputil_devices(text);
        assert_eq!(nics.len(), 1);
        assert!(nics[0].description.contains("vmxnet3"));
    }

    #[test]
    fn signature_enforcement_parse() {
        assert!(!parse_signature_enforcement("testsigning             Yes"));
        assert!(parse_signature_enforcement("description  Windows 10"));
    }

    #[test]
    fn bitlocker_parse_single_object() {
        let state = parse_bitlocker_json(r#"{"MountPoint":"C:","ProtectionStatus":1}"#).unwrap();
        assert!(state.any_protected);
        assert_eq!(state.volumes[0].protection, "on");
    }

    #[test]
    fn nic_config_parse() {
        let json = r#"{"Description":"Intel NIC","MACAddress":"00:11:22:33:44:55",
                       "IPAddress":["10.0.0.5"],"DefaultIPGateway":["10.0.0.1"],
                       "DNSServerSearchOrder":["10.0.0.2"],"DHCPEnabled":false}"#;
        let nics = parse_nic_config_json(json);
        assert_eq!(nics.len(), 1);
        assert!(!nics[0].dhcp);
        assert_eq!(nics[0].gateway.as_deref(), Some("10.0.0.1"));
    }
}
