// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GuestHealth builder from evidence + live runtime collectors.

mod guest_info;
mod platform;

pub use guest_info::{build_guest_info, build_service_health};
pub use platform::build_guest_health_live;

use crate::ai::semantic::analyze_semantic;
use crate::collectors::dbus::{collect_dns_health_safe, collect_timedate_health_safe};
use crate::evidence::snapshot::{EvidenceSnapshot, SystemdRuntimeInfo, SystemdRuntimeUnit};
use crate::journal::analyze::suggest_action_from_patterns;
use chrono::Utc;
use guestkit_agent_protocol::{
    CriticalService, GuestHealth, GuestHealthComponents, HealthLevel, NetworkHealth,
    Recommendation, SecurityHealthSummary, StorageHealth,
};

pub fn build_guest_health(evidence: &EvidenceSnapshot) -> GuestHealth {
    let semantic = analyze_semantic(evidence);
    let runtime = evidence.systemd.as_ref().and_then(|s| s.runtime.as_ref());

    let failed_units = runtime
        .map(|r| r.failed_unit_count)
        .unwrap_or_else(|| semantic.problem_units.len());

    let critical_services = build_critical_services(runtime, &semantic, evidence);

    let components = build_components(evidence, runtime);
    let reasons = build_reasons(evidence, runtime, &components);
    let guest_health = derive_health_level(failed_units, evidence, runtime, &components);
    let score = health_score(&guest_health, &components);

    let systemd_state = runtime
        .and_then(|r| r.manager.as_ref())
        .map(|m| m.system_state.clone())
        .or_else(|| runtime.map(|r| r.manager_state.clone()))
        .unwrap_or_else(|| "unknown".into());

    let network = build_network_health(evidence);
    let storage = build_storage_health(evidence);
    let security = SecurityHealthSummary {
        pending_security_updates: evidence.security.pending_security_updates,
        firewall_enabled: evidence.security.firewall_enabled,
        selinux: evidence.security.selinux.clone(),
    };

    let recommendations =
        build_recommendations(evidence, runtime, &guest_health, &critical_services);

    GuestHealth {
        vm_hostname: evidence.os.hostname.clone(),
        guest_health,
        boot_state: "running".into(),
        systemd_state,
        failed_units,
        critical_services,
        network,
        storage,
        security,
        recommendations,
        collected_at: Utc::now().to_rfc3339(),
        agent_version: crate::VERSION.to_string(),
        score,
        reasons,
        components,
        journal_hints: semantic.journal_hints.clone(),
    }
}

fn build_components(
    evidence: &EvidenceSnapshot,
    runtime: Option<&SystemdRuntimeInfo>,
) -> GuestHealthComponents {
    let dns = collect_dns_health_safe();
    let timedate = collect_timedate_health_safe();

    let systemd_level = if runtime
        .and_then(|r| r.manager.as_ref())
        .map(|m| m.system_state == "degraded" || m.system_state == "maintenance")
        .unwrap_or(false)
        || runtime.map(|r| r.failed_unit_count > 0).unwrap_or(false)
    {
        HealthLevel::Degraded
    } else if runtime.is_some() {
        HealthLevel::Healthy
    } else {
        HealthLevel::Unknown
    };

    let dns_level = if dns.errors.is_empty()
        && evidence
            .network_probes
            .as_ref()
            .map(|p| p.cluster_dns_reachable)
            .unwrap_or(true)
    {
        HealthLevel::Healthy
    } else if evidence
        .network_probes
        .as_ref()
        .map(|p| !p.cluster_dns_reachable)
        .unwrap_or(false)
    {
        HealthLevel::Unhealthy
    } else {
        HealthLevel::Degraded
    };

    let storage = build_storage_health(evidence);
    let storage_level = if storage.root_usage_percent >= 95 || storage.inode_usage_percent >= 95 {
        HealthLevel::Unhealthy
    } else if storage.root_usage_percent >= 90 || storage.read_only_mounts > 0 {
        HealthLevel::Degraded
    } else {
        HealthLevel::Healthy
    };

    let network_level = if evidence.network.interfaces.is_empty() {
        HealthLevel::Degraded
    } else {
        HealthLevel::Healthy
    };

    let boot_level = if timedate.ntp_synchronized || timedate.ntp_enabled {
        HealthLevel::Healthy
    } else {
        HealthLevel::Degraded
    };

    GuestHealthComponents {
        boot: boot_level,
        systemd: systemd_level,
        network: network_level,
        dns: dns_level,
        storage: storage_level,
        security: if evidence.security.pending_security_updates {
            HealthLevel::Degraded
        } else {
            HealthLevel::Healthy
        },
        agent: HealthLevel::Healthy,
    }
}

