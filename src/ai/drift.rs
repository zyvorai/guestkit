// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 3 — fleet semantic drift explanations.

use crate::ai::semantic::{analyze_semantic, SemanticAnalysis};
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetDriftReport {
    pub summary: String,
    pub os_drift: Vec<String>,
    pub systemd_drift: Vec<String>,
    pub security_drift: Vec<String>,
    pub package_drift: Vec<String>,
}

/// Explain semantic drift between two evidence snapshots (golden vs current).
pub fn explain_fleet_drift(
    baseline: &EvidenceSnapshot,
    current: &EvidenceSnapshot,
) -> FleetDriftReport {
    let base_sem = analyze_semantic(baseline);
    let cur_sem = analyze_semantic(current);

    let mut os_drift = Vec::new();
    if baseline.os.distribution != current.os.distribution
        || baseline.os.version != current.os.version
    {
        os_drift.push(format!(
            "OS changed: {} {} → {} {}",
            baseline.os.distribution,
            baseline.os.version,
            current.os.distribution,
            current.os.version
        ));
    }

    let systemd_drift = diff_systemd(&base_sem, &cur_sem);
    let mut security_drift = Vec::new();
    if baseline.security.selinux != current.security.selinux {
        security_drift.push(format!(
            "SELinux: {} → {}",
            baseline.security.selinux, current.security.selinux
        ));
    }
    if baseline.security.ssh_root_login != current.security.ssh_root_login {
        security_drift.push("SSH root login setting changed".into());
    }

    let mut package_drift = Vec::new();
    let delta = current.packages.count as i64 - baseline.packages.count as i64;
    if delta.abs() > 10 {
        package_drift.push(format!("Package count delta: {delta:+}"));
    }

    let summary = format!(
        "{} OS change(s), {} systemd drift item(s), {} security drift item(s)",
        os_drift.len(),
        systemd_drift.len(),
        security_drift.len()
    );

    FleetDriftReport {
        summary,
        os_drift,
        systemd_drift,
        security_drift,
        package_drift,
    }
}

fn diff_systemd(base: &SemanticAnalysis, current: &SemanticAnalysis) -> Vec<String> {
    let base_problems: std::collections::HashSet<_> =
        base.problem_units.iter().map(|u| u.name.clone()).collect();
    let cur_problems: std::collections::HashSet<_> = current
        .problem_units
        .iter()
        .map(|u| u.name.clone())
        .collect();

    let mut drift = Vec::new();
    for u in cur_problems.difference(&base_problems) {
        drift.push(format!("New systemd problem: {u}"));
    }
    for u in base_problems.difference(&cur_problems) {
        drift.push(format!("Resolved systemd problem: {u}"));
    }
    if current.timer_units.len() != base.timer_units.len() {
        drift.push(format!(
            "Timer units: {} → {}",
            base.timer_units.len(),
            current.timer_units.len()
        ));
    }
    drift
}
