// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migrate module tests

use guestkit::cli::migrate::*;

#[cfg(test)]
mod migration_target_tests {
    use super::*;

    #[test]
    fn test_migration_target_from_str() {
        assert_eq!(MigrationTarget::from_str("upgrade"), Some(MigrationTarget::OsUpgrade));
        assert_eq!(MigrationTarget::from_str("os"), Some(MigrationTarget::OsUpgrade));
        assert_eq!(MigrationTarget::from_str("cloud"), Some(MigrationTarget::CloudPlatform));
        assert_eq!(MigrationTarget::from_str("aws"), Some(MigrationTarget::CloudPlatform));
        assert_eq!(MigrationTarget::from_str("container"), Some(MigrationTarget::Containerization));
        assert_eq!(MigrationTarget::from_str("docker"), Some(MigrationTarget::Containerization));
        assert_eq!(MigrationTarget::from_str("invalid"), None);
    }
}

#[cfg(test)]
mod risk_level_tests {
    use super::*;

    #[test]
    fn test_risk_level_emoji() {
        assert_eq!(RiskLevel::Low.emoji(), "🟢");
        assert_eq!(RiskLevel::Medium.emoji(), "🟡");
        assert_eq!(RiskLevel::High.emoji(), "🟠");
        assert_eq!(RiskLevel::Critical.emoji(), "🔴");
    }
}

#[cfg(test)]
mod planner_tests {
    use super::*;

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
            services: vec![
                Service {
                    name: "nginx".to_string(),
                    enabled: true,
                },
            ],
            filesystems: vec![
                Filesystem {
                    device: "/dev/sda1".to_string(),
                    fstype: "ext4".to_string(),
                    size_gb: 20.0,
                },
            ],
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
}

#[cfg(test)]
mod analyzer_tests {
    use super::*;

    fn create_test_plan() -> MigrationPlan {
        MigrationPlan {
            source: SourceSystem {
                os_name: "Ubuntu".to_string(),
                os_version: "20.04".to_string(),
                os_major: 20,
                os_minor: 4,
                arch: "x86_64".to_string(),
                hostname: "test".to_string(),
                kernel: "Linux 5.4.0".to_string(),
                packages: vec![],
                services: vec![],
                filesystems: vec![],
                total_size_gb: 20.0,
            },
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
            steps: vec![
                MigrationStep {
                    order: 1,
                    phase: "Preparation".to_string(),
                    description: "Backup system".to_string(),
                    commands: vec!["tar -czf backup.tar.gz /".to_string()],
                    validation: "Verify backup".to_string(),
                    rollback: Some("Restore from backup".to_string()),
                },
            ],
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
}

#[cfg(test)]
mod reporter_tests {
    use super::*;

    fn create_test_plan() -> MigrationPlan {
        MigrationPlan {
            source: SourceSystem {
                os_name: "Ubuntu".to_string(),
                os_version: "20.04".to_string(),
                os_major: 20,
                os_minor: 4,
                arch: "x86_64".to_string(),
                hostname: "test".to_string(),
                kernel: "Linux 5.4.0".to_string(),
                packages: vec![],
                services: vec![],
                filesystems: vec![],
                total_size_gb: 20.0,
            },
            target_os: "Ubuntu".to_string(),
            target_version: "22.04".to_string(),
            migration_type: "OS Upgrade".to_string(),
            overall_risk: RiskLevel::Medium,
            compatibility_score: 85.0,
            issues: vec![
                MigrationIssue {
                    severity: RiskLevel::High,
                    category: "Package".to_string(),
                    description: "Test issue".to_string(),
                    impact: "Some impact".to_string(),
                    remediation: "Fix this".to_string(),
                },
            ],
            package_mappings: vec![],
            required_changes: vec![],
            recommendations: vec![
                "Test recommendation".to_string(),
            ],
            estimated_effort_hours: 8,
            steps: vec![],
        }
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

    #[test]
    fn test_format_report_contains_issues() {
        let plan = create_test_plan();
        let report = reporter::format_report(&plan, false);

        assert!(report.contains("Migration Issues"));
        assert!(report.contains("Test issue"));
        assert!(report.contains("Fix this"));
    }

    #[test]
    fn test_format_report_contains_recommendations() {
        let plan = create_test_plan();
        let report = reporter::format_report(&plan, false);

        assert!(report.contains("Recommendations"));
        assert!(report.contains("Test recommendation"));
    }

    #[test]
    fn test_format_html_basic_structure() {
        let plan = create_test_plan();
        let html = reporter::format_html(&plan);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html>"));
        assert!(html.contains("</html>"));
        assert!(html.contains("Migration Plan Report"));
    }

    #[test]
    fn test_format_html_contains_source_info() {
        let plan = create_test_plan();
        let html = reporter::format_html(&plan);

        assert!(html.contains("Ubuntu"));
        assert!(html.contains("20.04"));
    }
}

#[cfg(test)]
mod mapping_type_tests {
    use super::*;

    #[test]
    fn test_mapping_type_variants() {
        let types = vec![
            MappingType::DirectMapping,
            MappingType::NameChange,
            MappingType::Split,
            MappingType::Merge,
            MappingType::NotAvailable,
            MappingType::AlternativeRequired,
        ];

        // Just ensure all variants exist and can be created
        assert_eq!(types.len(), 6);
    }
}