fn build_reasons(
    evidence: &EvidenceSnapshot,
    runtime: Option<&SystemdRuntimeInfo>,
    components: &GuestHealthComponents,
) -> Vec<String> {
    let mut out = Vec::new();
    if components.systemd != HealthLevel::Healthy {
        out.push("systemd_degraded".into());
    }
    if components.dns == HealthLevel::Unhealthy || components.dns == HealthLevel::Degraded {
        out.push("dns_unhealthy".into());
    }
    if components.storage != HealthLevel::Healthy {
        out.push("disk_pressure".into());
    }
    if runtime.map(|r| r.failed_unit_count > 0).unwrap_or(false) {
        out.push("failed_units".into());
    }
    if evidence
        .network_probes
        .as_ref()
        .map(|p| !p.cluster_dns_reachable)
        .unwrap_or(false)
    {
        out.push("cluster_dns_unreachable".into());
    }
    out
}

fn health_score(overall: &HealthLevel, components: &GuestHealthComponents) -> u8 {
    let base: u8 = match overall {
        HealthLevel::Healthy => 90,
        HealthLevel::Degraded => 62,
        HealthLevel::Unhealthy => 35,
        HealthLevel::Unknown => 50,
    };
    let mut penalty = 0u8;
    for level in [
        components.systemd,
        components.dns,
        components.storage,
        components.network,
        components.security,
    ] {
        match level {
            HealthLevel::Degraded => penalty += 5,
            HealthLevel::Unhealthy => penalty += 15,
            _ => {}
        }
    }
    base.saturating_sub(penalty)
}

fn derive_health_level(
    failed_units: usize,
    evidence: &EvidenceSnapshot,
    runtime: Option<&SystemdRuntimeInfo>,
    components: &GuestHealthComponents,
) -> HealthLevel {
    if failed_units > 3 || components.dns == HealthLevel::Unhealthy {
        return HealthLevel::Unhealthy;
    }
    if failed_units > 0
        || components.systemd == HealthLevel::Degraded
        || components.storage == HealthLevel::Degraded
        || storage_pressure_high(evidence)
        || evidence
            .network_probes
            .as_ref()
            .map(|p| !p.cluster_dns_reachable)
            .unwrap_or(false)
    {
        return HealthLevel::Degraded;
    }
    if runtime
        .map(|r| r.manager_state == "degraded")
        .unwrap_or(false)
    {
        return HealthLevel::Degraded;
    }
    HealthLevel::Healthy
}

fn storage_pressure_high(evidence: &EvidenceSnapshot) -> bool {
    build_storage_health(evidence).root_usage_percent >= 90
}

fn build_critical_services(
    runtime: Option<&SystemdRuntimeInfo>,
    semantic: &crate::ai::semantic::SemanticAnalysis,
    evidence: &EvidenceSnapshot,
) -> Vec<CriticalService> {
    let mut out = Vec::new();

    if let Some(rt) = runtime {
        for unit in rt.units.iter().filter(|u| u.active_state == "failed") {
            out.push(runtime_unit_to_critical(unit, evidence));
        }
    }

    for problem in &semantic.problem_units {
        if out.iter().any(|c| c.name == problem.name) {
            continue;
        }
        out.push(CriticalService {
            name: problem.name.clone(),
            state: problem.state.clone(),
            sub_state: "unknown".into(),
            reason: problem.description.clone().unwrap_or_default(),
            last_exit_code: None,
            suggested_action: format!("Inspect {} and view journal logs", problem.name),
            main_pid: None,
            last_failure: None,
        });
    }

    out
}

