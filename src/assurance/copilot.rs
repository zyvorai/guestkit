// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration Copilot — deterministic AI-style briefings from evidence + boot analysis.

use crate::boot::BootabilityReport;
use crate::cli::migrate::plan::MigrationScoreReport;
use crate::evidence::EvidenceSnapshot;
use crate::inference::RootCauseReport;
use serde::{Deserialize, Serialize};

/// Condensed disk intelligence for UI / copilot context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceDigest {
    pub os: String,
    pub architecture: String,
    pub bootloader: String,
    pub root_filesystem: String,
    pub kernel_count: usize,
    pub fstab_entries: usize,
    pub virtio_modules_loaded: bool,
    pub vm_tools: Vec<String>,
    pub selinux: String,
}

/// Human-readable evidence snippet linked to root-cause refs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvidenceHighlight {
    pub r#ref: String,
    pub label: String,
    pub detail: String,
}

/// Prioritized migration action recommendation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotAction {
    pub priority: u8,
    pub title: String,
    pub detail: String,
    pub workflow: String,
}

/// Pre-computed Q&A pair for the copilot chat UI.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CopilotInsight {
    pub id: String,
    pub question: String,
    pub answer: String,
}

/// AI-style migration briefing synthesized from deterministic analysis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationBriefing {
    pub readiness: String,
    pub headline: String,
    pub summary: String,
    pub boot_score: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub migration_score: Option<f64>,
    pub blocker_count: usize,
    pub warning_count: usize,
    pub evidence_digest: EvidenceDigest,
    pub evidence_highlights: Vec<EvidenceHighlight>,
    pub recommended_actions: Vec<CopilotAction>,
    pub insights: Vec<CopilotInsight>,
    pub next_workflow: String,
}

pub fn build_evidence_digest(evidence: &EvidenceSnapshot) -> EvidenceDigest {
    let os = if evidence.os.distribution.is_empty() {
        evidence.os.os_type.clone()
    } else {
        format!("{} {}", evidence.os.distribution, evidence.os.version)
            .trim()
            .to_string()
    };

    let virtio_modules_loaded = evidence
        .boot
        .loaded_modules
        .iter()
        .any(|m| m.contains("virtio"));

    EvidenceDigest {
        os,
        architecture: evidence.os.architecture.clone(),
        bootloader: evidence.boot.bootloader.clone(),
        root_filesystem: evidence.storage.root_filesystem.clone(),
        kernel_count: evidence.packages.kernels.len(),
        fstab_entries: evidence.storage.fstab_entries.len(),
        virtio_modules_loaded,
        vm_tools: evidence.vm_tools.detected.clone(),
        selinux: evidence.security.selinux.clone(),
    }
}

pub fn resolve_evidence_highlights(
    evidence: &EvidenceSnapshot,
    root_cause: Option<&RootCauseReport>,
) -> Vec<EvidenceHighlight> {
    let mut out = Vec::new();
    let refs: Vec<String> = root_cause
        .map(|r| r.evidence_refs.clone())
        .unwrap_or_default();

    for r in refs {
        if let Some(h) = highlight_for_ref(&r, evidence) {
            out.push(h);
        }
    }

    if out.is_empty() {
        if !evidence.storage.fstab_entries.is_empty() {
            let first = &evidence.storage.fstab_entries[0];
            out.push(EvidenceHighlight {
                r#ref: "storage.fstab_entries".into(),
                label: "Root mount".into(),
                detail: format!("{} → {} ({})", first.device, first.mountpoint, first.fstype),
            });
        }
        if !evidence.boot.bootloader.is_empty() {
            out.push(EvidenceHighlight {
                r#ref: "boot.bootloader".into(),
                label: "Bootloader".into(),
                detail: evidence.boot.bootloader.clone(),
            });
        }
    }

    out.truncate(6);
    out
}

fn highlight_for_ref(r: &str, evidence: &EvidenceSnapshot) -> Option<EvidenceHighlight> {
    match r {
        "storage.fstab_entries" => {
            let lines: Vec<String> = evidence
                .storage
                .fstab_entries
                .iter()
                .take(3)
                .map(|e| format!("{} → {}", e.device, e.mountpoint))
                .collect();
            Some(EvidenceHighlight {
                r#ref: r.into(),
                label: "Fstab".into(),
                detail: if lines.is_empty() {
                    "No fstab entries collected".into()
                } else {
                    lines.join("; ")
                },
            })
        }
        "boot.initramfs_paths" => Some(EvidenceHighlight {
            r#ref: r.into(),
            label: "Initramfs".into(),
            detail: if evidence.boot.initramfs_paths.is_empty() {
                "No initramfs images found".into()
            } else {
                evidence.boot.initramfs_paths.join(", ")
            },
        }),
        "boot.loaded_modules" => Some(EvidenceHighlight {
            r#ref: r.into(),
            label: "Kernel modules".into(),
            detail: if evidence.boot.loaded_modules.is_empty() {
                "No virtio modules in initramfs".into()
            } else {
                evidence.boot.loaded_modules.join(", ")
            },
        }),
        "storage.crypttab_entries" => Some(EvidenceHighlight {
            r#ref: r.into(),
            label: "Crypttab".into(),
            detail: if evidence.storage.crypttab_entries.is_empty() {
                "No encrypted volumes in crypttab".into()
            } else {
                evidence
                    .storage
                    .crypttab_entries
                    .iter()
                    .map(|c| c.name.clone())
                    .collect::<Vec<_>>()
                    .join(", ")
            },
        }),
        other if other.starts_with("check:") => None,
        _ => None,
    }
}

