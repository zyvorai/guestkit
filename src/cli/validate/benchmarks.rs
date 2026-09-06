// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Industry benchmark policies (CIS, NIST, etc.)

use super::policy::{Policy, PolicyRule, RuleType};

/// Supported industry benchmarks
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Benchmark {
    CisUbuntu2004,
    CisRhel8,
    NistCsf,
    PciDss,
    Hipaa,
}

impl Benchmark {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "cis-ubuntu-20.04" | "cis-ubuntu" => Some(Self::CisUbuntu2004),
            "cis-rhel-8" | "cis-rhel" => Some(Self::CisRhel8),
            "nist-csf" | "nist" => Some(Self::NistCsf),
            "pci-dss" | "pci" => Some(Self::PciDss),
            "hipaa" => Some(Self::Hipaa),
            _ => None,
        }
    }

    pub fn to_policy(self) -> Policy {
        match self {
            Self::CisUbuntu2004 => cis_ubuntu_2004_policy(),
            Self::CisRhel8 => cis_rhel8_policy(),
            Self::NistCsf => nist_csf_policy(),
            Self::PciDss => pci_dss_policy(),
            Self::Hipaa => hipaa_policy(),
        }
    }
}

fn cis_ubuntu_2004_policy() -> Policy {
    Policy {
        name: "CIS Ubuntu 20.04 Benchmark".to_string(),
        version: "1.1.0".to_string(),
        description: "Center for Internet Security Ubuntu 20.04 LTS Benchmark".to_string(),
        rules: vec![
            PolicyRule {
                id: "CIS-1.1.1.1".to_string(),
                name: "Ensure mounting of cramfs filesystems is disabled".to_string(),
                description:
                    "The cramfs filesystem type is a compressed read-only Linux filesystem"
                        .to_string(),
                severity: "low".to_string(),
                rule_type: RuleType::FileNotExists {
                    path: "/etc/modprobe.d/cramfs.conf".to_string(),
                },
                expr: None,
                remediation: Some(
                    "echo 'install cramfs /bin/true' > /etc/modprobe.d/cramfs.conf".to_string(),
                ),
            },
            PolicyRule {
                id: "CIS-1.5.1".to_string(),
                name: "Ensure permissions on bootloader config are configured".to_string(),
                description: "Grub configuration must have restricted permissions".to_string(),
                severity: "high".to_string(),
                rule_type: RuleType::FilePermissions {
                    path: "/boot/grub/grub.cfg".to_string(),
                    mode: "400".to_string(),
                },
                expr: None,
                remediation: Some("chmod 400 /boot/grub/grub.cfg".to_string()),
            },
            PolicyRule {
                id: "CIS-5.2.1".to_string(),
                name: "Ensure permissions on /etc/ssh/sshd_config are configured".to_string(),
                description:
                    "The /etc/ssh/sshd_config file must be owned by root with 600 permissions"
                        .to_string(),
                severity: "high".to_string(),
                rule_type: RuleType::FilePermissions {
                    path: "/etc/ssh/sshd_config".to_string(),
                    mode: "600".to_string(),
                },
                expr: None,
                remediation: Some(
                    "chmod 600 /etc/ssh/sshd_config && chown root:root /etc/ssh/sshd_config"
                        .to_string(),
                ),
            },
            PolicyRule {
                id: "CIS-5.2.4".to_string(),
                name: "Ensure SSH root login is disabled".to_string(),
                description: "The PermitRootLogin parameter should be set to no".to_string(),
                severity: "critical".to_string(),
                rule_type: RuleType::FileContains {
                    path: "/etc/ssh/sshd_config".to_string(),
                    pattern: "PermitRootLogin no".to_string(),
                },
                expr: None,
                remediation: Some("Set 'PermitRootLogin no' in /etc/ssh/sshd_config".to_string()),
            },
        ],
    }
}

