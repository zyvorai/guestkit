// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration planning and compatibility analysis

pub mod analyzer;
pub mod plan;
pub mod planner;
pub mod reporter;

#[cfg(not(target_os = "windows"))]
use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Migration target type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MigrationTarget {
    OsUpgrade,
    CloudPlatform,
    Containerization,
}

impl MigrationTarget {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "upgrade" | "os" => Some(Self::OsUpgrade),
            "cloud" | "aws" | "azure" | "gcp" => Some(Self::CloudPlatform),
            "container" | "docker" | "kubernetes" => Some(Self::Containerization),
            _ => None,
        }
    }
}

/// Migration risk level
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

impl RiskLevel {
    pub fn emoji(&self) -> &str {
        match self {
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🟠",
            Self::Critical => "🔴",
        }
    }
}

/// Source system information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SourceSystem {
    pub os_name: String,
    pub os_version: String,
    pub os_major: i32,
    pub os_minor: i32,
    pub arch: String,
    pub hostname: String,
    pub kernel: String,
    pub packages: Vec<Package>,
    pub services: Vec<Service>,
    pub filesystems: Vec<Filesystem>,
    pub total_size_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub arch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filesystem {
    pub device: String,
    pub fstype: String,
    pub size_gb: f64,
}

/// Migration plan
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationPlan {
    pub source: SourceSystem,
    pub target_os: String,
    pub target_version: String,
    pub migration_type: String,
    pub overall_risk: RiskLevel,
    pub compatibility_score: f64,
    pub issues: Vec<MigrationIssue>,
    pub package_mappings: Vec<PackageMapping>,
    pub required_changes: Vec<RequiredChange>,
    pub recommendations: Vec<String>,
    pub estimated_effort_hours: u32,
    pub steps: Vec<MigrationStep>,
}

/// Migration issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationIssue {
    pub severity: RiskLevel,
    pub category: String,
    pub description: String,
    pub impact: String,
    pub remediation: String,
}

/// Package mapping between source and target
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMapping {
    pub source_package: String,
    pub target_package: String,
    pub mapping_type: MappingType,
    pub notes: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MappingType {
    DirectMapping,
    NameChange,
    Split,
    Merge,
    NotAvailable,
    AlternativeRequired,
}

/// Required configuration or system change
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequiredChange {
    pub category: String,
    pub description: String,
    pub priority: RiskLevel,
    pub automated: bool,
}

/// Migration step
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationStep {
    pub order: u32,
    pub phase: String,
    pub description: String,
    pub commands: Vec<String>,
    pub validation: String,
    pub rollback: Option<String>,
}

/// Analyze source system
#[cfg(not(target_os = "windows"))]
pub fn analyze_source<P: AsRef<Path>>(image_path: P, verbose: bool) -> Result<SourceSystem> {
    let image_path_str = image_path.as_ref().display().to_string();

    if verbose {
        println!("🔍 Analyzing source system: {}", image_path_str);
    }

    // Initialize guestfs
    let mut g = Guestfs::new()?;
    g.add_drive_opts(&image_path, true, None)?;
    g.launch()?;

    // Inspect OS
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        anyhow::bail!("No operating systems found in disk image");
    }

    let root = &roots[0];

    // Mount filesystems
    let mountpoints = g.inspect_get_mountpoints(root)?;
    for (mp, dev) in mountpoints {
        let _ = g.mount(&dev, &mp);
    }

    // Get OS information
    let os_name = g.inspect_get_product_name(root)?;
    let os_major = g.inspect_get_major_version(root)?;
    let os_minor = g.inspect_get_minor_version(root)?;
    let arch = g.inspect_get_arch(root)?;
    let hostname = g
        .inspect_get_hostname(root)
        .unwrap_or_else(|_| "unknown".to_string());

    // Get kernel version
    let kernel = if g.is_file("/proc/version").unwrap_or(false) {
        g.cat("/proc/version")
            .unwrap_or_else(|_| "unknown".to_string())
            .lines()
            .next()
            .unwrap_or("unknown")
            .to_string()
    } else {
        "unknown".to_string()
    };

    // Get packages
    let applications = g.inspect_list_applications2(root)?;
    let mut packages = Vec::new();
    for (name, version, _release) in applications {
        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
            arch: arch.clone(),
        });
    }

    if verbose {
        println!("  Found {} packages", packages.len());
    }

    // Get services
    let services = detect_services(&mut g, verbose);

    // Get filesystems
    let filesystems = detect_filesystems(&mut g);
    let total_size_gb: f64 = filesystems.iter().map(|f| f.size_gb).sum();

    g.shutdown()?;

    Ok(SourceSystem {
        os_name,
        os_version: format!("{}.{}", os_major, os_minor),
        os_major,
        os_minor,
        arch,
        hostname,
        kernel,
        packages,
        services,
        filesystems,
        total_size_gb,
    })
}

