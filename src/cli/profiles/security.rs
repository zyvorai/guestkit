// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Security audit profile

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection, RiskLevel};
use crate::Guestfs;
use anyhow::Result;

pub struct SecurityProfile;

impl InspectionProfile for SecurityProfile {
    fn name(&self) -> &str {
        "security"
    }

    fn description(&self) -> &str {
        "Security posture assessment and hardening recommendations"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            // Section 1: SSH Configuration
            self.audit_ssh(g, root),
            // Section 2: User Security
            self.audit_users(g, root),
            // Section 3: Firewall & Network Security
            self.audit_firewall(g, root),
            // Section 4: Mandatory Access Control (SELinux/AppArmor)
            self.audit_mac(g, root),
            // Section 5: Services Security
            self.audit_services(g, root),
            // Section 6: SSL/TLS Certificates
            self.audit_certificates(g, root),
        ];

        // Calculate overall risk
        let overall_risk = self.calculate_risk(&sections);

        Ok(ProfileReport {
            profile_name: "Security Audit".to_string(),
            sections,
            overall_risk: Some(overall_risk),
            summary: Some(format!(
                "Overall security risk level: {}. Review critical and high-risk findings.",
                overall_risk
            )),
        })
    }
}

impl SecurityProfile {
    fn audit_ssh(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(ssh_config) = g.inspect_ssh_config(root) {
            // Check PermitRootLogin
            if let Some(permit_root) = ssh_config.get("PermitRootLogin") {
                if permit_root == "yes" {
                    findings.push(Finding {
                        item: "SSH Root Login".to_string(),
                        status: FindingStatus::Fail,
                        message: format!("PermitRootLogin: {} (should be 'no')", permit_root),
                        risk_level: Some(RiskLevel::Critical),
                    });
                } else {
                    findings.push(Finding {
                        item: "SSH Root Login".to_string(),
                        status: FindingStatus::Pass,
                        message: format!("PermitRootLogin: {}", permit_root),
                        risk_level: Some(RiskLevel::Low),
                    });
                }
            }

            // Check PasswordAuthentication
            if let Some(password_auth) = ssh_config.get("PasswordAuthentication") {
                if password_auth == "yes" {
                    findings.push(Finding {
                        item: "SSH Password Authentication".to_string(),
                        status: FindingStatus::Warning,
                        message: format!(
                            "PasswordAuthentication: {} (consider using key-based auth only)",
                            password_auth
                        ),
                        risk_level: Some(RiskLevel::Medium),
                    });
                } else {
                    findings.push(Finding {
                        item: "SSH Password Authentication".to_string(),
                        status: FindingStatus::Pass,
                        message: format!("PasswordAuthentication: {}", password_auth),
                        risk_level: Some(RiskLevel::Low),
                    });
                }
            }

            // Check SSH port
            if let Some(port) = ssh_config.get("Port") {
                if port == "22" {
                    findings.push(Finding {
                        item: "SSH Port".to_string(),
                        status: FindingStatus::Info,
                        message: "Port: 22 (default - consider non-standard port)".to_string(),
                        risk_level: Some(RiskLevel::Low),
                    });
                } else {
                    findings.push(Finding {
                        item: "SSH Port".to_string(),
                        status: FindingStatus::Pass,
                        message: format!("Port: {} (non-standard)", port),
                        risk_level: Some(RiskLevel::Low),
                    });
                }
            }
        } else {
            findings.push(Finding {
                item: "SSH Configuration".to_string(),
                status: FindingStatus::Info,
                message: "SSH not installed or configured".to_string(),
                risk_level: None,
            });
        }

        ReportSection {
            title: "SSH Configuration".to_string(),
            findings,
        }
    }

