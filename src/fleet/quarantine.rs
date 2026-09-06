// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Split a fleet into shippable vs quarantined VMs.
//!
//! Quarantine reasons: boot score below threshold, analyzer migration
//! blockers, evidence-collection failures. hyper2kvm / h2kvmctl must
//! refuse quarantined members.

use super::report::{FleetAnalysisReport, FleetFailedVm, MigrationBlocker};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuarantineReason {
    LowScore,
    AnalyzerBlocker,
    CollectFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineMember {
    pub image: String,
    pub score: Option<f64>,
    pub reasons: Vec<QuarantineReason>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineReport {
    pub threshold: f64,
    pub total: usize,
    pub allowed: Vec<String>,
    pub quarantined: Vec<QuarantineMember>,
}

impl QuarantineReport {
    pub fn all_clear(&self) -> bool {
        self.quarantined.is_empty()
    }
}

/// Classify scored images + analyzer output + collect failures.
pub fn quarantine_fleet(
    scored: &[(String, f64)],
    analysis: &FleetAnalysisReport,
    failed: &[FleetFailedVm],
    threshold: f64,
) -> QuarantineReport {
    let mut quarantined = Vec::new();
    let mut allowed = Vec::new();

    for (image, score) in scored {
        let mut reasons = Vec::new();
        let mut detail = Vec::new();
        if *score < threshold {
            reasons.push(QuarantineReason::LowScore);
            detail.push(format!("score {score:.0} < {threshold:.0}"));
        }
        if let Some(b) = analysis
            .migration_blockers
            .iter()
            .find(|b| &b.image == image)
        {
            reasons.push(QuarantineReason::AnalyzerBlocker);
            detail.push(b.issue.clone());
        }
        if reasons.is_empty() {
            allowed.push(image.clone());
        } else {
            quarantined.push(QuarantineMember {
                image: image.clone(),
                score: Some(*score),
                reasons,
                detail: Some(detail.join("; ")),
            });
        }
    }

    for f in failed {
        quarantined.push(QuarantineMember {
            image: f.image.clone(),
            score: None,
            reasons: vec![QuarantineReason::CollectFailed],
            detail: Some(f.error.clone()),
        });
    }

    allowed.sort();
    quarantined.sort_by(|a, b| a.image.cmp(&b.image));

    QuarantineReport {
        threshold,
        total: scored.len() + failed.len(),
        allowed,
        quarantined,
    }
}

/// Helper for unit tests that only have scores.
pub fn quarantine_from_scores(scores: &[(&str, f64)], threshold: f64) -> QuarantineReport {
    let scored: Vec<(String, f64)> = scores.iter().map(|(n, s)| ((*n).to_string(), *s)).collect();
    let analysis = FleetAnalysisReport {
        total_vms: scores.len(),
        clusters: vec![],
        snowflakes: vec![],
        migration_blockers: scores
            .iter()
            .filter(|(_, s)| *s < 60.0)
            .map(|(n, s)| MigrationBlocker {
                image: (*n).to_string(),
                issue: format!("Low boot assurance score: {s:.0}%"),
                boot_score: *s,
            })
            .collect(),
        golden_image_candidates: vec![],
        failed_vms: vec![],
    };
    quarantine_fleet(&scored, &analysis, &[], threshold)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn splits_allow_and_quarantine() {
        let r = quarantine_from_scores(&[("good.qcow2", 91.0), ("bad.qcow2", 40.0)], 80.0);
        assert_eq!(r.allowed, vec!["good.qcow2".to_string()]);
        assert_eq!(r.quarantined.len(), 1);
        assert!(r.quarantined[0]
            .reasons
            .contains(&QuarantineReason::LowScore));
        assert!(!r.all_clear());
    }

    #[test]
    fn all_clear_when_everyone_clears_threshold() {
        let r = quarantine_from_scores(&[("a.qcow2", 88.0), ("b.qcow2", 95.0)], 80.0);
        assert!(r.all_clear());
        assert_eq!(r.allowed.len(), 2);
    }
}
