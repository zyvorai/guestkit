// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 3 — proactive recommendations engine.

use crate::ai::semantic::SemanticAnalysis;
use crate::boot::BootabilityReport;
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecommendationCategory {
    Critical,
    Security,
    Migration,
    Performance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Recommendation {
    pub category: RecommendationCategory,
    pub title: String,
    pub detail: String,
    pub citation: String,
    pub confidence: f32,
}

/// Generate deterministic proactive recommendations from evidence + semantic analysis.
pub fn generate_recommendations(
    evidence: &EvidenceSnapshot,
    boot: Option<&BootabilityReport>,
    semantic: &SemanticAnalysis,
) -> Vec<Recommendation> {
    let mut recs = Vec::new();

    if let Some(boot) = boot {
        for blocker in &boot.blockers {
            recs.push(Recommendation {
                category: RecommendationCategory::Critical,
                title: blocker.title.clone(),
                detail: blocker.message.clone(),
                citation: format!("boot.blockers:{}", blocker.check_id),
                confidence: 0.95,
            });
        }
        for warning in &boot.warnings {
            recs.push(Recommendation {
                category: RecommendationCategory::Migration,
                title: warning.title.clone(),
                detail: warning.message.clone(),
                citation: format!("boot.warnings:{}", warning.check_id),
                confidence: 0.85,
            });
        }
    }

    for hint in semantic.problem_units.iter().take(10) {
        recs.push(Recommendation {
            category: RecommendationCategory::Migration,
            title: format!("Systemd issue: {}", hint.name),
            detail: hint.description.clone().unwrap_or_default(),
            citation: hint.path.clone(),
            confidence: 0.9,
        });
    }

    for score in semantic
        .sandbox_scores
        .iter()
        .filter(|s| s.score < 25)
        .take(8)
    {
        recs.push(Recommendation {
            category: RecommendationCategory::Security,
            title: format!("Harden {}", score.unit),
            detail: format!(
                "Sandbox score {}/100 — runs as root: {}",
                score.score, score.runs_as_root
            ),
            citation: score.path.clone(),
            confidence: 0.88,
        });
    }

    for risk in &semantic.windows_risks {
        let cat = if risk.severity == "critical" {
            RecommendationCategory::Critical
        } else if risk.severity == "warning" {
            RecommendationCategory::Security
        } else {
            RecommendationCategory::Performance
        };
        recs.push(Recommendation {
            category: cat,
            title: risk.name.clone(),
            detail: risk.message.clone(),
            citation: format!("windows.services:{}", risk.code),
            confidence: 0.87,
        });
    }

    if evidence.security.pending_security_updates {
        recs.push(Recommendation {
            category: RecommendationCategory::Security,
            title: "Pending security updates".into(),
            detail: "Package manager reports pending security patches before migration.".into(),
            citation: "security.pending_security_updates".into(),
            confidence: 0.8,
        });
    }

    if !evidence
        .boot
        .loaded_modules
        .iter()
        .any(|m| m.contains("virtio"))
    {
        recs.push(Recommendation {
            category: RecommendationCategory::Performance,
            title: "Missing virtio modules in initramfs".into(),
            detail: "Regenerate initramfs with virtio_blk/virtio_net for cloud/KubeVirt targets."
                .into(),
            citation: "boot.loaded_modules".into(),
            confidence: 0.82,
        });
    }

    recs.sort_by(|a, b| {
        category_rank(a.category)
            .cmp(&category_rank(b.category))
            .then(
                b.confidence
                    .partial_cmp(&a.confidence)
                    .unwrap_or(std::cmp::Ordering::Equal),
            )
    });
    recs
}

fn category_rank(c: RecommendationCategory) -> u8 {
    match c {
        RecommendationCategory::Critical => 0,
        RecommendationCategory::Security => 1,
        RecommendationCategory::Migration => 2,
        RecommendationCategory::Performance => 3,
    }
}