fn cis_rhel8_policy() -> Policy {
    Policy {
        name: "CIS Red Hat Enterprise Linux 8 Benchmark".to_string(),
        version: "2.0.0".to_string(),
        description: "Center for Internet Security RHEL 8 Benchmark".to_string(),
        rules: vec![
            PolicyRule {
                id: "CIS-1.1.1.1".to_string(),
                name: "Ensure mounting of cramfs filesystems is disabled".to_string(),
                description: "The cramfs filesystem should be disabled".to_string(),
                severity: "low".to_string(),
                rule_type: RuleType::FileNotExists {
                    path: "/etc/modprobe.d/cramfs.conf".to_string(),
                },
                expr: None,
                remediation: Some(
                    "echo 'install cramfs /bin/true' > /etc/modprobe.d/cramfs.conf".to_string(),
                ),
            },
            PolicyRule {
                id: "CIS-1.5.1".to_string(),
                name: "Ensure permissions on bootloader config are configured".to_string(),
                severity: "high".to_string(),
                description: "Bootloader configuration must have restricted permissions"
                    .to_string(),
                rule_type: RuleType::FilePermissions {
                    path: "/boot/grub2/grub.cfg".to_string(),
                    mode: "600".to_string(),
                },
                expr: None,
                remediation: Some("chmod 600 /boot/grub2/grub.cfg".to_string()),
            },
        ],
    }
}

fn nist_csf_policy() -> Policy {
    Policy {
        name: "NIST Cybersecurity Framework".to_string(),
        version: "1.1".to_string(),
        description: "NIST CSF security controls".to_string(),
        rules: vec![
            PolicyRule {
                id: "NIST-PR.AC-1".to_string(),
                name: "Identities and credentials are managed".to_string(),
                description: "User accounts and credentials are properly managed".to_string(),
                severity: "high".to_string(),
                rule_type: RuleType::FileExists {
                    path: "/etc/passwd".to_string(),
                },
                expr: None,
                remediation: None,
            },
            PolicyRule {
                id: "NIST-PR.DS-1".to_string(),
                name: "Data-at-rest is protected".to_string(),
                description: "Ensure data at rest protection mechanisms are in place".to_string(),
                severity: "high".to_string(),
                rule_type: RuleType::PackageInstalled {
                    package: "cryptsetup".to_string(),
                },
                expr: None,
                remediation: Some("Install cryptsetup for disk encryption".to_string()),
            },
        ],
    }
}

fn pci_dss_policy() -> Policy {
    Policy {
        name: "PCI DSS Requirements".to_string(),
        version: "3.2.1".to_string(),
        description: "Payment Card Industry Data Security Standard".to_string(),
        rules: vec![
            PolicyRule {
                id: "PCI-2.2.2".to_string(),
                name: "Enable only necessary services".to_string(),
                description: "Disable all unnecessary services and protocols".to_string(),
                severity: "high".to_string(),
                rule_type: RuleType::PackageForbidden {
                    package: "telnet".to_string(),
                },
                expr: None,
                remediation: Some("Remove telnet and other insecure services".to_string()),
            },
            PolicyRule {
                id: "PCI-2.2.4".to_string(),
                name: "Configure security parameters".to_string(),
                description: "Security parameters must be configured to prevent misuse".to_string(),
                severity: "critical".to_string(),
                rule_type: RuleType::FileContains {
                    path: "/etc/ssh/sshd_config".to_string(),
                    pattern: "PermitRootLogin no".to_string(),
                },
                expr: None,
                remediation: Some("Disable root login via SSH".to_string()),
            },
        ],
    }
}

fn hipaa_policy() -> Policy {
    Policy {
        name: "HIPAA Security Rule".to_string(),
        version: "1.0".to_string(),
        description: "Health Insurance Portability and Accountability Act security controls"
            .to_string(),
        rules: vec![
            PolicyRule {
                id: "HIPAA-164.308".to_string(),
                name: "Access Control".to_string(),
                description:
                    "Implement technical policies and procedures for systems that maintain ePHI"
                        .to_string(),
                severity: "critical".to_string(),
                rule_type: RuleType::FileExists {
                    path: "/etc/passwd".to_string(),
                },
                expr: None,
                remediation: None,
            },
            PolicyRule {
                id: "HIPAA-164.312".to_string(),
                name: "Encryption and Decryption".to_string(),
                description: "Implement a mechanism to encrypt and decrypt ePHI".to_string(),
                severity: "critical".to_string(),
                rule_type: RuleType::PackageInstalled {
                    package: "cryptsetup".to_string(),
                },
                expr: None,
                remediation: Some("Install encryption tools".to_string()),
            },
        ],
    }
}