    fn audit_users(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(users) = g.inspect_users(root) {
            // Count users with UID 0 (root equivalents)
            let root_users: Vec<_> = users.iter().filter(|u| u.uid == "0").collect();

            if root_users.len() > 1 {
                findings.push(Finding {
                    item: "Root-Equivalent Users".to_string(),
                    status: FindingStatus::Warning,
                    message: format!(
                        "{} users with UID 0: {}",
                        root_users.len(),
                        root_users
                            .iter()
                            .map(|u| &u.username)
                            .cloned()
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    risk_level: Some(RiskLevel::High),
                });
            } else {
                findings.push(Finding {
                    item: "Root-Equivalent Users".to_string(),
                    status: FindingStatus::Pass,
                    message: "Only 'root' user has UID 0".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            }

            // Check for users with no password (empty shell)
            let no_password_users = users
                .iter()
                .filter(|u| u.shell.contains("nologin") || u.shell.contains("false"))
                .count();

            findings.push(Finding {
                item: "System Users".to_string(),
                status: FindingStatus::Info,
                message: format!("{} users with disabled login", no_password_users),
                risk_level: None,
            });

            // Total user count
            findings.push(Finding {
                item: "Total User Accounts".to_string(),
                status: FindingStatus::Info,
                message: format!("{} total users on system", users.len()),
                risk_level: None,
            });
        } else {
            findings.push(Finding {
                item: "User Enumeration".to_string(),
                status: FindingStatus::Warning,
                message: "Unable to read user accounts".to_string(),
                risk_level: Some(RiskLevel::Medium),
            });
        }

        ReportSection {
            title: "User Account Security".to_string(),
            findings,
        }
    }

    fn audit_firewall(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check if firewalld/iptables/ufw is present
        if g.mount(root, "/").is_ok() {
            let firewall_services = vec!["firewalld", "iptables", "ufw"];
            let mut firewall_found = false;

            for fw in firewall_services {
                if let Ok(services) = g.inspect_systemd_services(root) {
                    if services.iter().any(|s| s.name.contains(fw)) {
                        firewall_found = true;
                        findings.push(Finding {
                            item: format!("{} Service", fw),
                            status: FindingStatus::Pass,
                            message: format!("{} detected", fw),
                            risk_level: Some(RiskLevel::Low),
                        });
                        break;
                    }
                }
            }

            if !firewall_found {
                findings.push(Finding {
                    item: "Firewall".to_string(),
                    status: FindingStatus::Fail,
                    message: "No firewall service detected (firewalld/iptables/ufw)".to_string(),
                    risk_level: Some(RiskLevel::Critical),
                });
            }

            g.umount("/").ok();
        }

        ReportSection {
            title: "Firewall & Network Security".to_string(),
            findings,
        }
    }

    fn audit_mac(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check SELinux
        if let Ok(selinux) = g.inspect_selinux(root) {
            match selinux.as_str() {
                "enforcing" => findings.push(Finding {
                    item: "SELinux".to_string(),
                    status: FindingStatus::Pass,
                    message: "SELinux: enforcing".to_string(),
                    risk_level: Some(RiskLevel::Low),
                }),
                "permissive" => findings.push(Finding {
                    item: "SELinux".to_string(),
                    status: FindingStatus::Warning,
                    message: "SELinux: permissive (should be enforcing)".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                }),
                "disabled" => findings.push(Finding {
                    item: "SELinux".to_string(),
                    status: FindingStatus::Fail,
                    message: "SELinux: disabled (should be enforcing)".to_string(),
                    risk_level: Some(RiskLevel::High),
                }),
                _ => findings.push(Finding {
                    item: "SELinux".to_string(),
                    status: FindingStatus::Info,
                    message: format!("SELinux: {}", selinux),
                    risk_level: None,
                }),
            }
        } else {
            findings.push(Finding {
                item: "Mandatory Access Control".to_string(),
                status: FindingStatus::Info,
                message: "SELinux/AppArmor not detected".to_string(),
                risk_level: None,
            });
        }

        ReportSection {
            title: "Mandatory Access Control".to_string(),
            findings,
        }
    }

    fn audit_services(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(services) = g.inspect_systemd_services(root) {
            // Check for risky services
            let risky_services = vec!["telnet", "ftp", "rsh", "rlogin", "vsftpd"];

            for risky in &risky_services {
                if let Some(service) = services.iter().find(|s| s.name.contains(risky)) {
                    if service.enabled {
                        findings.push(Finding {
                            item: format!("{} Service", risky),
                            status: FindingStatus::Fail,
                            message: format!(
                                "{} is enabled (HIGH RISK - unencrypted)",
                                service.name
                            ),
                            risk_level: Some(RiskLevel::High),
                        });
                    }
                }
            }

            if findings.is_empty() {
                findings.push(Finding {
                    item: "Insecure Services".to_string(),
                    status: FindingStatus::Pass,
                    message: "No known insecure services enabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            }

            findings.push(Finding {
                item: "Enabled Services Count".to_string(),
                status: FindingStatus::Info,
                message: format!("{} services enabled", services.len()),
                risk_level: None,
            });
        }

        ReportSection {
            title: "Service Security".to_string(),
            findings,
        }
    }

    fn audit_certificates(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(cert_paths) = g.inspect_certificates(root) {
            findings.push(Finding {
                item: "SSL/TLS Certificates".to_string(),
                status: FindingStatus::Info,
                message: format!("{} certificate paths found", cert_paths.len()),
                risk_level: None,
            });

            // Show sample paths
            for (i, cert) in cert_paths.iter().take(3).enumerate() {
                findings.push(Finding {
                    item: format!("Certificate {}", i + 1),
                    status: FindingStatus::Info,
                    message: format!("{} ({})", cert.path, cert.subject),
                    risk_level: None,
                });
            }
        } else {
            findings.push(Finding {
                item: "SSL/TLS Certificates".to_string(),
                status: FindingStatus::Info,
                message: "No certificates found or unable to read".to_string(),
                risk_level: None,
            });
        }

        ReportSection {
            title: "SSL/TLS Certificates".to_string(),
            findings,
        }
    }

    fn calculate_risk(&self, sections: &[ReportSection]) -> RiskLevel {
        let mut critical = 0;
        let mut high = 0;
        let mut medium = 0;

        for section in sections {
            for finding in &section.findings {
                if let Some(risk) = finding.risk_level {
                    match risk {
                        RiskLevel::Critical => critical += 1,
                        RiskLevel::High => high += 1,
                        RiskLevel::Medium => medium += 1,
                        _ => {}
                    }
                }
            }
        }

        if critical > 0 {
            RiskLevel::Critical
        } else if high > 0 {
            RiskLevel::High
        } else if medium > 0 {
            RiskLevel::Medium
        } else {
            RiskLevel::Low
        }
    }
}
