// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! System Hardening Profile
//!
//! Provides actionable security hardening recommendations with remediation steps

use super::{Finding, FindingStatus, InspectionProfile, ProfileReport, ReportSection, RiskLevel};
use crate::Guestfs;
use anyhow::Result;

pub struct HardeningProfile;

impl InspectionProfile for HardeningProfile {
    fn name(&self) -> &str {
        "hardening"
    }

    fn description(&self) -> &str {
        "System hardening recommendations with actionable remediation steps"
    }

    fn inspect(&self, g: &mut Guestfs, root: &str) -> Result<ProfileReport> {
        let sections = vec![
            // Section 1: Kernel Hardening (sysctl parameters)
            self.audit_kernel_hardening(g, root),
            // Section 2: Network Hardening
            self.audit_network_hardening(g, root),
            // Section 3: Filesystem Hardening (mount options)
            self.audit_filesystem_hardening(g, root),
            // Section 4: Service Hardening
            self.audit_service_hardening(g, root),
            // Section 5: User Account Hardening
            self.audit_user_hardening(g, root),
        ];

        // Calculate overall risk
        let overall_risk = self.calculate_risk(&sections);

        Ok(ProfileReport {
            profile_name: "System Hardening".to_string(),
            sections,
            overall_risk: Some(overall_risk),
            summary: Some(format!(
                "Overall hardening risk level: {}. Review findings and apply recommended remediations.",
                overall_risk
            )),
        })
    }
}

