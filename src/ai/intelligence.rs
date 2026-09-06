// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Bundled intelligence output for doctor / migrate-plan / export.

use crate::ai::platform::build_machina_export;
use crate::ai::{
    analyze_semantic, build_report_narrative, evaluate_cis_profile, generate_recommendations,
    ReportNarrative, SemanticAnalysis,
};
use crate::ai::{MachinaEvidenceExport, Recommendation, SecurityProfileReport};
use crate::assurance::MigrationBriefing;
use crate::boot::BootabilityReport;
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntelligenceBundle {
    pub semantic: SemanticAnalysis,
    pub recommendations: Vec<Recommendation>,
    pub narrative: ReportNarrative,
    pub security_profile: SecurityProfileReport,
    pub machina_export: MachinaEvidenceExport,
}

/// Build the full deterministic intelligence bundle for a snapshot.
pub fn build_intelligence(
    evidence: &EvidenceSnapshot,
    boot: Option<&BootabilityReport>,
    copilot: Option<MigrationBriefing>,
) -> IntelligenceBundle {
    let semantic = analyze_semantic(evidence);
    let recommendations = generate_recommendations(evidence, boot, &semantic);
    let narrative = build_report_narrative(evidence, &semantic, &recommendations, copilot.as_ref());
    let security_profile = evaluate_cis_profile(evidence, &semantic);
    let machina_export = build_machina_export(
        evidence,
        &semantic,
        &recommendations,
        &security_profile,
        copilot,
        boot.map(|b| b.score),
    );
    IntelligenceBundle {
        semantic,
        recommendations,
        narrative,
        security_profile,
        machina_export,
    }
}
