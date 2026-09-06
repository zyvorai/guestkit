// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Bootability analysis engine.

use super::checks::all_checks;
use super::report::{BootabilityReport, CheckSeverity, Finding};
use crate::evidence::EvidenceSnapshot;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootTarget {
    Generic,
    Kvm,
    KubeVirt,
    Proxmox,
    HyperV,
    Cloud,
}

impl BootTarget {
    pub fn parse(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "kvm" | "qemu" => Self::Kvm,
            "kubevirt" => Self::KubeVirt,
            "proxmox" => Self::Proxmox,
            "hyperv" | "hyper-v" => Self::HyperV,
            "cloud" | "aws" | "azure" | "gcp" => Self::Cloud,
            _ => Self::Generic,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Generic => "generic",
            Self::Kvm => "kvm",
            Self::KubeVirt => "kubevirt",
            Self::Proxmox => "proxmox",
            Self::HyperV => "hyper-v",
            Self::Cloud => "cloud",
        }
    }
}

pub fn analyze_bootability(evidence: &EvidenceSnapshot, target: BootTarget) -> BootabilityReport {
    let target_str = target.as_str();
    let checks: Vec<_> = all_checks()
        .iter()
        .map(|c| c.run(evidence, target_str))
        .collect();

    let mut blockers = Vec::new();
    let mut warnings = Vec::new();
    let mut total_weight = 0.0f64;
    let mut earned_weight = 0.0f64;
    let mut executed = 0usize;

    for check in &checks {
        if check.weight <= 0.0 {
            continue;
        }
        executed += 1;
        total_weight += check.weight;
        if check.passed {
            earned_weight += check.weight;
        } else {
            match check.severity {
                CheckSeverity::Blocker => {
                    blockers.push(Finding {
                        check_id: check.id.clone(),
                        title: check.name.clone(),
                        message: check.message.clone(),
                        remediation: Some(remediation_for(&check.id)),
                    });
                }
                CheckSeverity::Warning => {
                    earned_weight += check.weight * 0.5;
                    warnings.push(Finding {
                        check_id: check.id.clone(),
                        title: check.name.clone(),
                        message: check.message.clone(),
                        remediation: Some(remediation_for(&check.id)),
                    });
                }
                CheckSeverity::Info => {
                    earned_weight += check.weight;
                }
            }
        }
    }

    let score = if blockers.is_empty() && total_weight > 0.0 {
        (earned_weight / total_weight * 100.0).clamp(0.0, 100.0)
    } else if !blockers.is_empty() {
        (earned_weight / total_weight * 50.0).clamp(0.0, 49.0)
    } else {
        100.0
    };

    let confidence = if executed >= 8 {
        0.91
    } else if executed >= 5 {
        0.75
    } else {
        0.5
    };

    let summary = if blockers.is_empty() {
        format!(
            "{:.0}% chance of successful first boot on {} (confidence: {:.0}%)",
            score,
            target_str,
            confidence * 100.0
        )
    } else {
        format!(
            "Boot blocked by {} issue(s). Score: {:.0}% on {}",
            blockers.len(),
            score,
            target_str
        )
    };

    BootabilityReport {
        score,
        confidence,
        target: target_str.to_string(),
        blockers,
        warnings,
        checks,
        summary,
    }
}

fn remediation_for(check_id: &str) -> String {
    match check_id {
        "BOOT-001" => "Update /etc/fstab UUIDs to match current partition table".to_string(),
        "BOOT-002" => "Fix /etc/crypttab device references".to_string(),
        "BOOT-003" => "Regenerate initramfs: dracut --force or update-initramfs -u".to_string(),
        "BOOT-004" => "Run grub-install and grub-mkconfig".to_string(),
        "BOOT-006" => "Install virtio drivers and rebuild initramfs before migration".to_string(),
        "BOOT-009" => "Allow SELinux relabel on first boot or run fixfiles onboot".to_string(),
        "BOOT-010" => "Remove VMware/Hyper-V guest tools and install qemu-guest-agent".to_string(),
        "BOOT-012" => {
            "Repair EFI boot chain: verify bootmgfw.efi and run bcdboot from WinPE".to_string()
        }
        "BOOT-013" => "Rebuild BCD from WinPE: bcdboot %WINDIR% or bootrec /rebuildbcd".to_string(),
        "BOOT-014" => {
            "Preserve System Reserved / ESP with the OS disk; do not convert C: alone".to_string()
        }
        _ => "Review check output and apply fix plan".to_string(),
    }
}
