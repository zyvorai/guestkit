// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 1 — semantic analysis over evidence snapshots.

use crate::evidence::snapshot::{
    EvidenceSnapshot, SystemdUnit, SystemdUnitState, WindowsServiceEntry, WindowsStartType,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

/// Semantic analysis derived from an evidence snapshot (deterministic, no LLM).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SemanticAnalysis {
    pub dependency_graph: DependencyGraph,
    pub sandbox_scores: Vec<SandboxScore>,
    pub windows_risks: Vec<WindowsServiceRisk>,
    pub failed_units: Vec<UnitSummary>,
    pub timer_units: Vec<UnitSummary>,
    pub socket_units: Vec<UnitSummary>,
    pub problem_units: Vec<UnitSummary>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub journal_hints: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub runtime_failed_units: Vec<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub nodes: Vec<String>,
    pub edges: Vec<DependencyEdge>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyEdge {
    pub from: String,
    pub to: String,
    pub relation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SandboxScore {
    pub unit: String,
    pub score: u8,
    pub runs_as_root: bool,
    pub flags: Vec<String>,
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WindowsServiceRisk {
    pub name: String,
    pub code: String,
    pub severity: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UnitSummary {
    pub name: String,
    pub unit_type: String,
    pub state: String,
    pub description: Option<String>,
    pub path: String,
}

const CRITICAL_WINDOWS_SERVICES: &[&str] = &[
    "EventLog", "RpcSs", "Dhcp", "Dnscache", "Schedule", "Winmgmt", "wuauserv",
];

/// Run Phase 1 semantic analysis on a collected evidence snapshot.
pub fn analyze_semantic(evidence: &EvidenceSnapshot) -> SemanticAnalysis {
    let mut out = SemanticAnalysis::default();

    if let Some(systemd) = &evidence.systemd {
        out.dependency_graph = build_dependency_graph(&systemd.units);
        out.sandbox_scores = score_sandboxing(&systemd.units);
        for unit in &systemd.units {
            let summary = unit_summary(unit);
            match unit.unit_type.as_str() {
                "timer" => out.timer_units.push(summary.clone()),
                "socket" => out.socket_units.push(summary.clone()),
                _ => {}
            }
            if unit.state == SystemdUnitState::Masked {
                out.failed_units.push(summary);
            }
        }
        for hint in &systemd.problem_hints {
            let sev = match hint.severity {
                crate::evidence::snapshot::SystemdProblemSeverity::Info => "info",
                crate::evidence::snapshot::SystemdProblemSeverity::Warning => "warning",
                crate::evidence::snapshot::SystemdProblemSeverity::Critical => "critical",
            };
            out.problem_units.push(UnitSummary {
                name: hint.unit.clone(),
                unit_type: "service".into(),
                state: sev.into(),
                description: Some(hint.message.clone()),
                path: hint.path.clone(),
            });
        }

        if let Some(runtime) = &systemd.runtime {
            for unit in runtime.units.iter().filter(|u| u.active_state == "failed") {
                if out.runtime_failed_units.iter().any(|n| n == &unit.name) {
                    continue;
                }
                out.runtime_failed_units.push(unit.name.clone());
                if !out.problem_units.iter().any(|p| p.name == unit.name) {
                    out.problem_units.push(UnitSummary {
                        name: unit.name.clone(),
                        unit_type: "service".into(),
                        state: "failed".into(),
                        description: Some(unit.description.clone()),
                        path: unit.fragment_path.clone(),
                    });
                }
            }
        }
    }

    #[cfg(feature = "agent")]
    enrich_journal_hints(&mut out);

    if let Some(windows) = &evidence.windows {
        out.windows_risks = analyze_windows_risks(&windows.services);
    }

    out
}

fn unit_summary(unit: &SystemdUnit) -> UnitSummary {
    UnitSummary {
        name: unit.name.clone(),
        unit_type: unit.unit_type.clone(),
        state: format!("{:?}", unit.state).to_lowercase(),
        description: unit.description.clone(),
        path: unit.path.clone(),
    }
}

fn build_dependency_graph(units: &[SystemdUnit]) -> DependencyGraph {
    let mut nodes: HashSet<String> = HashSet::new();
    let mut edges = Vec::new();

    for unit in units {
        nodes.insert(unit.name.clone());
        for dep in &unit.after {
            let target = normalize_dep(dep);
            nodes.insert(target.clone());
            edges.push(DependencyEdge {
                from: unit.name.clone(),
                to: target,
                relation: "after".into(),
            });
        }
        for dep in &unit.before {
            let target = normalize_dep(dep);
            nodes.insert(target.clone());
            edges.push(DependencyEdge {
                from: target,
                to: unit.name.clone(),
                relation: "before".into(),
            });
        }
        for dep in &unit.requires {
            let target = normalize_dep(dep);
            nodes.insert(target.clone());
            edges.push(DependencyEdge {
                from: unit.name.clone(),
                to: target,
                relation: "requires".into(),
            });
        }
        for dep in &unit.wants {
            let target = normalize_dep(dep);
            nodes.insert(target.clone());
            edges.push(DependencyEdge {
                from: unit.name.clone(),
                to: target,
                relation: "wants".into(),
            });
        }
    }

    DependencyGraph {
        nodes: nodes.into_iter().collect(),
        edges,
    }
}

fn normalize_dep(dep: &str) -> String {
    let d = dep.trim();
    if d.ends_with(".service") || d.ends_with(".target") || d.ends_with(".socket") {
        d.to_string()
    } else {
        format!("{d}.service")
    }
}

fn score_sandboxing(units: &[SystemdUnit]) -> Vec<SandboxScore> {
    let mut scores = Vec::new();
    for unit in units {
        if unit.unit_type != "service" || unit.state != SystemdUnitState::Enabled {
            continue;
        }
        let runs_as_root = unit
            .user
            .as_deref()
            .is_none_or(|u| u.is_empty() || u == "root");
        let mut flags = Vec::new();
        let mut points = 0u8;
        for section in &unit.sections {
            if section.name != "Service" {
                continue;
            }
            for (k, v) in &section.keys {
                let key = k.as_str();
                if (key.starts_with("Protect")
                    || key.starts_with("Private")
                    || key == "NoNewPrivileges"
                    || key == "CapabilityBoundingSet"
                    || key == "SystemCallFilter"
                    || key == "RestrictNamespaces")
                    && v != "false"
                    && !v.is_empty()
                {
                    flags.push(format!("{k}={v}"));
                    points = points.saturating_add(8);
                }
            }
        }
        if !runs_as_root {
            points = points.saturating_add(25);
        }
        let score = points.min(100);
        if runs_as_root || score < 50 {
            scores.push(SandboxScore {
                unit: unit.name.clone(),
                score,
                runs_as_root,
                flags,
                path: unit.path.clone(),
            });
        }
    }
    scores.sort_by_key(|s| s.score);
    scores
}

fn analyze_windows_risks(services: &[WindowsServiceEntry]) -> Vec<WindowsServiceRisk> {
    let mut risks = Vec::new();
    let by_name: HashMap<&str, &WindowsServiceEntry> =
        services.iter().map(|s| (s.name.as_str(), s)).collect();

    for svc in services {
        let localsystem = svc
            .object_name
            .as_deref()
            .is_some_and(|o| o.eq_ignore_ascii_case("localsystem") || o.contains("LocalSystem"));
        if localsystem && svc.auto_start {
            risks.push(WindowsServiceRisk {
                name: svc.name.clone(),
                code: "localsystem-autostart".into(),
                severity: "warning".into(),
                message: format!(
                    "Service {} auto-starts as LocalSystem — review privileges",
                    svc.name
                ),
            });
        }
    }

    for critical in CRITICAL_WINDOWS_SERVICES {
        if let Some(svc) = by_name.get(critical) {
            if svc.start_type == WindowsStartType::Disabled {
                risks.push(WindowsServiceRisk {
                    name: svc.name.clone(),
                    code: "critical-disabled".into(),
                    severity: "critical".into(),
                    message: format!("Critical service {critical} is disabled"),
                });
            }
        }
    }

    risks
}

#[cfg(feature = "agent")]
fn enrich_journal_hints(out: &mut SemanticAnalysis) {
    for unit in out.runtime_failed_units.iter().take(3) {
        let slice = crate::journal::live::collect_journal_slice(unit, 15);
        for pattern in slice.top_patterns.iter().take(2) {
            let hint = format!("{unit}: {pattern}");
            if !out.journal_hints.contains(&hint) {
                out.journal_hints.push(hint);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_dependency_edges() {
        let units = vec![SystemdUnit {
            name: "app.service".into(),
            unit_type: "service".into(),
            after: vec!["network.target".into()],
            requires: vec!["db.service".into()],
            ..Default::default()
        }];
        let graph = build_dependency_graph(&units);
        assert!(graph.edges.iter().any(|e| e.relation == "after"));
        assert!(graph.edges.iter().any(|e| e.relation == "requires"));
    }

    #[test]
    fn sandbox_score_penalizes_root_without_protect() {
        let units = vec![SystemdUnit {
            name: "rootapp.service".into(),
            unit_type: "service".into(),
            state: SystemdUnitState::Enabled,
            path: "/etc/systemd/system/rootapp.service".into(),
            ..Default::default()
        }];
        let scores = score_sandboxing(&units);
        assert_eq!(scores.len(), 1);
        assert!(scores[0].runs_as_root);
        assert!(scores[0].score < 30);
    }
}
