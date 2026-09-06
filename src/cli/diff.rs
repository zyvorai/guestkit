// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! VM comparison and diff functionality

use super::formatters::InspectionReport;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// Diff between two inspection reports
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InspectionDiff {
    pub os_changes: Vec<Change>,
    pub package_changes: PackageChanges,
    pub service_changes: ServiceChanges,
    pub user_changes: UserChanges,
    pub network_changes: Vec<Change>,
    pub config_changes: Vec<Change>,
}

/// Individual change record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Change {
    pub field: String,
    pub old_value: String,
    pub new_value: String,
}

/// Package-specific changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
    pub updated: Vec<PackageUpdate>,
}

/// Package update record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageUpdate {
    pub name: String,
    pub old_version: String,
    pub new_version: String,
}

/// Service changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceChanges {
    pub enabled: Vec<String>,
    pub disabled: Vec<String>,
}

/// User changes
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserChanges {
    pub added: Vec<String>,
    pub removed: Vec<String>,
}

impl InspectionDiff {
    /// Compute diff between two inspection reports
    pub fn compute(report1: &InspectionReport, report2: &InspectionReport) -> Self {
        let mut diff = InspectionDiff {
            os_changes: Vec::new(),
            package_changes: PackageChanges {
                added: Vec::new(),
                removed: Vec::new(),
                updated: Vec::new(),
            },
            service_changes: ServiceChanges {
                enabled: Vec::new(),
                disabled: Vec::new(),
            },
            user_changes: UserChanges {
                added: Vec::new(),
                removed: Vec::new(),
            },
            network_changes: Vec::new(),
            config_changes: Vec::new(),
        };

        // Compare OS details
        if let (Some(os1), Some(os2)) = (&report1.os.hostname, &report2.os.hostname) {
            if os1 != os2 {
                diff.os_changes.push(Change {
                    field: "hostname".to_string(),
                    old_value: os1.clone(),
                    new_value: os2.clone(),
                });
            }
        }

        if let (Some(v1), Some(v2)) = (&report1.os.version, &report2.os.version) {
            if v1.major != v2.major || v1.minor != v2.minor {
                diff.os_changes.push(Change {
                    field: "version".to_string(),
                    old_value: format!("{}.{}", v1.major, v1.minor),
                    new_value: format!("{}.{}", v2.major, v2.minor),
                });
            }
        }

        // Compare packages (kernels as proxy since we don't have full package lists in report)
        if let (Some(p1), Some(p2)) = (&report1.packages, &report2.packages) {
            let kernels1: HashSet<_> = p1.kernels.iter().collect();
            let kernels2: HashSet<_> = p2.kernels.iter().collect();

            for kernel in kernels2.difference(&kernels1) {
                diff.package_changes
                    .added
                    .push(format!("kernel: {}", kernel));
            }

            for kernel in kernels1.difference(&kernels2) {
                diff.package_changes
                    .removed
                    .push(format!("kernel: {}", kernel));
            }

            if p1.count != p2.count {
                diff.os_changes.push(Change {
                    field: "package_count".to_string(),
                    old_value: p1.count.to_string(),
                    new_value: p2.count.to_string(),
                });
            }
        }

        // Compare services
        if let (Some(s1), Some(s2)) = (&report1.services, &report2.services) {
            let services1: HashSet<_> = s1.enabled_services.iter().map(|s| &s.name).collect();
            let services2: HashSet<_> = s2.enabled_services.iter().map(|s| &s.name).collect();

            for service in services2.difference(&services1) {
                diff.service_changes.enabled.push((*service).clone());
            }

            for service in services1.difference(&services2) {
                diff.service_changes.disabled.push((*service).clone());
            }
        }

        // Compare users
        if let (Some(u1), Some(u2)) = (&report1.users, &report2.users) {
            let users1: HashSet<_> = u1.regular_users.iter().map(|u| &u.username).collect();
            let users2: HashSet<_> = u2.regular_users.iter().map(|u| &u.username).collect();

            for user in users2.difference(&users1) {
                diff.user_changes.added.push((*user).clone());
            }

            for user in users1.difference(&users2) {
                diff.user_changes.removed.push((*user).clone());
            }
        }

        // Compare network
        if let (Some(n1), Some(n2)) = (&report1.network, &report2.network) {
            if let (Some(ifaces1), Some(ifaces2)) = (&n1.interfaces, &n2.interfaces) {
                // Simple comparison - check if interface count changed
                if ifaces1.len() != ifaces2.len() {
                    diff.network_changes.push(Change {
                        field: "interface_count".to_string(),
                        old_value: ifaces1.len().to_string(),
                        new_value: ifaces2.len().to_string(),
                    });
                }

                // Check for IP address changes on matching interfaces
                for iface1 in ifaces1 {
                    if let Some(iface2) = ifaces2.iter().find(|i| i.name == iface1.name) {
                        if iface1.ip_address != iface2.ip_address {
                            diff.network_changes.push(Change {
                                field: format!("{}_ip", iface1.name),
                                old_value: iface1.ip_address.join(", "),
                                new_value: iface2.ip_address.join(", "),
                            });
                        }
                    }
                }
            }
        }

        // Compare system config
        if let (Some(c1), Some(c2)) = (&report1.system_config, &report2.system_config) {
            if c1.timezone != c2.timezone {
                if let (Some(tz1), Some(tz2)) = (&c1.timezone, &c2.timezone) {
                    diff.config_changes.push(Change {
                        field: "timezone".to_string(),
                        old_value: tz1.clone(),
                        new_value: tz2.clone(),
                    });
                }
            }

            if c1.selinux != c2.selinux {
                if let (Some(se1), Some(se2)) = (&c1.selinux, &c2.selinux) {
                    diff.config_changes.push(Change {
                        field: "selinux".to_string(),
                        old_value: se1.clone(),
                        new_value: se2.clone(),
                    });
                }
            }
        }

        diff
    }