fn runtime_unit_to_critical(
    unit: &SystemdRuntimeUnit,
    _evidence: &EvidenceSnapshot,
) -> CriticalService {
    let journal = crate::journal::live::collect_journal_slice(&unit.name, 30);
    let last_failure = journal
        .last_error
        .as_ref()
        .map(|e| e.message.clone())
        .or_else(|| {
            journal
                .entries
                .iter()
                .find(|e| e.priority <= 3)
                .map(|e| e.message.clone())
        });

    let suggested = suggest_action_from_patterns(&journal.top_patterns)
        .unwrap_or_else(|| format!("Restart {} or inspect journal for {}", unit.name, unit.name));

    let reason = if unit.exec_main_status.unwrap_or(0) != 0 {
        format!("exit code {}", unit.exec_main_status.unwrap_or(0))
    } else if let Some(msg) = &last_failure {
        msg.clone()
    } else {
        format!("unit in {} / {}", unit.active_state, unit.sub_state)
    };

    CriticalService {
        name: unit.name.clone(),
        state: unit.active_state.clone(),
        sub_state: unit.sub_state.clone(),
        reason,
        last_exit_code: unit.exec_main_status,
        suggested_action: suggested,
        main_pid: if unit.main_pid > 0 {
            Some(unit.main_pid)
        } else {
            None
        },
        last_failure,
    }
}

fn build_network_health(evidence: &EvidenceSnapshot) -> NetworkHealth {
    let probes = evidence.network_probes.as_ref();
    let dns_working = probes.map(|p| p.cluster_dns_reachable).unwrap_or(true);
    let dns_error = if dns_working {
        None
    } else {
        Some("cluster DNS unreachable from guest".into())
    };
    NetworkHealth {
        default_route: evidence.network.default_gateway.is_some()
            || !evidence.network.interfaces.is_empty(),
        dns_working,
        dns_error,
        interfaces_up: evidence.network.interfaces.len(),
        cluster_dns_reachable: probes.map(|p| p.cluster_dns_reachable).unwrap_or(false),
    }
}

fn build_storage_health(evidence: &EvidenceSnapshot) -> StorageHealth {
    let root_usage = read_root_usage_percent();
    let inode_usage = read_inode_usage_percent();
    let read_only = count_readonly_mounts();
    let pressure_io = evidence
        .process
        .as_ref()
        .and_then(|p| p.pressure_io.clone());

    StorageHealth {
        root_usage_percent: root_usage,
        inode_usage_percent: inode_usage,
        read_only_mounts: read_only,
        pressure_io,
    }
}

fn read_root_usage_percent() -> u8 {
    statvfs_usage("/").unwrap_or_else(|| df_usage("/"))
}

fn read_inode_usage_percent() -> u8 {
    statvfs_inode_usage("/").unwrap_or(0)
}

#[cfg(target_os = "windows")]
fn statvfs_usage(_path: &str) -> Option<u8> {
    None
}

#[cfg(target_os = "windows")]
fn statvfs_inode_usage(_path: &str) -> Option<u8> {
    None
}

#[cfg(not(target_os = "windows"))]
fn statvfs_usage(path: &str) -> Option<u8> {
    use std::ffi::CString;
    let c_path = CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    if stat.f_blocks == 0 {
        return None;
    }
    let used = stat.f_blocks.saturating_sub(stat.f_bfree);
    Some(((used as u64) * 100 / stat.f_blocks as u64) as u8)
}

#[cfg(not(target_os = "windows"))]
fn statvfs_inode_usage(path: &str) -> Option<u8> {
    use std::ffi::CString;
    let c_path = CString::new(path).ok()?;
    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    if unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) } != 0 {
        return None;
    }
    if stat.f_files == 0 {
        return None;
    }
    let used = stat.f_files.saturating_sub(stat.f_ffree);
    Some(((used as u64) * 100 / stat.f_files as u64) as u8)
}