#[cfg(not(target_os = "windows"))]
fn detect_services(g: &mut Guestfs, verbose: bool) -> Vec<Service> {
    let mut services = Vec::new();

    if verbose {
        println!("  Detecting services...");
    }

    // Common critical services
    for service_name in &[
        "sshd",
        "nginx",
        "apache2",
        "httpd",
        "mysql",
        "mariadb",
        "postgresql",
        "redis",
        "docker",
        "kubelet",
    ] {
        let service_file = format!("/lib/systemd/system/{}.service", service_name);
        if g.is_file(&service_file).unwrap_or(false) {
            services.push(Service {
                name: service_name.to_string(),
                enabled: true,
            });
        }
    }

    services
}

#[cfg(not(target_os = "windows"))]
fn detect_filesystems(g: &mut Guestfs) -> Vec<Filesystem> {
    let mut filesystems = Vec::new();

    if let Ok(list) = g.list_filesystems() {
        for (device, fstype) in list {
            if fstype != "unknown" && !fstype.is_empty() {
                let size_bytes = g.blockdev_getsize64(&device).unwrap_or(0);
                let size_gb = size_bytes as f64 / 1_073_741_824.0;

                filesystems.push(Filesystem {
                    device,
                    fstype,
                    size_gb,
                });
            }
        }
    }

    filesystems
}

