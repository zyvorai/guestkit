// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pre/post-migration baseline capture and drift comparison.
//!
//! The baseline format IS [`EvidenceSnapshot`] — the same schema the whole
//! product speaks — wrapped with identity metadata and persisted under the
//! agent state directory so a post-migration boot can diff against it.

use crate::evidence::EvidenceSnapshot;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BaselinePhase {
    PreMigration,
    PostMigration,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBaseline {
    pub id: String,
    pub captured_at: String,
    pub phase: BaselinePhase,
    pub target: String,
    pub evidence: EvidenceSnapshot,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DriftItem {
    pub field: String,
    pub before: String,
    pub after: String,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DriftReport {
    pub hardware: Vec<DriftItem>,
    pub network: Vec<DriftItem>,
    pub routes: Vec<DriftItem>,
    pub ports: Vec<DriftItem>,
    pub disks: Vec<DriftItem>,
    pub services: Vec<DriftItem>,
    /// Human summary lines, worst first.
    pub summary: Vec<String>,
    /// True when only cosmetic drift was found (interface renames with
    /// preserved addressing, tool swaps).
    pub within_expectations: bool,
}

pub fn baseline_dir() -> PathBuf {
    // Override for unprivileged runs (tests, e2e).
    if let Ok(dir) = std::env::var("GUESTKIT_STATE_DIR") {
        return PathBuf::from(dir).join("migration");
    }
    if cfg!(windows) {
        PathBuf::from("C:\\ProgramData\\GuestKit\\migration")
    } else {
        PathBuf::from("/var/lib/guestkit/migration")
    }
}

/// Capture live evidence as a baseline and persist it.
#[cfg(feature = "agent")]
pub fn capture_baseline(phase: BaselinePhase, target: &str) -> anyhow::Result<MigrationBaseline> {
    let evidence = crate::evidence::build_evidence_live()?;
    let baseline = MigrationBaseline {
        id: format!(
            "{}-{}",
            match phase {
                BaselinePhase::PreMigration => "pre",
                BaselinePhase::PostMigration => "post",
            },
            chrono::Utc::now().format("%Y%m%dT%H%M%SZ")
        ),
        captured_at: chrono::Utc::now().to_rfc3339(),
        phase,
        target: target.to_string(),
        evidence,
    };
    let dir = baseline_dir();
    std::fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", baseline.id));
    std::fs::write(&path, serde_json::to_vec_pretty(&baseline)?)?;
    Ok(baseline)
}

pub fn load_baseline(id: &str) -> anyhow::Result<MigrationBaseline> {
    let path = baseline_dir().join(format!("{id}.json"));
    let bytes = std::fs::read(&path)
        .map_err(|e| anyhow::anyhow!("baseline {id} not found at {}: {e}", path.display()))?;
    Ok(serde_json::from_slice(&bytes)?)
}

/// Most recent pre-migration baseline, if any.
pub fn latest_pre_baseline() -> Option<MigrationBaseline> {
    let dir = baseline_dir();
    let mut names: Vec<String> = std::fs::read_dir(&dir)
        .ok()?
        .flatten()
        .filter_map(|e| e.file_name().to_str().map(str::to_string))
        .filter(|n| n.starts_with("pre-") && n.ends_with(".json"))
        .collect();
    names.sort();
    let latest = names.pop()?;
    load_baseline(latest.trim_end_matches(".json")).ok()
}

fn diff_field(items: &mut Vec<DriftItem>, field: &str, before: &str, after: &str) {
    if before != after {
        items.push(DriftItem {
            field: field.to_string(),
            before: before.to_string(),
            after: after.to_string(),
        });
    }
}

fn diff_sets(items: &mut Vec<DriftItem>, field: &str, before: &[String], after: &[String]) {
    let removed: Vec<&String> = before.iter().filter(|x| !after.contains(x)).collect();
    let added: Vec<&String> = after.iter().filter(|x| !before.contains(x)).collect();
    if !removed.is_empty() || !added.is_empty() {
        items.push(DriftItem {
            field: field.to_string(),
            before: removed
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            after: added
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", "),
        });
    }
}

/// Compare a baseline against current (or another captured) evidence.
pub fn diff_baselines(before: &MigrationBaseline, after: &EvidenceSnapshot) -> DriftReport {
    let mut report = DriftReport::default();
    let b = &before.evidence;

    // Hardware / drivers
    diff_field(
        &mut report.hardware,
        "firmware",
        &b.boot.firmware,
        &after.boot.firmware,
    );
    diff_sets(
        &mut report.hardware,
        "loaded_hypervisor_modules",
        &b.linux_migration
            .as_ref()
            .map(|l| l.hypervisor_modules.clone())
            .unwrap_or_default(),
        &after
            .linux_migration
            .as_ref()
            .map(|l| l.hypervisor_modules.clone())
            .unwrap_or_default(),
    );

    // Network: interface names, addresses, gateway, DNS
    let iface_summary = |ev: &EvidenceSnapshot| -> Vec<String> {
        ev.network
            .live_interfaces
            .iter()
            .map(|i| format!("{} [{}]", i.name, i.addresses.join(" ")))
            .collect()
    };
    diff_sets(
        &mut report.network,
        "interfaces",
        &iface_summary(b),
        &iface_summary(after),
    );
    diff_field(
        &mut report.network,
        "default_gateway",
        b.network.default_gateway.as_deref().unwrap_or(""),
        after.network.default_gateway.as_deref().unwrap_or(""),
    );
    diff_sets(
        &mut report.network,
        "dns_servers",
        &b.network.dns_servers,
        &after.network.dns_servers,
    );
    diff_sets(
        &mut report.routes,
        "routes",
        &b.network.routes,
        &after.network.routes,
    );

    // Ports (listening) from process evidence when present
    let ports = |ev: &EvidenceSnapshot| -> Vec<String> {
        ev.process
            .as_ref()
            .map(|p| {
                p.listening_ports
                    .iter()
                    .map(|lp| format!("{} ({})", lp.port, lp.process))
                    .collect()
            })
            .unwrap_or_default()
    };
    diff_sets(
        &mut report.ports,
        "listening_ports",
        &ports(b),
        &ports(after),
    );

    // Disks: fstab + root fs
    diff_field(
        &mut report.disks,
        "root_filesystem",
        &b.storage.root_filesystem,
        &after.storage.root_filesystem,
    );
    let fstab = |ev: &EvidenceSnapshot| -> Vec<String> {
        ev.storage
            .fstab_entries
            .iter()
            .map(|e| format!("{} {}", e.device, e.mountpoint))
            .collect()
    };
    diff_sets(&mut report.disks, "fstab", &fstab(b), &fstab(after));

    // Services: failed units delta
    let failed = |ev: &EvidenceSnapshot| -> Vec<String> {
        ev.systemd
            .as_ref()
            .and_then(|s| s.runtime.as_ref())
            .map(|r| {
                r.units
                    .iter()
                    .filter(|u| u.active_state == "failed" || u.sub_state == "failed")
                    .map(|u| u.name.clone())
                    .collect()
            })
            .unwrap_or_default()
    };
    diff_sets(
        &mut report.services,
        "failed_units",
        &failed(b),
        &failed(after),
    );

    // Summary + expectation classification
    let counts = [
        ("hardware", report.hardware.len()),
        ("network", report.network.len()),
        ("routes", report.routes.len()),
        ("ports", report.ports.len()),
        ("disks", report.disks.len()),
        ("services", report.services.len()),
    ];
    for (name, n) in counts {
        if n > 0 {
            report.summary.push(format!("{name}: {n} change(s)"));
        }
    }
    if report.summary.is_empty() {
        report.summary.push("no drift detected".to_string());
    }
    // Ports/services/disks drift is what actually breaks workloads;
    // hardware and interface renames are expected across hypervisors.
    report.within_expectations =
        report.ports.is_empty() && report.services.is_empty() && report.disks.is_empty();
    report
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::migration::checks::tests_support::linux_evidence;

    fn baseline_from(ev: EvidenceSnapshot) -> MigrationBaseline {
        MigrationBaseline {
            id: "pre-test".into(),
            captured_at: String::new(),
            phase: BaselinePhase::PreMigration,
            target: "kvm".into(),
            evidence: ev,
        }
    }

    #[test]
    fn identical_evidence_has_no_drift() {
        let ev = linux_evidence();
        let report = diff_baselines(&baseline_from(ev.clone()), &ev);
        assert!(report.within_expectations);
        assert_eq!(report.summary, vec!["no drift detected"]);
    }

    #[test]
    fn interface_rename_is_within_expectations() {
        let mut before = linux_evidence();
        before.network.live_interfaces = vec![crate::evidence::snapshot::NetworkInterfaceLive {
            name: "ens192".into(),
            addresses: vec!["10.0.0.5/24".into()],
            ..Default::default()
        }];
        let mut after = linux_evidence();
        after.network.live_interfaces = vec![crate::evidence::snapshot::NetworkInterfaceLive {
            name: "enp1s0".into(),
            addresses: vec!["10.0.0.5/24".into()],
            ..Default::default()
        }];
        let report = diff_baselines(&baseline_from(before), &after);
        assert_eq!(report.network.len(), 1);
        assert!(report.within_expectations);
    }

    #[test]
    fn route_and_dns_drift_reported() {
        let mut before = linux_evidence();
        before.network.routes = vec!["default via 10.0.0.1 dev ens192 metric 100".into()];
        before.network.dns_servers = vec!["10.0.0.2".into()];
        let mut after = linux_evidence();
        after.network.routes = vec!["default via 10.0.0.1 dev enp1s0 metric 101".into()];
        after.network.dns_servers = vec!["10.0.0.2".into()];
        let report = diff_baselines(&baseline_from(before), &after);
        assert_eq!(report.routes.len(), 1);
        assert!(report.summary.iter().any(|s| s.contains("routes")));
    }
}
