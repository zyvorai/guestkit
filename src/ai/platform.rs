// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 4 — platform integration exports (Machina dashboard, policy DSL hints).

use crate::ai::recommendations::Recommendation;
use crate::ai::security_profiles::SecurityProfileReport;
use crate::ai::semantic::SemanticAnalysis;
use crate::assurance::MigrationBriefing;
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

/// Export bundle for external dashboards (Machina, Zeus OS).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachinaEvidenceExport {
    pub schema_version: u32,
    pub image_path: String,
    pub collected_at: String,
    pub summary: PlatformSummary,
    pub semantic: SemanticAnalysis,
    pub recommendations: Vec<Recommendation>,
    pub security_profile: SecurityProfileReport,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub copilot: Option<MigrationBriefing>,
    pub policy_hints: Vec<PolicyHint>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlatformSummary {
    pub os: String,
    pub boot_score: Option<f64>,
    pub migration_readiness: String,
    pub critical_count: usize,
    pub security_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyHint {
    pub rule_id: String,
    pub expression: String,
    pub rationale: String,
}

/// Build a Machina-ready export from evidence and derived analysis.
pub fn build_machina_export(
    evidence: &EvidenceSnapshot,
    semantic: &SemanticAnalysis,
    recommendations: &[Recommendation],
    security_profile: &SecurityProfileReport,
    copilot: Option<MigrationBriefing>,
    boot_score: Option<f64>,
) -> MachinaEvidenceExport {
    let critical_count = recommendations
        .iter()
        .filter(|r| matches!(r.category, crate::ai::RecommendationCategory::Critical))
        .count();
    let security_count = recommendations
        .iter()
        .filter(|r| matches!(r.category, crate::ai::RecommendationCategory::Security))
        .count();

    let readiness = if critical_count > 0 {
        "blocked"
    } else if security_count > 0 {
        "caution"
    } else {
        "ready"
    };

    let policy_hints = build_policy_hints(evidence, semantic, security_profile);

    MachinaEvidenceExport {
        schema_version: evidence.schema_version,
        image_path: evidence.image_path.clone(),
        collected_at: evidence.collected_at.clone(),
        summary: PlatformSummary {
            os: format!("{} {}", evidence.os.distribution, evidence.os.version),
            boot_score,
            migration_readiness: readiness.into(),
            critical_count,
            security_count,
        },
        semantic: semantic.clone(),
        recommendations: recommendations.to_vec(),
        security_profile: security_profile.clone(),
        copilot,
        policy_hints,
    }
}

fn build_policy_hints(
    evidence: &EvidenceSnapshot,
    semantic: &SemanticAnalysis,
    profile: &SecurityProfileReport,
) -> Vec<PolicyHint> {
    let mut hints = Vec::new();
    for fail in &profile.failed {
        hints.push(PolicyHint {
            rule_id: fail.id.clone(),
            expression: format!("guestkit.security.{} == pass", fail.id.to_lowercase()),
            rationale: fail.detail.clone(),
        });
    }
    if evidence.security.ssh_root_login == Some(true) {
        hints.push(PolicyHint {
            rule_id: "ssh-no-root".into(),
            expression: "evidence.security.ssh_root_login == false".into(),
            rationale: "Disallow root SSH login before fleet migration".into(),
        });
    }
    for score in semantic
        .sandbox_scores
        .iter()
        .filter(|s| s.score < 15)
        .take(5)
    {
        hints.push(PolicyHint {
            rule_id: format!("sandbox-{}", score.unit),
            expression: format!("systemd.unit('{}').sandbox_score >= 25", score.unit),
            rationale: format!("Harden {} (score {})", score.unit, score.score),
        });
    }
    hints
}