pub fn generate_briefing(
    target: &str,
    boot: &BootabilityReport,
    migration: Option<&MigrationScoreReport>,
    root_cause: Option<&RootCauseReport>,
    evidence: &EvidenceSnapshot,
) -> MigrationBriefing {
    let digest = build_evidence_digest(evidence);
    let highlights = resolve_evidence_highlights(evidence, root_cause);
    let blocker_count = boot.blockers.len();
    let warning_count = boot.warnings.len();

    let readiness = if blocker_count > 0 {
        "blocked"
    } else if boot.score >= 75.0 {
        "ready"
    } else if boot.score >= 50.0 {
        "caution"
    } else {
        "high_risk"
    };

    let headline = match readiness {
        "ready" => format!("Ready to migrate to {target}"),
        "caution" => format!("Migratable with precautions on {target}"),
        "blocked" => "Migration blocked — fix boot issues first".to_string(),
        _ => format!("High risk for first boot on {target}"),
    };

    let mut summary_parts = vec![boot.summary.clone()];
    if let Some(rc) = root_cause {
        summary_parts.push(rc.summary.clone());
    }
    if let Some(m) = migration {
        summary_parts.push(format!(
            "Migration readiness {:.0}% — est. downtime {} min.",
            m.score, m.estimated_downtime_minutes
        ));
    }
    let summary = summary_parts.join(" ");

    let mut actions = Vec::new();
    for (i, b) in boot.blockers.iter().enumerate() {
        actions.push(CopilotAction {
            priority: (i + 1) as u8,
            title: b.title.clone(),
            detail: b.remediation.clone().unwrap_or_else(|| b.message.clone()),
            workflow: "repair-plan".into(),
        });
    }
    if actions.is_empty() && warning_count > 0 {
        for (i, w) in boot.warnings.iter().take(3).enumerate() {
            actions.push(CopilotAction {
                priority: (i + 1) as u8,
                title: w.title.clone(),
                detail: w.remediation.clone().unwrap_or_else(|| w.message.clone()),
                workflow: "migration-plan".into(),
            });
        }
    }
    if actions.is_empty() {
        actions.push(CopilotAction {
            priority: 1,
            title: "Generate migration plan".into(),
            detail: "Boot checks passed — compute driver and config changes for the target.".into(),
            workflow: "migration-plan".into(),
        });
    }

    let next_workflow = match readiness {
        "blocked" => "repair-plan",
        "ready" => "provision",
        _ => "migration-plan",
    }
    .to_string();

    let insights = build_insights(target, boot, migration, root_cause, &digest, &highlights);

    MigrationBriefing {
        readiness: readiness.into(),
        headline,
        summary,
        boot_score: boot.score,
        migration_score: migration.map(|m| m.score),
        blocker_count,
        warning_count,
        evidence_digest: digest,
        evidence_highlights: highlights,
        recommended_actions: actions,
        insights,
        next_workflow,
    }
}