    /// Print diff in human-readable format
    pub fn print(&self) {
        let mut has_changes = false;

        // OS Changes
        if !self.os_changes.is_empty() {
            println!("\n=== OS Differences ===");
            has_changes = true;
            for change in &self.os_changes {
                println!(
                    "  {}: {} → {}",
                    change.field, change.old_value, change.new_value
                );
            }
        }

        // Package Changes
        if !self.package_changes.added.is_empty()
            || !self.package_changes.removed.is_empty()
            || !self.package_changes.updated.is_empty()
        {
            println!("\n=== Package Differences ===");
            has_changes = true;

            if !self.package_changes.added.is_empty() {
                println!("  Added ({}):", self.package_changes.added.len());
                for pkg in &self.package_changes.added {
                    println!("    + {}", pkg);
                }
            }

            if !self.package_changes.removed.is_empty() {
                println!("  Removed ({}):", self.package_changes.removed.len());
                for pkg in &self.package_changes.removed {
                    println!("    - {}", pkg);
                }
            }

            if !self.package_changes.updated.is_empty() {
                println!("  Updated ({}):", self.package_changes.updated.len());
                for pkg in &self.package_changes.updated {
                    println!(
                        "    ~ {}: {} → {}",
                        pkg.name, pkg.old_version, pkg.new_version
                    );
                }
            }
        }

        // Service Changes
        if !self.service_changes.enabled.is_empty() || !self.service_changes.disabled.is_empty() {
            println!("\n=== Service Differences ===");
            has_changes = true;

            if !self.service_changes.enabled.is_empty() {
                println!("  Enabled:");
                for service in &self.service_changes.enabled {
                    println!("    + {}", service);
                }
            }

            if !self.service_changes.disabled.is_empty() {
                println!("  Disabled:");
                for service in &self.service_changes.disabled {
                    println!("    - {}", service);
                }
            }
        }

        // User Changes
        if !self.user_changes.added.is_empty() || !self.user_changes.removed.is_empty() {
            println!("\n=== User Differences ===");
            has_changes = true;

            if !self.user_changes.added.is_empty() {
                println!("  Added:");
                for user in &self.user_changes.added {
                    println!("    + {}", user);
                }
            }

            if !self.user_changes.removed.is_empty() {
                println!("  Removed:");
                for user in &self.user_changes.removed {
                    println!("    - {}", user);
                }
            }
        }

        // Network Changes
        if !self.network_changes.is_empty() {
            println!("\n=== Network Differences ===");
            has_changes = true;
            for change in &self.network_changes {
                println!(
                    "  {}: {} → {}",
                    change.field, change.old_value, change.new_value
                );
            }
        }

        // Config Changes
        if !self.config_changes.is_empty() {
            println!("\n=== Configuration Differences ===");
            has_changes = true;
            for change in &self.config_changes {
                println!(
                    "  {}: {} → {}",
                    change.field, change.old_value, change.new_value
                );
            }
        }

        if !has_changes {
            println!("\nNo differences detected.");
        }
    }

