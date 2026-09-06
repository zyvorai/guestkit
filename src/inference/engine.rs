// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rule-based root cause inference.

use super::report::{CauseStep, RootCauseReport};
use crate::boot::BootabilityReport;
use crate::evidence::EvidenceSnapshot;

pub fn infer_root_cause(evidence: &EvidenceSnapshot, boot: &BootabilityReport) -> RootCauseReport {
    let mut chain = Vec::new();
    let mut evidence_refs = Vec::new();

    let (primary, confidence) =
        if let Some(check) = boot.checks.iter().find(|c| c.id == "BOOT-001" && !c.passed) {
            evidence_refs.push("storage.fstab_entries".to_string());
            chain.push(CauseStep {
                step: 1,
                description: check.message.clone(),
                check_id: Some("BOOT-001".to_string()),
            });
            chain.push(CauseStep {
                step: 2,
                description: "Kernel cannot mount root → initramfs drops to emergency shell"
                    .to_string(),
                check_id: None,
            });
            (
                "Fstab UUID mismatch prevents root filesystem mount".to_string(),
                0.91,
            )
        } else if let Some(check) = boot.checks.iter().find(|c| c.id == "BOOT-003" && !c.passed) {
            evidence_refs.push("boot.initramfs_paths".to_string());
            chain.push(CauseStep {
                step: 1,
                description: check.message.clone(),
                check_id: Some("BOOT-003".to_string()),
            });
            if !evidence.packages.kernels.is_empty() {
                chain.push(CauseStep {
                    step: 2,
                    description: format!(
                        "Installed kernels: {}",
                        evidence.packages.kernels.join(", ")
                    ),
                    check_id: None,
                });
            }
            (
                "Missing or outdated initramfs after kernel update".to_string(),
                0.88,
            )
        } else if let Some(check) = boot.checks.iter().find(|c| c.id == "BOOT-006" && !c.passed) {
            evidence_refs.push("boot.loaded_modules".to_string());
            chain.push(CauseStep {
                step: 1,
                description: check.message.clone(),
                check_id: Some("BOOT-006".to_string()),
            });
            chain.push(CauseStep {
                step: 2,
                description: "Disk/network inaccessible without virtio_blk/virtio_net".to_string(),
                check_id: None,
            });
            (
                "Missing virtio drivers for KVM target hypervisor".to_string(),
                0.85,
            )
        } else if let Some(check) = boot.checks.iter().find(|c| c.id == "BOOT-002" && !c.passed) {
            evidence_refs.push("storage.crypttab_entries".to_string());
            chain.push(CauseStep {
                step: 1,
                description: check.message.clone(),
                check_id: Some("BOOT-002".to_string()),
            });
            (
                "Encrypted volume configuration error in crypttab".to_string(),
                0.90,
            )
        } else if let Some(blocker) = boot.blockers.first() {
            evidence_refs.push(format!("check:{}", blocker.check_id));
            chain.push(CauseStep {
                step: 1,
                description: blocker.message.clone(),
                check_id: Some(blocker.check_id.clone()),
            });
            (blocker.title.clone(), boot.confidence * 0.9)
        } else if boot.score >= 80.0 {
            chain.push(CauseStep {
                step: 1,
                description: boot.summary.clone(),
                check_id: None,
            });
            (
                "No significant boot blockers detected".to_string(),
                boot.confidence,
            )
        } else {
            for (i, w) in boot.warnings.iter().take(3).enumerate() {
                chain.push(CauseStep {
                    step: i + 1,
                    description: w.message.clone(),
                    check_id: Some(w.check_id.clone()),
                });
            }
            (
                "Multiple warnings may cause boot instability".to_string(),
                0.6,
            )
        };

    let summary = format!(
        "probable cause: {} (confidence: {:.0}%)",
        primary,
        confidence * 100.0
    );

    RootCauseReport {
        primary_cause: primary,
        confidence,
        chain,
        evidence_refs,
        summary,
    }
}