impl HardeningProfile {
    /// Kernel Hardening - sysctl parameters
    fn audit_kernel_hardening(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check /etc/sysctl.conf and /etc/sysctl.d/
        if let Ok(sysctl_conf) = g.with_mount(root, |guestfs| {
            guestfs
                .read_file("/etc/sysctl.conf")
                .or_else(|_| guestfs.read_file("/etc/sysctl.d/99-sysctl.conf"))
        }) {
            let content = String::from_utf8_lossy(&sysctl_conf);

            // Check kernel.dmesg_restrict (prevent unprivileged access to kernel logs)
            if content.contains("kernel.dmesg_restrict") && content.contains("= 1") {
                findings.push(Finding {
                    item: "kernel.dmesg_restrict".to_string(),
                    status: FindingStatus::Pass,
                    message: "Kernel log access is restricted to privileged users".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "kernel.dmesg_restrict".to_string(),
                    status: FindingStatus::Fail,
                    message: "Kernel logs accessible to unprivileged users".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check kernel.kptr_restrict (hide kernel pointers)
            if content.contains("kernel.kptr_restrict")
                && (content.contains("= 1") || content.contains("= 2"))
            {
                findings.push(Finding {
                    item: "kernel.kptr_restrict".to_string(),
                    status: FindingStatus::Pass,
                    message: "Kernel pointers are hidden from unprivileged users".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "kernel.kptr_restrict".to_string(),
                    status: FindingStatus::Fail,
                    message: "Kernel pointers exposed (information leak risk)".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }

            // Check kernel.yama.ptrace_scope (restrict ptrace)
            if content.contains("kernel.yama.ptrace_scope") && content.contains("= 1") {
                findings.push(Finding {
                    item: "kernel.yama.ptrace_scope".to_string(),
                    status: FindingStatus::Pass,
                    message: "ptrace restricted to parent processes only".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "kernel.yama.ptrace_scope".to_string(),
                    status: FindingStatus::Warning,
                    message: "ptrace not restricted (debugging/injection risk)".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check fs.protected_hardlinks
            if content.contains("fs.protected_hardlinks") && content.contains("= 1") {
                findings.push(Finding {
                    item: "fs.protected_hardlinks".to_string(),
                    status: FindingStatus::Pass,
                    message: "Hardlink creation is protected".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "fs.protected_hardlinks".to_string(),
                    status: FindingStatus::Fail,
                    message: "Hardlink creation not protected (TOCTOU attack risk)".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check fs.protected_symlinks
            if content.contains("fs.protected_symlinks") && content.contains("= 1") {
                findings.push(Finding {
                    item: "fs.protected_symlinks".to_string(),
                    status: FindingStatus::Pass,
                    message: "Symlink following is protected".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "fs.protected_symlinks".to_string(),
                    status: FindingStatus::Fail,
                    message: "Symlink following not protected (privilege escalation risk)"
                        .to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }
        } else {
            findings.push(Finding {
                item: "Kernel Hardening".to_string(),
                status: FindingStatus::Fail,
                message: "No sysctl configuration found".to_string(),
                risk_level: Some(RiskLevel::High),
            });
        }

        ReportSection {
            title: "Kernel Hardening (sysctl)".to_string(),
            findings,
        }
    }

    /// Network Hardening
    fn audit_network_hardening(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(sysctl_conf) = g.with_mount(root, |guestfs| {
            guestfs
                .read_file("/etc/sysctl.conf")
                .or_else(|_| guestfs.read_file("/etc/sysctl.d/99-sysctl.conf"))
        }) {
            let content = String::from_utf8_lossy(&sysctl_conf);

            // IP forwarding should be disabled unless this is a router
            if content.contains("net.ipv4.ip_forward") && content.contains("= 0") {
                findings.push(Finding {
                    item: "net.ipv4.ip_forward".to_string(),
                    status: FindingStatus::Pass,
                    message: "IP forwarding is disabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.ip_forward".to_string(),
                    status: FindingStatus::Warning,
                    message: "IP forwarding may be enabled (unless router)".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // SYN cookies (protection against SYN flood)
            if content.contains("net.ipv4.tcp_syncookies") && content.contains("= 1") {
                findings.push(Finding {
                    item: "net.ipv4.tcp_syncookies".to_string(),
                    status: FindingStatus::Pass,
                    message: "SYN cookies enabled (SYN flood protection)".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.tcp_syncookies".to_string(),
                    status: FindingStatus::Fail,
                    message: "SYN cookies not enabled (vulnerable to SYN floods)".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }

            // Ignore ICMP redirects
            if content.contains("net.ipv4.conf.all.accept_redirects") && content.contains("= 0") {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.accept_redirects".to_string(),
                    status: FindingStatus::Pass,
                    message: "ICMP redirects are ignored".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.accept_redirects".to_string(),
                    status: FindingStatus::Fail,
                    message: "ICMP redirects accepted (MitM attack risk)".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }

            // Source routing disabled
            if content.contains("net.ipv4.conf.all.accept_source_route") && content.contains("= 0")
            {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.accept_source_route".to_string(),
                    status: FindingStatus::Pass,
                    message: "Source routing is disabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.accept_source_route".to_string(),
                    status: FindingStatus::Fail,
                    message: "Source routing enabled (spoofing/hijacking risk)".to_string(),
                    risk_level: Some(RiskLevel::Critical),
                });
            }

            // Reverse path filtering (anti-spoofing)
            if content.contains("net.ipv4.conf.all.rp_filter") && content.contains("= 1") {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.rp_filter".to_string(),
                    status: FindingStatus::Pass,
                    message: "Reverse path filtering enabled (anti-spoofing)".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.rp_filter".to_string(),
                    status: FindingStatus::Fail,
                    message: "Reverse path filtering disabled (IP spoofing risk)".to_string(),
                    risk_level: Some(RiskLevel::High),
                });
            }

            // Log martian packets
            if content.contains("net.ipv4.conf.all.log_martians") && content.contains("= 1") {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.log_martians".to_string(),
                    status: FindingStatus::Pass,
                    message: "Martian packet logging enabled".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "net.ipv4.conf.all.log_martians".to_string(),
                    status: FindingStatus::Warning,
                    message: "Martian packet logging disabled (visibility gap)".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            }
        }

        ReportSection {
            title: "Network Hardening".to_string(),
            findings,
        }
    }

    /// Filesystem Hardening - check mount options
    fn audit_filesystem_hardening(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        // Check fstab for proper mount options
        if let Ok(fstab_entries) = g.inspect_fstab(root) {
            // Check /tmp for noexec, nosuid, nodev
            let tmp_entry = fstab_entries.iter().find(|(_, mp, _)| mp == "/tmp");
            if let Some((_device, _, _fstype)) = tmp_entry {
                // Read actual mount options from fstab
                if let Ok(fstab_content) =
                    g.with_mount(root, |guestfs| guestfs.read_file("/etc/fstab"))
                {
                    let content = String::from_utf8_lossy(&fstab_content);
                    let tmp_line = content
                        .lines()
                        .find(|l| l.contains("/tmp") && !l.trim().starts_with('#'));

                    if let Some(line) = tmp_line {
                        let has_noexec = line.contains("noexec");
                        let has_nosuid = line.contains("nosuid");
                        let has_nodev = line.contains("nodev");

                        if has_noexec && has_nosuid && has_nodev {
                            findings.push(Finding {
                                item: "/tmp mount options".to_string(),
                                status: FindingStatus::Pass,
                                message: "noexec,nosuid,nodev options set".to_string(),
                                risk_level: Some(RiskLevel::Low),
                            });
                        } else {
                            let missing = vec![
                                (!has_noexec).then_some("noexec"),
                                (!has_nosuid).then_some("nosuid"),
                                (!has_nodev).then_some("nodev"),
                            ]
                            .into_iter()
                            .flatten()
                            .collect::<Vec<_>>()
                            .join(",");

                            findings.push(Finding {
                                item: "/tmp mount options".to_string(),
                                status: FindingStatus::Fail,
                                message: format!("Missing hardening options: {}", missing),
                                risk_level: Some(RiskLevel::High),
                            });
                        }
                    }
                }
            } else {
                findings.push(Finding {
                    item: "/tmp mount".to_string(),
                    status: FindingStatus::Warning,
                    message: "/tmp is not a separate partition".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check /var/tmp for noexec, nosuid, nodev
            let var_tmp_entry = fstab_entries.iter().find(|(_, mp, _)| mp == "/var/tmp");
            if var_tmp_entry.is_none() {
                findings.push(Finding {
                    item: "/var/tmp mount".to_string(),
                    status: FindingStatus::Warning,
                    message: "/var/tmp is not a separate partition".to_string(),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check /home for nosuid, nodev
            let home_entry = fstab_entries.iter().find(|(_, mp, _)| mp == "/home");
            if home_entry.is_none() {
                findings.push(Finding {
                    item: "/home mount".to_string(),
                    status: FindingStatus::Info,
                    message: "/home is not a separate partition".to_string(),
                    risk_level: None,
                });
            }
        }

        ReportSection {
            title: "Filesystem Hardening".to_string(),
            findings,
        }
    }

    /// Service Hardening
    fn audit_service_hardening(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(services) = g.inspect_systemd_services(root) {
            // Check for unnecessary services that should be disabled
            let unnecessary_services = vec![
                ("avahi-daemon", "Zeroconf/Bonjour service"),
                ("cups", "Print service"),
                ("rpcbind", "RPC service (unless NFS needed)"),
                ("bluetooth", "Bluetooth service"),
            ];

            for (svc_name, desc) in unnecessary_services {
                let found = services
                    .iter()
                    .any(|s| s.name.contains(svc_name) && s.enabled);
                if found {
                    findings.push(Finding {
                        item: format!("{} service", svc_name),
                        status: FindingStatus::Warning,
                        message: format!("{} is enabled (disable if not needed)", desc),
                        risk_level: Some(RiskLevel::Low),
                    });
                } else {
                    findings.push(Finding {
                        item: format!("{} service", svc_name),
                        status: FindingStatus::Pass,
                        message: format!("{} is not enabled", desc),
                        risk_level: Some(RiskLevel::Low),
                    });
                }
            }

            // Check that essential security services are enabled
            let essential_services =
                vec![("auditd", "Audit daemon"), ("firewalld", "Firewall daemon")];

            for (svc_name, desc) in essential_services {
                let found = services
                    .iter()
                    .any(|s| s.name.contains(svc_name) && s.enabled);
                if found {
                    findings.push(Finding {
                        item: format!("{} service", svc_name),
                        status: FindingStatus::Pass,
                        message: format!("{} is enabled", desc),
                        risk_level: Some(RiskLevel::Low),
                    });
                } else {
                    findings.push(Finding {
                        item: format!("{} service", svc_name),
                        status: FindingStatus::Fail,
                        message: format!("{} is not enabled", desc),
                        risk_level: Some(RiskLevel::High),
                    });
                }
            }
        }

        ReportSection {
            title: "Service Hardening".to_string(),
            findings,
        }
    }

    /// User Account Hardening
    fn audit_user_hardening(&self, g: &mut Guestfs, root: &str) -> ReportSection {
        let mut findings = Vec::new();

        if let Ok(users) = g.inspect_users(root) {
            // Check user count
            let regular_users: Vec<_> = users
                .iter()
                .filter(|u| u.uid.parse::<u32>().unwrap_or(0) >= 1000)
                .collect();

            findings.push(Finding {
                item: "User Accounts".to_string(),
                status: FindingStatus::Info,
                message: format!(
                    "{} total users ({} regular users)",
                    users.len(),
                    regular_users.len()
                ),
                risk_level: None,
            });

            // Check for users with UID 0 (besides root)
            let root_equivalent_users: Vec<_> = users
                .iter()
                .filter(|u| u.uid == "0" && u.username != "root")
                .collect();

            if root_equivalent_users.is_empty() {
                findings.push(Finding {
                    item: "Root-equivalent Users".to_string(),
                    status: FindingStatus::Pass,
                    message: "No non-root users with UID 0".to_string(),
                    risk_level: Some(RiskLevel::Low),
                });
            } else {
                findings.push(Finding {
                    item: "Root-equivalent Users".to_string(),
                    status: FindingStatus::Fail,
                    message: format!(
                        "{} non-root user(s) with UID 0: {}",
                        root_equivalent_users.len(),
                        root_equivalent_users
                            .iter()
                            .map(|u| u.username.as_str())
                            .collect::<Vec<_>>()
                            .join(", ")
                    ),
                    risk_level: Some(RiskLevel::Critical),
                });
            }

            // Check for users without home directories
            let homeless_users: Vec<_> = users
                .iter()
                .filter(|u| u.home.is_empty() || u.home == "/nonexistent")
                .filter(|u| !u.shell.contains("nologin") && !u.shell.contains("false"))
                .collect();

            if !homeless_users.is_empty() {
                findings.push(Finding {
                    item: "Users without Home Directories".to_string(),
                    status: FindingStatus::Warning,
                    message: format!(
                        "{} user(s) without proper home directories",
                        homeless_users.len()
                    ),
                    risk_level: Some(RiskLevel::Medium),
                });
            }

            // Check default umask
            if let Ok(profile_content) = g.with_mount(root, |guestfs| {
                guestfs
                    .read_file("/etc/profile")
                    .or_else(|_| guestfs.read_file("/etc/bash.bashrc"))
            }) {
                let content = String::from_utf8_lossy(&profile_content);
                if content.contains("umask 027") || content.contains("umask 077") {
                    findings.push(Finding {
                        item: "Default umask".to_string(),
                        status: FindingStatus::Pass,
                        message: "Restrictive umask configured (027 or 077)".to_string(),
                        risk_level: Some(RiskLevel::Low),
                    });
                } else if content.contains("umask 022") {
                    findings.push(Finding {
                        item: "Default umask".to_string(),
                        status: FindingStatus::Warning,
                        message: "Permissive umask (022) - consider 027".to_string(),
                        risk_level: Some(RiskLevel::Low),
                    });
                }
            }
        }

        ReportSection {
            title: "User Account Hardening".to_string(),
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