fn df_usage(path: &str) -> u8 {
    std::process::Command::new("df")
        .args(["-P", path])
        .output()
        .ok()
        .and_then(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .nth(1)
                .map(String::from)
        })
        .and_then(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            parts
                .get(4)
                .and_then(|p| p.trim_end_matches('%').parse().ok())
        })
        .unwrap_or(0)
}

fn count_readonly_mounts() -> usize {
    std::fs::read_to_string("/proc/mounts")
        .map(|content| {
            content
                .lines()
                .filter(|l| {
                    let parts: Vec<&str> = l.split_whitespace().collect();
                    parts.len() >= 4 && parts[3].contains("ro") && !parts[3].contains("rw")
                })
                .count()
        })
        .unwrap_or(0)
}

fn build_recommendations(
    evidence: &EvidenceSnapshot,
    runtime: Option<&SystemdRuntimeInfo>,
    level: &HealthLevel,
    critical: &[CriticalService],
) -> Vec<Recommendation> {
    let mut out = Vec::new();

    if *level == HealthLevel::Degraded || *level == HealthLevel::Unhealthy {
        if let Some(rt) = runtime {
            for unit in rt
                .units
                .iter()
                .filter(|u| u.active_state == "failed")
                .take(3)
            {
                out.push(Recommendation {
                    priority: 1,
                    category: "systemd".into(),
                    title: format!("Fix failed unit {}", unit.name),
                    detail: unit.description.clone(),
                    action: format!("restart_unit:{}", unit.name),
                });
            }
        }
    }

    for svc in critical.iter().filter(|c| c.last_failure.is_some()).take(2) {
        out.push(Recommendation {
            priority: 1,
            category: "incident".into(),
            title: format!("Repeated failures on {}", svc.name),
            detail: svc.last_failure.clone().unwrap_or_default(),
            action: svc.suggested_action.clone(),
        });
    }

    let storage = build_storage_health(evidence);
    if storage.root_usage_percent >= 90 {
        out.push(Recommendation {
            priority: 2,
            category: "storage".into(),
            title: "Root disk nearly full".into(),
            detail: format!("Root usage at {}%", storage.root_usage_percent),
            action: "clean_disk".into(),
        });
    }

    if let Some(probes) = &evidence.network_probes {
        if !probes.cluster_dns_reachable {
            out.push(Recommendation {
                priority: 1,
                category: "network".into(),
                title: "DNS resolution broken".into(),
                detail: "Cluster DNS is unreachable from the guest".into(),
                action: "fix_dns".into(),
            });
        }
    }

    out
}

pub fn list_failed_units_from_evidence(evidence: &EvidenceSnapshot) -> Vec<SystemdRuntimeUnit> {
    evidence
        .systemd
        .as_ref()
        .and_then(|s| s.runtime.as_ref())
        .map(|rt| {
            rt.units
                .iter()
                .filter(|u| u.active_state == "failed")
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::snapshot::EvidenceSnapshot;

    #[test]
    fn guest_health_defaults_healthy_without_problems() {
        let evidence = EvidenceSnapshot {
            schema_version: 3,
            image_path: "live".into(),
            collected_at: "now".into(),
            root: "/".into(),
            os: crate::evidence::snapshot::OsEvidence {
                hostname: "test-vm".into(),
                ..Default::default()
            },
            storage: Default::default(),
            boot: Default::default(),
            network: Default::default(),
            packages: Default::default(),
            security: Default::default(),
            vm_tools: Default::default(),
            systemd: None,
            windows: None,
            kubevirt: None,
            cloud_init: None,
            network_probes: None,
            snapshot_readiness: None,
            process: None,
            hardware: None,
            linux_migration: None,
            online_cache: None,
        };
        let health = build_guest_health(&evidence);
        assert_eq!(health.vm_hostname, "test-vm");
        assert_eq!(health.failed_units, 0);
        assert!(health.score > 0);
    }
}
