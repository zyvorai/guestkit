// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Hypervisor-aware migration scoring.

use crate::boot::BootabilityReport;
use crate::evidence::EvidenceSnapshot;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationScoreReport {
    pub score: f64,
    pub driver_injections: Vec<String>,
    pub required_changes: Vec<String>,
    pub licensing_warnings: Vec<String>,
    pub estimated_downtime_minutes: u32,
}

pub fn compute_migration_score(
    evidence: &EvidenceSnapshot,
    boot: &BootabilityReport,
    target: &str,
) -> MigrationScoreReport {
    let mut score = boot.score;
    let mut driver_injections = Vec::new();
    let mut required_changes = Vec::new();
    let mut licensing_warnings = Vec::new();

    let target_lower = target.to_lowercase();
    if matches!(
        target_lower.as_str(),
        "kvm" | "proxmox" | "qemu" | "kubevirt"
    ) {
        if evidence
            .vm_tools
            .detected
            .iter()
            .any(|t| t.contains("vmware"))
        {
            required_changes.push("Remove VMware Tools; install qemu-guest-agent".to_string());
            score -= 5.0;
        }
        if !evidence
            .boot
            .loaded_modules
            .iter()
            .any(|m| m.contains("virtio"))
        {
            driver_injections.push("virtio_blk".to_string());
            driver_injections.push("virtio_net".to_string());
            driver_injections.push("virtio_pci".to_string());
            score -= 10.0;
        }
        required_changes.push("Set disk bus to virtio-scsi or virtio-blk".to_string());
        required_changes.push("Set network model to virtio-net".to_string());
    }

    if matches!(target_lower.as_str(), "aws" | "azure" | "gcp") {
        licensing_warnings
            .push("Verify OS license portability to cloud (BYOL vs pay-as-you-go)".to_string());
        if evidence.boot.cloud_init_present {
            required_changes.push("Reconfigure cloud-init datasource for target cloud".to_string());
        } else {
            required_changes.push("Install and configure cloud-init for cloud target".to_string());
            score -= 5.0;
        }
    }

    if let Some(win) = &evidence.windows {
        if win.bitlocker_detected {
            licensing_warnings
                .push("BitLocker encryption must be suspended before migration".to_string());
            score -= 20.0;
        }
        if win.pending_reboot {
            required_changes.push("Complete pending Windows reboot before migration".to_string());
            score -= 5.0;
        }
    }

    if evidence.boot.pending_relabel {
        required_changes.push("Allow SELinux relabel on first boot after migration".to_string());
    }

    let blocker_penalty = boot.blockers.len() as f64 * 8.0;
    score = (score - blocker_penalty).clamp(0.0, 100.0);

    let downtime = 30
        + boot.blockers.len() as u32 * 15
        + driver_injections.len() as u32 * 10
        + required_changes.len() as u32 * 5;

    MigrationScoreReport {
        score,
        driver_injections,
        required_changes,
        licensing_warnings,
        estimated_downtime_minutes: downtime,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::boot::report::BootabilityReport;
    use crate::evidence::snapshot::{
        BootEvidence, EvidenceSnapshot, OsEvidence, PackageEvidence, SecurityEvidence,
        StorageEvidence, VmToolsEvidence, SCHEMA_VERSION,
    };

    fn base_evidence() -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: SCHEMA_VERSION,
            image_path: "vm.qcow2".to_string(),
            collected_at: "2026-01-01".to_string(),
            root: "/".to_string(),
            os: OsEvidence::default(),
            storage: StorageEvidence::default(),
            boot: BootEvidence {
                loaded_modules: vec!["virtio_blk".to_string()],
                cloud_init_present: true,
                ..Default::default()
            },
            network: Default::default(),
            packages: PackageEvidence::default(),
            security: SecurityEvidence::default(),
            vm_tools: VmToolsEvidence {
                detected: vec!["open-vm-tools".to_string()],
            },
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

    fn boot_report(score: f64) -> BootabilityReport {
        BootabilityReport {
            score,
            confidence: 0.9,
            target: "KVM".to_string(),
            blockers: vec![],
            warnings: vec![],
            checks: vec![],
            summary: "ok".to_string(),
        }
    }

    #[test]
    fn proxmox_penalizes_vmware_tools_without_virtio() {
        let mut ev = base_evidence();
        ev.boot.loaded_modules.clear();
        ev.vm_tools.detected = vec!["vmware-tools".to_string()];
        let report = compute_migration_score(&ev, &boot_report(90.0), "proxmox");
        assert!(report.score < 90.0);
        assert!(report.required_changes.iter().any(|c| c.contains("VMware")));
        assert!(!report.driver_injections.is_empty());
    }

    #[test]
    fn bitlocker_lowers_score() {
        let mut ev = base_evidence();
        ev.windows = Some(crate::evidence::snapshot::WindowsEvidence {
            bitlocker_detected: true,
            ..Default::default()
        });
        let report = compute_migration_score(&ev, &boot_report(85.0), "kvm");
        assert!(report.score <= 65.0);
        assert!(!report.licensing_warnings.is_empty());
    }
}