/// Plan migration
pub fn plan_migration(
    source: &SourceSystem,
    target_os: &str,
    target_version: &str,
    migration_type: MigrationTarget,
) -> Result<MigrationPlan> {
    match migration_type {
        MigrationTarget::OsUpgrade => planner::plan_os_upgrade(source, target_os, target_version),
        MigrationTarget::CloudPlatform => planner::plan_cloud_migration(source, target_os),
        MigrationTarget::Containerization => planner::plan_containerization(source),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_migration_target_from_str() {
        assert_eq!(
            MigrationTarget::from_str("upgrade"),
            Some(MigrationTarget::OsUpgrade)
        );
        assert_eq!(
            MigrationTarget::from_str("os"),
            Some(MigrationTarget::OsUpgrade)
        );
        assert_eq!(
            MigrationTarget::from_str("cloud"),
            Some(MigrationTarget::CloudPlatform)
        );
        assert_eq!(
            MigrationTarget::from_str("aws"),
            Some(MigrationTarget::CloudPlatform)
        );
        assert_eq!(
            MigrationTarget::from_str("container"),
            Some(MigrationTarget::Containerization)
        );
        assert_eq!(
            MigrationTarget::from_str("docker"),
            Some(MigrationTarget::Containerization)
        );
        assert_eq!(MigrationTarget::from_str("invalid"), None);
    }

    #[test]
    fn test_risk_level_emoji() {
        assert_eq!(RiskLevel::Low.emoji(), "🟢");
        assert_eq!(RiskLevel::Medium.emoji(), "🟡");
        assert_eq!(RiskLevel::High.emoji(), "🟠");
        assert_eq!(RiskLevel::Critical.emoji(), "🔴");
    }

    fn create_test_source() -> SourceSystem {
        SourceSystem {
            os_name: "Ubuntu".to_string(),
            os_version: "20.04".to_string(),
            os_major: 20,
            os_minor: 4,
            arch: "x86_64".to_string(),
            hostname: "test-host".to_string(),
            kernel: "Linux 5.4.0".to_string(),
            packages: vec![
                Package {
                    name: "nginx".to_string(),
                    version: "1.18.0".to_string(),
                    arch: "amd64".to_string(),
                },
                Package {
                    name: "python3".to_string(),
                    version: "3.8.0".to_string(),
                    arch: "amd64".to_string(),
                },
            ],
            services: vec![Service {
                name: "nginx".to_string(),
                enabled: true,
            }],
            filesystems: vec![Filesystem {
                device: "/dev/sda1".to_string(),
                fstype: "ext4".to_string(),
                size_gb: 20.0,
            }],
            total_size_gb: 20.0,
        }
    }

    #[test]
    fn test_plan_os_upgrade() {
        let source = create_test_source();
        let result = planner::plan_os_upgrade(&source, "Ubuntu", "22.04");

        assert!(result.is_ok());
        let plan = result.unwrap();

        assert_eq!(plan.target_os, "Ubuntu");
        assert_eq!(plan.target_version, "22.04");
        assert_eq!(plan.migration_type, "OS Upgrade");
        assert!(plan.compatibility_score >= 0.0 && plan.compatibility_score <= 100.0);
        assert!(!plan.steps.is_empty());
    }

    #[test]
    fn test_plan_cloud_migration() {
        let source = create_test_source();
        let result = planner::plan_cloud_migration(&source, "aws");

        assert!(result.is_ok());
        let plan = result.unwrap();

        assert_eq!(plan.target_os, "aws");
        assert_eq!(plan.migration_type, "Cloud Migration");
        assert!(!plan.required_changes.is_empty());
        assert!(!plan.recommendations.is_empty());
    }

    #[test]
    fn test_plan_containerization() {
        let source = create_test_source();
        let result = planner::plan_containerization(&source);

        assert!(result.is_ok());
        let plan = result.unwrap();

        assert_eq!(plan.target_os, "Container");
        assert_eq!(plan.migration_type, "Containerization");
        assert!(!plan.required_changes.is_empty());
    }

    #[test]
    fn test_migration_plan_has_steps() {
        let source = create_test_source();
        let result = planner::plan_os_upgrade(&source, "Ubuntu", "22.04");

        assert!(result.is_ok());
        let plan = result.unwrap();

        assert!(!plan.steps.is_empty());
        // Should have preparation, upgrade, and post-upgrade steps
        assert!(plan.steps.iter().any(|s| s.phase == "Preparation"));
    }

    #[test]
    fn test_compatibility_score_range() {
        let source = create_test_source();
        let result = planner::plan_os_upgrade(&source, "Ubuntu", "22.04");

        assert!(result.is_ok());
        let plan = result.unwrap();

        assert!(plan.compatibility_score >= 0.0);
        assert!(plan.compatibility_score <= 100.0);
    }

    fn create_test_plan() -> MigrationPlan {
        MigrationPlan {
            source: create_test_source(),
            target_os: "Ubuntu".to_string(),
            target_version: "22.04".to_string(),
            migration_type: "OS Upgrade".to_string(),
            overall_risk: RiskLevel::Medium,
            compatibility_score: 85.0,
            issues: vec![],
            package_mappings: vec![],
            required_changes: vec![],
            recommendations: vec![],
            estimated_effort_hours: 8,
            steps: vec![MigrationStep {
                order: 1,
                phase: "Preparation".to_string(),
                description: "Backup system".to_string(),
                commands: vec!["tar -czf backup.tar.gz /".to_string()],
                validation: "Verify backup".to_string(),
                rollback: Some("Restore from backup".to_string()),
            }],
        }
    }

    #[test]
    fn test_analyze_feasibility_high_score() {
        let plan = create_test_plan();
        let feasibility = analyzer::analyze_feasibility(&plan);

        assert!(feasibility.is_feasible);
        assert_eq!(feasibility.confidence, "High");
        assert_eq!(feasibility.critical_blockers, 0);
    }

    #[test]
    fn test_analyze_feasibility_low_score() {
        let mut plan = create_test_plan();
        plan.compatibility_score = 30.0;
        plan.issues.push(MigrationIssue {
            severity: RiskLevel::Critical,
            category: "Test".to_string(),
            description: "Critical issue".to_string(),
            impact: "High impact".to_string(),
            remediation: "Fix it".to_string(),
        });

        let feasibility = analyzer::analyze_feasibility(&plan);

        assert!(!feasibility.is_feasible);
        assert_eq!(feasibility.critical_blockers, 1);
    }

    #[test]
    fn test_estimate_downtime() {
        let plan = create_test_plan();
        let downtime = analyzer::estimate_downtime(&plan);

        assert!(downtime.minimum_minutes > 0);
        assert!(downtime.expected_minutes >= downtime.minimum_minutes);
        assert!(downtime.maximum_minutes >= downtime.expected_minutes);
        assert!(downtime.can_rollback); // Plan has rollback steps
    }

    #[test]
    fn test_downtime_expected_hours() {
        let plan = create_test_plan();
        let downtime = analyzer::estimate_downtime(&plan);

        let hours = downtime.expected_hours();
        assert!(hours > 0.0);
        assert_eq!(hours, downtime.expected_minutes as f64 / 60.0);
    }

    #[test]
    fn test_format_report_contains_header() {
        let plan = create_test_plan();
        let report = reporter::format_report(&plan, false);

        assert!(report.contains("Migration Plan Report"));
        assert!(report.contains("Source System"));
        assert!(report.contains("Target System"));
    }

    #[test]
    fn test_format_report_contains_metrics() {
        let plan = create_test_plan();
        let report = reporter::format_report(&plan, false);

        assert!(report.contains("Compatibility Score: 85.0%"));
        assert!(report.contains("Estimated Effort: 8 hours"));
        assert!(report.contains("Overall Risk: Medium"));
    }
}
