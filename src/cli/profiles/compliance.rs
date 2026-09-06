// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Compliance audit profile
//!
//! Checks for regulatory compliance (CIS Benchmarks, FIPS, HIPAA, PCI-DSS)

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection, RiskLevel};
use crate::Guestfs;
use anyhow::Result;

pub struct ComplianceProfile;

impl InspectionProfile for ComplianceProfile {
    fn name(&self) -> &str {
        "compliance"
    }

    fn description(&self) -> &str {
        "Regulatory compliance assessment (CIS, FIPS, HIPAA, PCI-DSS)"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            // Section 1: CIS Benchmarks
            self.audit_cis_benchmarks(g, root),
            // Section 2: FIPS Compliance
            self.audit_fips(g, root),
            // Section 3: Password Policy
            self.audit_password_policy(g, root),
            // Section 4: Audit Logging
            self.audit_logging(g, root),
            // Section 5: File Permissions
            self.audit_file_permissions(g, root),
        ];

        // Calculate overall risk
        let overall_risk = self.calculate_risk(&sections);

        Ok(ProfileReport {
            profile_name: "Compliance Audit".to_string(),
            sections,
            overall_risk: Some(overall_risk),
            summary: Some(format!(
                "Overall compliance risk level: {}. Address critical findings for regulatory compliance.",
                overall_risk
            )),
        })
    }
}

