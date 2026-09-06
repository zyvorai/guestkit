// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Inspection profiles for focused use cases

use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};

pub mod compliance;
pub mod hardening;
pub mod migration;
pub mod performance;
pub mod security;
pub mod windows_migration;

pub use compliance::ComplianceProfile;
pub use hardening::HardeningProfile;
pub use migration::MigrationProfile;
pub use performance::PerformanceProfile;
pub use security::SecurityProfile;
pub use windows_migration::WindowsMigrationProfile;

/// Risk level for security findings
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Critical,
    High,
    Medium,
    Low,
    Info,
}

impl std::fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RiskLevel::Critical => write!(f, "CRITICAL"),
            RiskLevel::High => write!(f, "HIGH"),
            RiskLevel::Medium => write!(f, "MEDIUM"),
            RiskLevel::Low => write!(f, "LOW"),
            RiskLevel::Info => write!(f, "INFO"),
        }
    }
}

/// Report section with findings
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReportSection {
    pub title: String,
    pub findings: Vec<Finding>,
}

/// Individual finding in a report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Finding {
    pub item: String,
    pub status: FindingStatus,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub risk_level: Option<RiskLevel>,
}

/// Status of a finding
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FindingStatus {
    Pass,
    Warning,
    Fail,
    Info,
}

impl std::fmt::Display for FindingStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            FindingStatus::Pass => write!(f, "✓"),
            FindingStatus::Warning => write!(f, "⚠"),
            FindingStatus::Fail => write!(f, "✗"),
            FindingStatus::Info => write!(f, "ℹ"),
        }
    }
}

/// Profile report structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileReport {
    pub profile_name: String,
    pub sections: Vec<ReportSection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub overall_risk: Option<RiskLevel>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
}

/// Trait for inspection profiles
pub trait InspectionProfile {
    /// Get profile name
    #[allow(dead_code)]
    fn name(&self) -> &str;

    /// Get profile description
    fn description(&self) -> &str;

    /// Run inspection with this profile
    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport>;
}

/// Get profile by name
pub fn get_profile(name: &str) -> Option<Box<dyn InspectionProfile>> {
    match name.to_lowercase().as_str() {
        "security" => Some(Box::new(SecurityProfile)),
        "migration" => Some(Box::new(MigrationProfile)),
        "performance" => Some(Box::new(PerformanceProfile)),
        "compliance" => Some(Box::new(ComplianceProfile)),
        "hardening" => Some(Box::new(HardeningProfile)),
        "windows-migration" | "windows_migration" => Some(Box::new(WindowsMigrationProfile)),
        // windows-rdp / windows-hostname / windows-winrm / windows-domain-leave /
        // windows-timezone / windows-static-ip / linux-ssh are
        // generate-only plan profiles (see PlanGenerator::* in plan/generator.rs)
        _ => None,
    }
}

