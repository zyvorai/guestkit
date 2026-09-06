// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Windows migration readiness profile.

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection, RiskLevel};
use crate::guestfs::windows_registry;
use crate::Guestfs;
use anyhow::Result;
use std::path::PathBuf;

pub struct WindowsMigrationProfile;

impl InspectionProfile for WindowsMigrationProfile {
    fn name(&self) -> &str {
        "windows-migration"
    }

    fn description(&self) -> &str {
        "Windows migration readiness for VMware/KVM exits"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            self.analyze_os(g, root),
            self.analyze_boot_blockers(g, root),
            self.analyze_hypervisor_remnants(g, root),
            self.analyze_security(g, root),
            self.analyze_drivers(g, root),
        ];

        let has_critical = sections.iter().any(|s| {
            s.findings
                .iter()
                .any(|f| f.risk_level == Some(RiskLevel::Critical))
        });

        Ok(ProfileReport {
            profile_name: "Windows Migration".to_string(),
            sections,
            overall_risk: if has_critical {
                Some(RiskLevel::Critical)
            } else {
                Some(RiskLevel::Medium)
            },
            summary: Some(
                "Review boot blockers, hypervisor remnants, and driver gaps before KVM migration."
                    .to_string(),
            ),
        })
    }
}

impl WindowsMigrationProfile {
    fn analyze_os(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();
        let systemroot = g
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let software = format!("{}/System32/config/SOFTWARE", systemroot);

        if let Ok((name, version, edition)) =
            windows_registry::get_windows_version(PathBuf::from(&software).as_path())
        {
            findings.push(Finding {
                item: "Windows Version".to_string(),
                status: FindingStatus::Info,
                message: format!("{} {} ({})", name, version, edition),
                risk_level: None,
            });
        }

        ReportSection {
            title: "Operating System".to_string(),
            findings,
        }
    }

    fn analyze_boot_blockers(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();
        let systemroot = g
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let system_hive = format!("{}/System32/config/SYSTEM", systemroot);

        if windows_registry::parse_pending_reboot(PathBuf::from(&system_hive).as_path()) {
            findings.push(Finding {
                item: "Pending Reboot".to_string(),
                status: FindingStatus::Warning,
                message: "PendingFileRenameOperations detected — reboot required before migration"
                    .to_string(),
                risk_level: Some(RiskLevel::High),
            });
        }

        if g.exists("/$BitLocker").unwrap_or(false)
            || windows_registry::parse_bitlocker_status(
                PathBuf::from(format!("{}/System32/config/SOFTWARE", systemroot)).as_path(),
            )
        {
            findings.push(Finding {
                item: "BitLocker".to_string(),
                status: FindingStatus::Fail,
                message: "BitLocker encryption detected — suspend before migration".to_string(),
                risk_level: Some(RiskLevel::Critical),
            });
        }

        if g.is_windows_hibernated().unwrap_or(false) {
            findings.push(Finding {
                item: "Hibernation".to_string(),
                status: FindingStatus::Fail,
                message: "Windows is hibernated — delete hiberfil.sys or fully shut down"
                    .to_string(),
                risk_level: Some(RiskLevel::Critical),
            });
        }

        let minidumps = format!("{}/Minidump", systemroot);
        if let Ok(dumps) = g.ls(&minidumps) {
            if !dumps.is_empty() {
                findings.push(Finding {
                    item: "BSOD History".to_string(),
                    status: FindingStatus::Warning,
                    message: format!("{} minidump(s) found — review crash history", dumps.len()),
                    risk_level: Some(RiskLevel::Medium),
                });
            }
        }

        ReportSection {
            title: "Boot Blockers".to_string(),
            findings,
        }
    }

    fn analyze_hypervisor_remnants(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();
        let systemroot = g
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let system_hive = format!("{}/System32/config/SYSTEM", systemroot);
        let drivers_path = format!("{}/System32/drivers", systemroot);

        let remnants = windows_registry::detect_hypervisor_remnants(
            PathBuf::from(&system_hive).as_path(),
            &drivers_path,
            g,
        );

        for r in &remnants {
            findings.push(Finding {
                item: "Hypervisor Remnant".to_string(),
                status: FindingStatus::Warning,
                message: format!("Detected: {} — remove before KVM migration", r),
                risk_level: Some(RiskLevel::Medium),
            });
        }

        if remnants.is_empty() {
            findings.push(Finding {
                item: "Hypervisor Remnants".to_string(),
                status: FindingStatus::Pass,
                message: "No VMware/Hyper-V guest components detected".to_string(),
                risk_level: None,
            });
        }

        ReportSection {
            title: "Hypervisor Remnants".to_string(),
            findings,
        }
    }

    fn analyze_security(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();
        let systemroot = g
            .inspect_get_windows_systemroot(root)
            .unwrap_or_else(|_| "/Windows".to_string());
        let system_hive = format!("{}/System32/config/SYSTEM", systemroot);
        let software = format!("{}/System32/config/SOFTWARE", systemroot);

        let (joined, domain) =
            windows_registry::parse_domain_info(PathBuf::from(&system_hive).as_path());
        if joined {
            findings.push(Finding {
                item: "Domain Join".to_string(),
                status: FindingStatus::Info,
                message: format!("Domain joined: {}", domain.unwrap_or_default()),
                risk_level: None,
            });
        }

        if windows_registry::parse_rdp_enabled(PathBuf::from(&system_hive).as_path()) {
            findings.push(Finding {
                item: "RDP Exposure".to_string(),
                status: FindingStatus::Warning,
                message: "Remote Desktop is enabled — audit before migration".to_string(),
                risk_level: Some(RiskLevel::Medium),
            });
        }

        let av =
            windows_registry::detect_av_edr(PathBuf::from(&software).as_path(), g, &systemroot);
        for product in av {
            findings.push(Finding {
                item: "AV/EDR".to_string(),
                status: FindingStatus::Info,
                message: format!("Detected: {}", product),
                risk_level: None,
            });
        }

        ReportSection {
            title: "Security & Compliance".to_string(),
            findings,
        }
    }

    fn analyze_drivers(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();
        let drivers = g.inspect_list_windows_drivers(root).unwrap_or_default();

        let virtio_needed = ["viostor", "vioscsi", "netkvm", "balloon"];
        let mut missing = Vec::new();
        for needed in virtio_needed {
            if !drivers.iter().any(|d| d.to_lowercase().contains(needed)) {
                missing.push(needed);
            }
        }

        if !missing.is_empty() {
            findings.push(Finding {
                item: "VirtIO Drivers".to_string(),
                status: FindingStatus::Warning,
                message: format!(
                    "Missing VirtIO drivers for KVM: {} — inject before migration",
                    missing.join(", ")
                ),
                risk_level: Some(RiskLevel::High),
            });
        } else {
            findings.push(Finding {
                item: "VirtIO Drivers".to_string(),
                status: FindingStatus::Pass,
                message: "VirtIO storage/network drivers present".to_string(),
                risk_level: None,
            });
        }

        findings.push(Finding {
            item: "Installed Drivers".to_string(),
            status: FindingStatus::Info,
            message: format!("{} kernel drivers in System32/drivers", drivers.len()),
            risk_level: None,
        });

        ReportSection {
            title: "Drivers & VirtIO Readiness".to_string(),
            findings,
        }
    }
}