impl ComplianceProfile {
    /// CIS Benchmarks (Center for Internet Security)
    /// Checks based on CIS Linux Benchmark Level 1
    fn audit_cis_benchmarks(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // CIS 1.1.1.1: Ensure mounting of cramfs filesystems is disabled
        if let Ok(modprobe) = g.with_mount(root, |guestfs| {
            guestfs.read_file("/etc/modprobe.d/cramfs.conf")
        }) {
            let content = String::from_utf8_lossy(&modprobe);
            if content.contains("install cramfs /bin/true") {
                findings.push(Finding {
                    item: "CIS 1.1.1.1 - cramfs disabled".to_string(),
                    status: FindingStatus::Pass,
                    message: "cramfs filesystem mounting is disabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "CIS 1.1.1.1 - cramfs disabled".to_string(),
                    status: FindingStatus::Fail,
                    message: "cramfs filesystem mounting is not disabled".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }
        } else {
            findings.push(Finding {
                item: "CIS 1.1.1.1 - cramfs disabled".to_string(),
                status: FindingStatus::Fail,
                message: "cramfs configuration not found".to_string(),
                risk_level: Some(RiskLevel::Medium),
            });
        }

        // CIS 1.5.1: Ensure permissions on bootloader config are configured
        if let Ok(stat) = g.with_mount(root, |guestfs| {
            guestfs
                .stat("/boot/grub2/grub.cfg")
                .or_else(|_| guestfs.stat("/boot/grub/grub.cfg"))
        }) {
            let mode = stat.mode & 0o777;
            if mode == 0o400 || mode == 0o600 {
                findings.push(Finding {
                    item: "CIS 1.5.1 - Bootloader permissions".to_string(),
                    status: FindingStatus::Pass,
                    message: format!("grub.cfg has secure permissions: {:o}", mode),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "CIS 1.5.1 - Bootloader permissions".to_string(),
                    status: FindingStatus::Fail,
                    message: format!(
                        "grub.cfg has insecure permissions: {:o} (should be 0400 or 0600)",
                        mode
                    ),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        // CIS 3.4.1: Ensure TCP Wrappers is installed
        if let Ok(packages) = g.inspect_packages(root) {
            let has_tcp_wrappers = packages
                .packages
                .iter()
                .any(|pkg| pkg.name.contains("tcp_wrappers") || pkg.name.contains("tcpd"));

            if has_tcp_wrappers {
                findings.push(Finding {
                    item: "CIS 3.4.1 - TCP Wrappers".to_string(),
                    status: FindingStatus::Pass,
                    message: "TCP Wrappers is installed".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "CIS 3.4.1 - TCP Wrappers".to_string(),
                    status: FindingStatus::Warning,
                    message: "TCP Wrappers is not installed".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }
        }

        // CIS 4.1.1.1: Ensure auditd is installed
        if let Ok(packages) = g.inspect_packages(root) {
            let has_auditd = packages
                .packages
                .iter()
                .any(|pkg| pkg.name.contains("audit"));

            if has_auditd {
                findings.push(Finding {
                    item: "CIS 4.1.1.1 - auditd installed".to_string(),
                    status: FindingStatus::Pass,
                    message: "auditd package is installed".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "CIS 4.1.1.1 - auditd installed".to_string(),
                    status: FindingStatus::Fail,
                    message: "auditd package is not installed".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        // CIS 5.2.1: Ensure permissions on /etc/ssh/sshd_config are configured
        if let Ok(stat) = g.with_mount(root, |guestfs| guestfs.stat("/etc/ssh/sshd_config")) {
            let mode = stat.mode & 0o777;
            if mode == 0o600 {
                findings.push(Finding {
                    item: "CIS 5.2.1 - SSH config permissions".to_string(),
                    status: FindingStatus::Pass,
                    message: "sshd_config has secure permissions: 0600".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "CIS 5.2.1 - SSH config permissions".to_string(),
                    status: FindingStatus::Fail,
                    message: format!(
                        "sshd_config has insecure permissions: {:o} (should be 0600)",
                        mode
                    ),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        ReportSection {
            title: "CIS Benchmarks".to_string(),
            findings,
        }
    }

    /// FIPS 140-2 Compliance
    /// Check if FIPS mode is enabled for cryptographic operations
    fn audit_fips(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check /proc/sys/crypto/fips_enabled
        if let Ok(fips_status) = g.with_mount(root, |guestfs| {
            guestfs.read_file("/proc/sys/crypto/fips_enabled")
        }) {
            let status = String::from_utf8_lossy(&fips_status).trim().to_string();
            if status == "1" {
                findings.push(Finding {
                    item: "FIPS Mode".to_string(),
                    status: FindingStatus::Pass,
                    message: "FIPS mode is enabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "FIPS Mode".to_string(),
                    status: FindingStatus::Fail,
                    message: "FIPS mode is not enabled".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        } else {
            findings.push(Finding {
                item: "FIPS Mode".to_string(),
                status: FindingStatus::Info,
                message: "FIPS status file not found (may not be applicable)".to_string(),
                risk_level: None,
            });
        }

        // Check for FIPS-approved cryptographic libraries
        if let Ok(packages) = g.inspect_packages(root) {
            let has_fips_openssl = packages.packages.iter().any(|pkg| {
                pkg.name.contains("openssl-fips")
                    || pkg.name.contains("openssl") && pkg.name.contains("fips")
            });

            if has_fips_openssl {
                findings.push(Finding {
                    item: "FIPS OpenSSL".to_string(),
                    status: FindingStatus::Pass,
                    message: "FIPS-certified OpenSSL is installed".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "FIPS OpenSSL".to_string(),
                    status: FindingStatus::Warning,
                    message: "FIPS-certified OpenSSL not detected".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }
        }

        // Check kernel cmdline for fips=1
        if let Ok(cmdline) = g.with_mount(root, |guestfs| guestfs.read_file("/proc/cmdline")) {
            let cmdline_str = String::from_utf8_lossy(&cmdline);
            if cmdline_str.contains("fips=1") {
                findings.push(Finding {
                    item: "FIPS Kernel Parameter".to_string(),
                    status: FindingStatus::Pass,
                    message: "Kernel booted with fips=1 parameter".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "FIPS Kernel Parameter".to_string(),
                    status: FindingStatus::Fail,
                    message: "Kernel not booted with fips=1 parameter".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        ReportSection {
            title: "FIPS 140-2 Compliance".to_string(),
            findings,
        }
    }

    /// Password Policy (NIST, PCI-DSS requirements)
    fn audit_password_policy(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check /etc/login.defs for password aging
        if let Ok(login_defs) = g.with_mount(root, |guestfs| guestfs.read_file("/etc/login.defs")) {
            let content = String::from_utf8_lossy(&login_defs);

            // PASS_MAX_DAYS (should be <= 90 per CIS)
            if let Some(line) = content.lines().find(|l| l.starts_with("PASS_MAX_DAYS")) {
                if let Some(days_str) = line.split_whitespace().nth(1) {
                    if let Ok(days) = days_str.parse::<u32>() {
                        if days <= 90 {
                            findings.push(Finding {
                                item: "Password Maximum Age".to_string(),
                                status: FindingStatus::Pass,
                                message: format!("PASS_MAX_DAYS: {} days (compliant)", days),
                                risk_level: Some(RiskLevel::Low),
                            });
                        } else {
                            findings.push(Finding {
                                item: "Password Maximum Age".to_string(),
                                status: FindingStatus::Fail,
                                message: format!("PASS_MAX_DAYS: {} days (should be <= 90)", days),
                                risk_level: Some(RiskLevel::Medium),
                            });
                        }
                    }
                }
            }

            // PASS_MIN_DAYS (should be >= 1)
            if let Some(line) = content.lines().find(|l| l.starts_with("PASS_MIN_DAYS")) {
                if let Some(days_str) = line.split_whitespace().nth(1) {
                    if let Ok(days) = days_str.parse::<u32>() {
                        if days >= 1 {
                            findings.push(Finding {
                                item: "Password Minimum Age".to_string(),
                                status: FindingStatus::Pass,
                                message: format!("PASS_MIN_DAYS: {} days (compliant)", days),
                                risk_level: Some(RiskLevel::Low),
                            });
                        } else {
                            findings.push(Finding {
                                item: "Password Minimum Age".to_string(),
                                status: FindingStatus::Fail,
                                message: format!("PASS_MIN_DAYS: {} days (should be >= 1)", days),
                                risk_level: Some(RiskLevel::Medium),
                            });
                        }
                    }
                }
            }

            // PASS_WARN_AGE (should be >= 7)
            if let Some(line) = content.lines().find(|l| l.starts_with("PASS_WARN_AGE")) {
                if let Some(days_str) = line.split_whitespace().nth(1) {
                    if let Ok(days) = days_str.parse::<u32>() {
                        if days >= 7 {
                            findings.push(Finding {
                                item: "Password Warning Age".to_string(),
                                status: FindingStatus::Pass,
                                message: format!("PASS_WARN_AGE: {} days (compliant)", days),
                                risk_level: Some(RiskLevel::Low),
                            });
                        } else {
                            findings.push(Finding {
                                item: "Password Warning Age".to_string(),
                                status: FindingStatus::Warning,
                                message: format!("PASS_WARN_AGE: {} days (should be >= 7)", days),
                                risk_level: Some(RiskLevel::Low),
                            });
                        }
                    }
                }
            }
        }

        // Check PAM password quality requirements
        if let Ok(pam_pwquality) = g.with_mount(root, |guestfs| {
            guestfs
                .read_file("/etc/security/pwquality.conf")
                .or_else(|_| guestfs.read_file("/etc/pam.d/system-auth"))
        }) {
            let content = String::from_utf8_lossy(&pam_pwquality);

            if content.contains("minlen") || content.contains("pam_pwquality") {
                findings.push(Finding {
                    item: "Password Complexity".to_string(),
                    status: FindingStatus::Pass,
                    message: "Password quality requirements are configured".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "Password Complexity".to_string(),
                    status: FindingStatus::Fail,
                    message: "Password quality requirements not found".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        ReportSection {
            title: "Password Policy".to_string(),
            findings,
        }
    }

    /// Audit Logging (Required for HIPAA, PCI-DSS, SOX)
    fn audit_logging(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check if rsyslog or syslog-ng is installed
        if let Ok(packages) = g.inspect_packages(root) {
            let has_syslog = packages
                .packages
                .iter()
                .any(|pkg| pkg.name.contains("rsyslog") || pkg.name.contains("syslog-ng"));

            if has_syslog {
                findings.push(Finding {
                    item: "System Logging".to_string(),
                    status: FindingStatus::Pass,
                    message: "System logging daemon is installed".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "System Logging".to_string(),
                    status: FindingStatus::Fail,
                    message: "No system logging daemon found".to_string(),
                    risk_level: Some(RiskLevel::Critical),
                });
            }
        }

        // Check if auditd is running (from services)
        if let Ok(services) = g.inspect_systemd_services(root) {
            let auditd_enabled = services
                .iter()
                .any(|svc| svc.name.contains("auditd") && svc.enabled);

            if auditd_enabled {
                findings.push(Finding {
                    item: "Audit Daemon".to_string(),
                    status: FindingStatus::Pass,
                    message: "auditd service is enabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "Audit Daemon".to_string(),
                    status: FindingStatus::Fail,
                    message: "auditd service is not enabled".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        // Check for remote logging configuration
        if let Ok(rsyslog_conf) =
            g.with_mount(root, |guestfs| guestfs.read_file("/etc/rsyslog.conf"))
        {
            let content = String::from_utf8_lossy(&rsyslog_conf);
            if content.contains("@@") || content.contains("@") {
                findings.push(Finding {
                    item: "Remote Logging".to_string(),
                    status: FindingStatus::Pass,
                    message: "Remote logging is configured".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "Remote Logging".to_string(),
                    status: FindingStatus::Warning,
                    message: "Remote logging is not configured".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }
        }

        ReportSection {
            title: "Audit Logging".to_string(),
            findings,
        }
    }

    /// File Permissions (Sensitive Files)
    fn audit_file_permissions(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check /etc/passwd permissions
        if let Ok(stat) = g.with_mount(root, |guestfs| guestfs.stat("/etc/passwd")) {
            let mode = stat.mode & 0o777;
            if mode == 0o644 {
                findings.push(Finding {
                    item: "/etc/passwd permissions".to_string(),
                    status: FindingStatus::Pass,
                    message: "Correct permissions (0644)".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "/etc/passwd permissions".to_string(),
                    status: FindingStatus::Fail,
                    message: format!("Incorrect permissions: {:o} (should be 0644)", mode),
                    risk_level: Some(RiskLevel::High),
                });
            }
        }

        // Check /etc/shadow permissions
        if let Ok(stat) = g.with_mount(root, |guestfs| guestfs.stat("/etc/shadow")) {
            let mode = stat.mode & 0o777;
            if mode == 0o000 || mode == 0o400 || mode == 0o600 {
                findings.push(Finding {
                    item: "/etc/shadow permissions".to_string(),
                    status: FindingStatus::Pass,
                    message: format!("Secure permissions ({:o})", mode),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "/etc/shadow permissions".to_string(),
                    status: FindingStatus::Fail,
                    message: format!(
                        "Insecure permissions: {:o} (should be 0000, 0400, or 0600)",
                        mode
                    ),
                    risk_level: Some(RiskLevel::Critical),
                });
            }
        }

        // Check /etc/gshadow permissions
        if let Ok(stat) = g.with_mount(root, |guestfs| guestfs.stat("/etc/gshadow")) {
            let mode = stat.mode & 0o777;
            if mode == 0o000 || mode == 0o400 || mode == 0o600 {
                findings.push(Finding {
                    item: "/etc/gshadow permissions".to_string(),
                    status: FindingStatus::Pass,
                    message: format!("Secure permissions ({:o})", mode),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "/etc/gshadow permissions".to_string(),
                    status: FindingStatus::Fail,
                    message: format!(
                        "Insecure permissions: {:o} (should be 0000, 0400, or 0600)",
                        mode
                    ),
                    risk_level: Some(RiskLevel::Critical),
                });
            }
        }

        ReportSection {
            title: "File Permissions".to_string(),
            findings,
        }
    }

    /// Calculate overall risk level from all sections
    fn calculate_risk(&self, sections: &[ReportSection]) -> RiskLevel {
        let mut has_critical = false;
        let mut has_high = false;
        let mut has_medium = false;

        for section in sections {
            for finding in &section.findings {
                if let Some(risk) = &finding.risk_level {
                    match risk {
                        RiskLevel::Critical => has_critical = true,
                        RiskLevel::High => has_high = true,
                        RiskLevel::Medium => has_medium = true,
                        _ => {}
                    }
                }
            }
        }

        if has_critical {
            RiskLevel::Critical
        } else if has_high {
            RiskLevel::High
        } else if has_medium {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}