    /// Check if diff is empty (no changes)
    #[allow(dead_code)]
    pub fn is_empty(&self) -> bool {
        self.os_changes.is_empty()
            && self.package_changes.added.is_empty()
            && self.package_changes.removed.is_empty()
            && self.package_changes.updated.is_empty()
            && self.service_changes.enabled.is_empty()
            && self.service_changes.disabled.is_empty()
            && self.user_changes.added.is_empty()
            && self.user_changes.removed.is_empty()
            && self.network_changes.is_empty()
            && self.config_changes.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_change_creation() {
        let change = Change {
            field: "hostname".to_string(),
            old_value: "old-host".to_string(),
            new_value: "new-host".to_string(),
        };

        assert_eq!(change.field, "hostname");
        assert_eq!(change.old_value, "old-host");
        assert_eq!(change.new_value, "new-host");
    }

    #[test]
    fn test_package_update_creation() {
        let update = PackageUpdate {
            name: "nginx".to_string(),
            old_version: "1.18.0".to_string(),
            new_version: "1.20.0".to_string(),
        };

        assert_eq!(update.name, "nginx");
        assert_eq!(update.old_version, "1.18.0");
        assert_eq!(update.new_version, "1.20.0");
    }

    #[test]
    fn test_package_changes_creation() {
        let changes = PackageChanges {
            added: vec!["docker".to_string(), "kubernetes".to_string()],
            removed: vec!["apache2".to_string()],
            updated: vec![PackageUpdate {
                name: "nginx".to_string(),
                old_version: "1.18.0".to_string(),
                new_version: "1.20.0".to_string(),
            }],
        };

        assert_eq!(changes.added.len(), 2);
        assert_eq!(changes.removed.len(), 1);
        assert_eq!(changes.updated.len(), 1);
    }

    #[test]
    fn test_service_changes_creation() {
        let changes = ServiceChanges {
            enabled: vec!["docker".to_string(), "nginx".to_string()],
            disabled: vec!["apache2".to_string()],
        };

        assert_eq!(changes.enabled.len(), 2);
        assert_eq!(changes.disabled.len(), 1);
    }

    #[test]
    fn test_user_changes_creation() {
        let changes = UserChanges {
            added: vec!["newuser".to_string()],
            removed: vec!["olduser".to_string()],
        };

        assert_eq!(changes.added.len(), 1);
        assert_eq!(changes.removed.len(), 1);
    }

    #[test]
    fn test_inspection_diff_is_empty_true() {
        let diff = InspectionDiff {
            os_changes: vec![],
            package_changes: PackageChanges {
                added: vec![],
                removed: vec![],
                updated: vec![],
            },
            service_changes: ServiceChanges {
                enabled: vec![],
                disabled: vec![],
            },
            user_changes: UserChanges {
                added: vec![],
                removed: vec![],
            },
            network_changes: vec![],
            config_changes: vec![],
        };

        assert!(diff.is_empty());
    }

    #[test]
    fn test_inspection_diff_is_empty_false_with_os_changes() {
        let diff = InspectionDiff {
            os_changes: vec![Change {
                field: "hostname".to_string(),
                old_value: "old".to_string(),
                new_value: "new".to_string(),
            }],
            package_changes: PackageChanges {
                added: vec![],
                removed: vec![],
                updated: vec![],
            },
            service_changes: ServiceChanges {
                enabled: vec![],
                disabled: vec![],
            },
            user_changes: UserChanges {
                added: vec![],
                removed: vec![],
            },
            network_changes: vec![],
            config_changes: vec![],
        };

        assert!(!diff.is_empty());
    }

    #[test]
    fn test_inspection_diff_is_empty_false_with_packages() {
        let diff = InspectionDiff {
            os_changes: vec![],
            package_changes: PackageChanges {
                added: vec!["newpkg".to_string()],
                removed: vec![],
                updated: vec![],
            },
            service_changes: ServiceChanges {
                enabled: vec![],
                disabled: vec![],
            },
            user_changes: UserChanges {
                added: vec![],
                removed: vec![],
            },
            network_changes: vec![],
            config_changes: vec![],
        };

        assert!(!diff.is_empty());
    }

    #[test]
    fn test_inspection_diff_creation() {
        let diff = InspectionDiff {
            os_changes: vec![],
            package_changes: PackageChanges {
                added: vec![],
                removed: vec![],
                updated: vec![],
            },
            service_changes: ServiceChanges {
                enabled: vec![],
                disabled: vec![],
            },
            user_changes: UserChanges {
                added: vec![],
                removed: vec![],
            },
            network_changes: vec![],
            config_changes: vec![],
        };

        assert!(diff.os_changes.is_empty());
        assert!(diff.network_changes.is_empty());
        assert!(diff.config_changes.is_empty());
    }
}