fn build_insights(
    target: &str,
    boot: &BootabilityReport,
    migration: Option<&MigrationScoreReport>,
    root_cause: Option<&RootCauseReport>,
    digest: &EvidenceDigest,
    highlights: &[EvidenceHighlight],
) -> Vec<CopilotInsight> {
    let blockers_answer = if boot.blockers.is_empty() {
        "No boot blockers detected. Warnings may still affect first boot.".into()
    } else {
        boot.blockers
            .iter()
            .map(|b| format!("{} — {}", b.title, b.message))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let fix_first = boot
        .blockers
        .first()
        .map(|b| {
            b.remediation
                .clone()
                .unwrap_or_else(|| format!("Address: {}", b.message))
        })
        .unwrap_or_else(|| {
            if boot.warnings.is_empty() {
                "Run Migrate Plan to review driver and config changes.".into()
            } else {
                boot.warnings[0]
                    .remediation
                    .clone()
                    .unwrap_or_else(|| boot.warnings[0].message.clone())
            }
        });

    let score_answer = if let Some(rc) = root_cause {
        format!(
            "Boot score {:.0}%. Primary cause: {} (confidence {:.0}%). {}",
            boot.score,
            rc.primary_cause,
            rc.confidence * 100.0,
            rc.summary
        )
    } else {
        format!("Boot score {:.0}%. {}", boot.score, boot.summary)
    };

    let ready_answer = if boot.blockers.is_empty() && boot.score >= 75.0 {
        format!(
            "Yes — {:.0}% boot score with no blockers for {target}. Proceed to migration plan, then provision KubeVirt YAML.",
            boot.score
        )
    } else if boot.blockers.is_empty() {
        format!(
            "Partially — score {:.0}% with warnings. Review migration plan before cutover.",
            boot.score
        )
    } else {
        format!(
            "Not yet — {} blocker(s). Run Repair (dry-run) before planning cutover.",
            boot.blockers.len()
        )
    };

    let evidence_answer = if highlights.is_empty() {
        format!(
            "Collected {} fstab entries, {} kernels, bootloader {}.",
            digest.fstab_entries, digest.kernel_count, digest.bootloader
        )
    } else {
        highlights
            .iter()
            .map(|h| format!("{}: {}", h.label, h.detail))
            .collect::<Vec<_>>()
            .join("\n")
    };

    let mut insights = vec![
        CopilotInsight {
            id: "blockers".into(),
            question: "What's blocking migration?".into(),
            answer: blockers_answer,
        },
        CopilotInsight {
            id: "fix_first".into(),
            question: "What should I fix first?".into(),
            answer: fix_first,
        },
        CopilotInsight {
            id: "boot_score".into(),
            question: "Why is the boot score what it is?".into(),
            answer: score_answer,
        },
        CopilotInsight {
            id: "ready".into(),
            question: format!("Am I ready for {target}?"),
            answer: ready_answer,
        },
        CopilotInsight {
            id: "evidence".into(),
            question: "What evidence supports the diagnosis?".into(),
            answer: evidence_answer,
        },
    ];

    if let Some(m) = migration {
        let changes = if m.required_changes.is_empty() {
            "No mandatory config changes flagged.".into()
        } else {
            m.required_changes.join("\n")
        };
        insights.push(CopilotInsight {
            id: "migration_changes".into(),
            question: "What changes are required for migration?".into(),
            answer: format!(
                "Migration score {:.0}%. Required changes:\n{changes}",
                m.score
            ),
        });
    }

    insights
}

/// Match a free-text copilot question to the best pre-computed insight.
pub fn answer_copilot_question(question: &str, briefing: &MigrationBriefing) -> CopilotInsight {
    let q = question.to_lowercase();

    let id = if q.contains("block") || q.contains("stop") || q.contains("prevent") {
        "blockers"
    } else if q.contains("fix") || q.contains("first") || q.contains("repair") {
        "fix_first"
    } else if q.contains("score") || q.contains("why") || q.contains("low") || q.contains("boot") {
        "boot_score"
    } else if q.contains("ready") || q.contains("go live") || q.contains("proceed") {
        "ready"
    } else if q.contains("evidence") || q.contains("proof") || q.contains("support") {
        "evidence"
    } else if q.contains("change") || q.contains("driver") || q.contains("migrate") {
        "migration_changes"
    } else {
        "boot_score"
    };

    briefing
        .insights
        .iter()
        .find(|i| i.id == id)
        .cloned()
        .unwrap_or_else(|| CopilotInsight {
            id: "summary".into(),
            question: question.to_string(),
            answer: briefing.summary.clone(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::BootabilityReport;
    use crate::evidence::snapshot::{
        BootEvidence, EvidenceSnapshot, OsEvidence, PackageEvidence, SecurityEvidence,
        StorageEvidence, VmToolsEvidence,
    };

    fn empty_evidence() -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: 2,
            image_path: "/tmp/test.qcow2".into(),
            collected_at: "now".into(),
            root: "/".into(),
            os: OsEvidence::default(),
            storage: StorageEvidence::default(),
            boot: BootEvidence::default(),
            network: Default::default(),
            packages: PackageEvidence::default(),
            security: SecurityEvidence::default(),
            vm_tools: VmToolsEvidence::default(),
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
        }
    }

    #[test]
    fn briefing_marks_blocked_when_blockers_present() {
        let evidence = empty_evidence();
        let boot = BootabilityReport {
            score: 40.0,
            confidence: 0.8,
            target: "kubevirt".into(),
            blockers: vec![crate::boot::Finding {
                check_id: "BOOT-001".into(),
                title: "Fstab".into(),
                message: "UUID mismatch".into(),
                remediation: Some("Fix fstab".into()),
            }],
            warnings: vec![],
            checks: vec![],
            summary: "Boot unlikely".into(),
        };
        let b = generate_briefing("kubevirt", &boot, None, None, &evidence);
        assert_eq!(b.readiness, "blocked");
        assert_eq!(b.next_workflow, "repair-plan");
        assert!(!b.insights.is_empty());
    }

    #[test]
    fn answer_routes_blocker_questions() {
        let evidence = empty_evidence();
        let boot = BootabilityReport {
            score: 90.0,
            confidence: 0.9,
            target: "kubevirt".into(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: "Looks good".into(),
        };
        let b = generate_briefing("kubevirt", &boot, None, None, &evidence);
        let ans = answer_copilot_question("what is blocking migration?", &b);
        assert_eq!(ans.id, "blockers");
    }
}
