// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 3 — what-if boot score simulator.

use crate::ai::semantic::SemanticAnalysis;
use crate::boot::{analyze_bootability, BootTarget};
use crate::evidence::snapshot::{EvidenceSnapshot, SystemdUnitState};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WhatIfResult {
    pub unit: String,
    pub action: String,
    pub baseline_score: f64,
    pub projected_score: f64,
    pub delta: f64,
    pub notes: Vec<String>,
}

/// Simulate disabling a systemd unit and estimate boot score impact.
pub fn simulate_unit_disable(
    evidence: &EvidenceSnapshot,
    unit_name: &str,
    target: BootTarget,
) -> WhatIfResult {
    let baseline = analyze_bootability(evidence, target);
    let mut modified = evidence.clone();
    let mut notes = Vec::new();

    if let Some(systemd) = modified.systemd.as_mut() {
        if let Some(unit) = systemd.units.iter_mut().find(|u| u.name == unit_name) {
            if unit.state == SystemdUnitState::Enabled {
                unit.state = SystemdUnitState::Disabled;
                notes.push(format!("Disabled {unit_name} for simulation"));
            } else {
                notes.push(format!("{unit_name} was already {:?}", unit.state));
            }
        } else {
            notes.push(format!("Unit {unit_name} not found in evidence"));
        }
        systemd.problem_hints.retain(|h| h.unit != unit_name);
    }

    let projected = analyze_bootability(&modified, target);
    WhatIfResult {
        unit: unit_name.to_string(),
        action: "disable".into(),
        baseline_score: baseline.score,
        projected_score: projected.score,
        delta: projected.score - baseline.score,
        notes,
    }
}

/// Rank units whose disable would most improve boot score (heuristic).
pub fn rank_disable_candidates(
    evidence: &EvidenceSnapshot,
    semantic: &SemanticAnalysis,
    target: BootTarget,
    limit: usize,
) -> Vec<WhatIfResult> {
    let mut results = Vec::new();
    for hint in semantic.problem_units.iter().take(limit * 2) {
        results.push(simulate_unit_disable(evidence, &hint.name, target));
    }
    results.sort_by(|a, b| {
        b.delta
            .partial_cmp(&a.delta)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    results.truncate(limit);
    results
}
