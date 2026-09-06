// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Policy definitions and loading

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

/// Security/compliance policy
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Policy {
    pub name: String,
    pub version: String,
    pub description: String,
    pub rules: Vec<PolicyRule>,
}

/// Individual policy rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyRule {
    pub id: String,
    pub name: String,
    pub description: String,
    pub severity: String,
    #[serde(flatten)]
    pub rule_type: RuleType,
    /// Expression over evidence snapshot (e.g. `security.ssh.root_login == false`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expr: Option<String>,
    pub remediation: Option<String>,
}

/// Types of validation rules
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RuleType {
    PackageInstalled {
        package: String,
    },
    PackageForbidden {
        package: String,
    },
    FileExists {
        path: String,
    },
    FileNotExists {
        path: String,
    },
    FileContains {
        path: String,
        pattern: String,
    },
    FilePermissions {
        path: String,
        mode: String,
    },
    ServiceEnabled {
        service: String,
    },
    ServiceDisabled {
        service: String,
    },
    UserExists {
        username: String,
    },
    UserNotExists {
        username: String,
    },
    PortClosed {
        port: u16,
    },
    Custom {
        check: String,
    },
    /// Expression-based rule over evidence fields
    Expression {
        expr: String,
    },
}

impl Policy {
    /// Load policy from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let policy: Policy = serde_yaml::from_str(&content)?;
        Ok(policy)
    }

    /// Create example policy
    pub fn example() -> Self {
        Self {
            name: "Example Security Policy".to_string(),
            version: "1.0.0".to_string(),
            description: "Example policy for demonstration".to_string(),
            rules: vec![
                PolicyRule {
                    id: "PKG-001".to_string(),
                    name: "OpenSSH Server Installed".to_string(),
                    description: "Ensure OpenSSH server is installed".to_string(),
                    severity: "medium".to_string(),
                    rule_type: RuleType::PackageInstalled {
                        package: "openssh-server".to_string(),
                    },
                    expr: None,
                    remediation: Some("Install openssh-server package".to_string()),
                },
                PolicyRule {
                    id: "PKG-002".to_string(),
                    name: "Telnet Not Installed".to_string(),
                    description: "Ensure telnet is not installed".to_string(),
                    severity: "high".to_string(),
                    rule_type: RuleType::PackageForbidden {
                        package: "telnet".to_string(),
                    },
                    expr: None,
                    remediation: Some("Remove telnet package".to_string()),
                },
                PolicyRule {
                    id: "FILE-001".to_string(),
                    name: "Password File Exists".to_string(),
                    description: "Ensure /etc/passwd exists".to_string(),
                    severity: "critical".to_string(),
                    rule_type: RuleType::FileExists {
                        path: "/etc/passwd".to_string(),
                    },
                    expr: None,
                    remediation: None,
                },
                PolicyRule {
                    id: "PERM-001".to_string(),
                    name: "SSH Config Permissions".to_string(),
                    description: "Ensure /etc/ssh/sshd_config has correct permissions".to_string(),
                    severity: "high".to_string(),
                    rule_type: RuleType::FilePermissions {
                        path: "/etc/ssh/sshd_config".to_string(),
                        mode: "600".to_string(),
                    },
                    expr: None,
                    remediation: Some("chmod 600 /etc/ssh/sshd_config".to_string()),
                },
                PolicyRule {
                    id: "SVC-001".to_string(),
                    name: "SSH Service Enabled".to_string(),
                    description: "Ensure SSH service is enabled".to_string(),
                    severity: "medium".to_string(),
                    rule_type: RuleType::ServiceEnabled {
                        service: "sshd".to_string(),
                    },
                    expr: None,
                    remediation: Some("systemctl enable sshd".to_string()),
                },
                PolicyRule {
                    id: "USER-001".to_string(),
                    name: "Root User Exists".to_string(),
                    description: "Ensure root user exists".to_string(),
                    severity: "critical".to_string(),
                    rule_type: RuleType::UserExists {
                        username: "root".to_string(),
                    },
                    expr: None,
                    remediation: None,
                },
                PolicyRule {
                    id: "EXPR-001".to_string(),
                    name: "SSH root login disabled".to_string(),
                    description: "Root SSH login must be disabled".to_string(),
                    severity: "high".to_string(),
                    rule_type: RuleType::Custom {
                        check: "ssh_root_login".to_string(),
                    },
                    expr: Some("security.ssh.root_login == false".to_string()),
                    remediation: Some("Set PermitRootLogin no in sshd_config".to_string()),
                },
            ],
        }
    }

    /// Save policy to YAML file
    #[allow(dead_code)]
    pub fn to_file<P: AsRef<Path>>(&self, path: P) -> Result<()> {
        let yaml = serde_yaml::to_string(self)?;
        fs::write(path, yaml)?;
        Ok(())
    }
}
