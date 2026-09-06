// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Phase 3 — AI narrative sections for HTML/Markdown/PDF reports.

use crate::ai::recommendations::{Recommendation, RecommendationCategory};
use crate::ai::semantic::SemanticAnalysis;
use crate::assurance::MigrationBriefing;
use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportNarrative {
    pub title: String,
    pub executive_summary: String,
    pub sections: Vec<NarrativeSection>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NarrativeSection {
    pub heading: String,
    pub body: String,
}

/// Build deterministic narrative blocks suitable for HTML/PDF export (LLM-free).
pub fn build_report_narrative(
    evidence: &EvidenceSnapshot,
    semantic: &SemanticAnalysis,
    recommendations: &[Recommendation],
    copilot: Option<&MigrationBriefing>,
) -> ReportNarrative {
    let os = if evidence.os.distribution.is_empty() {
        evidence.os.os_type.clone()
    } else {
        format!("{} {}", evidence.os.distribution, evidence.os.version)
    };

    let critical = recommendations
        .iter()
        .filter(|r| r.category == RecommendationCategory::Critical)
        .count();
    let executive = if let Some(c) = copilot {
        format!("{} — {}", c.headline, c.summary)
    } else {
        format!(
            "GuestKit analyzed {os} with {} critical finding(s), {} systemd problem hint(s), and {} sandbox review item(s).",
            critical,
            semantic.problem_units.len(),
            semantic.sandbox_scores.len()
        )
    };

    let mut sections = vec![
        NarrativeSection {
            heading: "System overview".into(),
            body: format!(
                "Hostname `{}`, bootloader {}, root FS {}. Packages: {}.",
                evidence.os.hostname,
                evidence.boot.bootloader,
                evidence.storage.root_filesystem,
                evidence.packages.count
            ),
        },
        NarrativeSection {
            heading: "Systemd intelligence".into(),
            body: format!(
                "{} timers, {} sockets, {} problem units. Dependency graph: {} nodes, {} edges.",
                semantic.timer_units.len(),
                semantic.socket_units.len(),
                semantic.problem_units.len(),
                semantic.dependency_graph.nodes.len(),
                semantic.dependency_graph.edges.len()
            ),
        },
    ];

    if !recommendations.is_empty() {
        let bullets: Vec<String> = recommendations
            .iter()
            .take(8)
            .map(|r| {
                format!(
                    "- [{}] {} ({})",
                    category_label(r.category),
                    r.title,
                    r.citation
                )
            })
            .collect();
        sections.push(NarrativeSection {
            heading: "Recommendations".into(),
            body: bullets.join("\n"),
        });
    }

    if let Some(windows) = &evidence.windows {
        sections.push(NarrativeSection {
            heading: "Windows services".into(),
            body: format!(
                "{} services, {} persistence run keys, {} scheduled tasks, {} event logs ({} bytes).",
                windows.services.len(),
                windows.persistence.run_keys.len(),
                windows.persistence.scheduled_tasks.len(),
                windows.event_logs.log_count,
                windows.event_logs.total_bytes
            ),
        });
    }

    ReportNarrative {
        title: format!("Guest Intelligence Report — {os}"),
        executive_summary: executive,
        sections,
    }
}

fn category_label(c: RecommendationCategory) -> &'static str {
    match c {
        RecommendationCategory::Critical => "Critical",
        RecommendationCategory::Security => "Security",
        RecommendationCategory::Migration => "Migration",
        RecommendationCategory::Performance => "Performance",
    }
}