/// List available profiles
#[allow(dead_code)]
pub fn list_profiles() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "security",
            "Security posture assessment and hardening recommendations",
        ),
        ("migration", "Migration planning and compatibility analysis"),
        (
            "performance",
            "Performance tuning opportunities and bottleneck detection",
        ),
        (
            "compliance",
            "Regulatory compliance assessment (CIS, FIPS, HIPAA, PCI-DSS)",
        ),
        (
            "hardening",
            "System hardening recommendations with actionable remediation steps",
        ),
        (
            "windows-migration",
            "Windows migration readiness for VMware/KVM exits",
        ),
        (
            "windows-rdp",
            "Generate-only: offline Windows Remote Desktop enablement (Terminal Server + firewall + TermService)",
        ),
        (
            "windows-hostname",
            "Generate-only: offline Windows hostname (ComputerName + Tcpip Hostname); requires --hostname",
        ),
        (
            "windows-winrm",
            "Generate-only: offline Windows WinRM (service Automatic + WINRM-HTTP-In-TCP)",
        ),
        (
            "windows-domain-leave",
            "Generate-only: offline domain→workgroup markers (--workgroup, default WORKGROUP)",
        ),
        (
            "windows-timezone",
            "Generate-only: offline TimeZoneKeyName (--timezone, e.g. UTC)",
        ),
        (
            "windows-static-ip",
            "Generate-only: offline static IPv4 (--interface-guid --ip --mask [--gateway] [--dns])",
        ),
        (
            "linux-ssh",
            "Generate-only: offline Linux SSH enablement (wants symlink + sshd drop-in; optional --user/--key)",
        ),
        (
            "selinux-relabel",
            "Generate-only: offline FileWrite /.autorelabel (next boot restorecon)",
        ),
        (
            "sysprep",
            "Generate-only: offline Windows unattend + optional first-boot generalize flag",
        ),
        (
            "virtio-initramfs",
            "Generate-only: virtio modules drop-in + /GuestKit/rebuild-initramfs.flag (first-boot rebuild)",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_risk_level_display() {
        assert_eq!(RiskLevel::Critical.to_string(), "CRITICAL");
        assert_eq!(RiskLevel::High.to_string(), "HIGH");
        assert_eq!(RiskLevel::Medium.to_string(), "MEDIUM");
        assert_eq!(RiskLevel::Low.to_string(), "LOW");
        assert_eq!(RiskLevel::Info.to_string(), "INFO");
    }

    #[test]
    fn test_risk_level_ordering() {
        // Test that risk levels can be compared
        assert_eq!(RiskLevel::High, RiskLevel::High);
        assert_ne!(RiskLevel::High, RiskLevel::Low);
    }

    #[test]
    fn test_finding_status_display() {
        assert_eq!(FindingStatus::Pass.to_string(), "✓");
        assert_eq!(FindingStatus::Warning.to_string(), "⚠");
        assert_eq!(FindingStatus::Fail.to_string(), "✗");
        assert_eq!(FindingStatus::Info.to_string(), "ℹ");
    }

    #[test]
    fn test_finding_creation() {
        let finding = Finding {
            item: "SSH Configuration".to_string(),
            status: FindingStatus::Pass,
            message: "SSH root login is disabled".to_string(),
            risk_level: Some(RiskLevel::High),
        };

        assert_eq!(finding.item, "SSH Configuration");
        assert_eq!(finding.status, FindingStatus::Pass);
        assert_eq!(finding.risk_level, Some(RiskLevel::High));
    }

    #[test]
    fn test_report_section_creation() {
        let section = ReportSection {
            title: "Network Security".to_string(),
            findings: vec![Finding {
                item: "Firewall".to_string(),
                status: FindingStatus::Pass,
                message: "Firewall enabled".to_string(),
                risk_level: Some(RiskLevel::High),
            }],
        };

        assert_eq!(section.title, "Network Security");
        assert_eq!(section.findings.len(), 1);
    }

    #[test]
    fn test_profile_report_creation() {
        let report = ProfileReport {
            profile_name: "Security".to_string(),
            sections: vec![],
            overall_risk: Some(RiskLevel::Medium),
            summary: Some("System has moderate security issues".to_string()),
        };

        assert_eq!(report.profile_name, "Security");
        assert_eq!(report.overall_risk, Some(RiskLevel::Medium));
        assert!(report.summary.is_some());
    }

    #[test]
    fn test_get_profile_security() {
        let profile = get_profile("security");
        assert!(profile.is_some());
    }

    #[test]
    fn test_get_profile_migration() {
        let profile = get_profile("migration");
        assert!(profile.is_some());
    }

    #[test]
    fn test_get_profile_case_insensitive() {
        let profile1 = get_profile("SECURITY");
        let profile2 = get_profile("Security");
        let profile3 = get_profile("security");

        assert!(profile1.is_some());
        assert!(profile2.is_some());
        assert!(profile3.is_some());
    }

    #[test]
    fn test_get_profile_invalid() {
        let profile = get_profile("nonexistent");
        assert!(profile.is_none());
    }

    #[test]
    fn test_list_profiles_count() {
        let profiles = list_profiles();
        assert_eq!(profiles.len(), 16);
    }

    #[test]
    fn test_list_profiles_contains_all() {
        let profiles = list_profiles();
        let names: Vec<&str> = profiles.iter().map(|(name, _)| *name).collect();

        assert!(names.contains(&"security"));
        assert!(names.contains(&"migration"));
        assert!(names.contains(&"performance"));
        assert!(names.contains(&"compliance"));
        assert!(names.contains(&"hardening"));
        assert!(names.contains(&"windows-migration"));
        assert!(names.contains(&"windows-rdp"));
        assert!(names.contains(&"windows-hostname"));
        assert!(names.contains(&"windows-winrm"));
        assert!(names.contains(&"windows-domain-leave"));
        assert!(names.contains(&"windows-timezone"));
        assert!(names.contains(&"windows-static-ip"));
        assert!(names.contains(&"linux-ssh"));
        assert!(names.contains(&"selinux-relabel"));
        assert!(names.contains(&"sysprep"));
        assert!(names.contains(&"virtio-initramfs"));
    }

    #[test]
    fn test_finding_status_equality() {
        assert_eq!(FindingStatus::Pass, FindingStatus::Pass);
        assert_ne!(FindingStatus::Pass, FindingStatus::Fail);
    }

    #[test]
    fn test_finding_without_risk_level() {
        let finding = Finding {
            item: "Info item".to_string(),
            status: FindingStatus::Info,
            message: "Informational message".to_string(),
            risk_level: None,
        };

        assert!(finding.risk_level.is_none());
    }
}
