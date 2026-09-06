// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration planning logic

use super::*;
use anyhow::Result;

/// Plan OS upgrade migration
pub fn plan_os_upgrade(
    source: &SourceSystem,
    target_os: &str,
    target_version: &str,
) -> Result<MigrationPlan> {
    let mut issues = Vec::new();
    let mut package_mappings = Vec::new();
    let mut required_changes = Vec::new();
    let mut recommendations = Vec::new();
    let mut steps = Vec::new();

    // Determine migration complexity
    let source_lower = source.os_name.to_lowercase();
    let target_lower = target_os.to_lowercase();

    // Check if it's a supported migration path
    let (is_supported, overall_risk) =
        check_migration_path(&source_lower, &target_lower, target_version);

    if !is_supported {
        issues.push(MigrationIssue {
            severity: RiskLevel::Critical,
            category: "Compatibility".to_string(),
            description: format!(
                "Migration from {} to {} is not a standard path",
                source.os_name, target_os
            ),
            impact: "May require complete reinstallation rather than upgrade".to_string(),
            remediation: "Consider backup and fresh installation approach".to_string(),
        });
    }

    // Check package compatibility
    analyze_package_compatibility(source, target_os, &mut package_mappings, &mut issues);

    // Check service compatibility
    analyze_service_compatibility(source, &mut issues, &mut required_changes);

    // Check filesystem compatibility
    analyze_filesystem_compatibility(source, &mut issues);

    // Generate recommendations
    generate_recommendations(source, target_os, &mut recommendations);

    // Generate migration steps
    generate_migration_steps(source, target_os, target_version, &mut steps);

    // Calculate compatibility score
    let compatibility_score =
        calculate_compatibility_score(&package_mappings, &issues, source.packages.len());

    // Estimate effort
    let estimated_effort_hours = estimate_effort(&issues, &package_mappings, &required_changes);

    Ok(MigrationPlan {
        source: source.clone(),
        target_os: target_os.to_string(),
        target_version: target_version.to_string(),
        migration_type: "OS Upgrade".to_string(),
        overall_risk,
        compatibility_score,
        issues,
        package_mappings,
        required_changes,
        recommendations,
        estimated_effort_hours,
        steps,
    })
}

/// Plan cloud migration
pub fn plan_cloud_migration(source: &SourceSystem, cloud_provider: &str) -> Result<MigrationPlan> {
    let mut issues = Vec::new();
    let package_mappings = Vec::new();
    let mut required_changes = Vec::new();
    let mut recommendations = Vec::new();
    let mut steps = Vec::new();

    // Cloud-specific considerations
    let provider_lower = cloud_provider.to_lowercase();

    // Check cloud agent requirements
    required_changes.push(RequiredChange {
        category: "Cloud Integration".to_string(),
        description: format!("Install {} cloud agent/tools", cloud_provider),
        priority: RiskLevel::High,
        automated: false,
    });

    // Check network configuration
    issues.push(MigrationIssue {
        severity: RiskLevel::Medium,
        category: "Network".to_string(),
        description: "Network configuration needs cloud adaptation".to_string(),
        impact: "Static IPs and network configs must be updated for cloud networking".to_string(),
        remediation: "Use cloud-native networking (VPC, subnets)".to_string(),
    });

    // Storage considerations
    if source.total_size_gb > 1000.0 {
        issues.push(MigrationIssue {
            severity: RiskLevel::Medium,
            category: "Storage".to_string(),
            description: format!("Large disk size: {:.1}GB", source.total_size_gb),
            impact: "Migration and storage costs may be significant".to_string(),
            remediation: "Consider data cleanup before migration".to_string(),
        });
    }

    // Generate cloud-specific steps
    generate_cloud_migration_steps(&provider_lower, source, &mut steps);

    recommendations.push(format!(
        "Review {} pricing for compute and storage",
        cloud_provider
    ));
    recommendations.push("Test migration in non-production environment first".to_string());
    recommendations.push("Plan for cloud-native alternatives to reduce costs".to_string());

    let compatibility_score = 75.0; // Cloud migrations are generally feasible
    let overall_risk = RiskLevel::Medium;
    let estimated_effort_hours = 40;

    Ok(MigrationPlan {
        source: source.clone(),
        target_os: cloud_provider.to_string(),
        target_version: "Cloud".to_string(),
        migration_type: "Cloud Migration".to_string(),
        overall_risk,
        compatibility_score,
        issues,
        package_mappings,
        required_changes,
        recommendations,
        estimated_effort_hours,
        steps,
    })
}

/// Plan containerization
pub fn plan_containerization(source: &SourceSystem) -> Result<MigrationPlan> {
    let mut issues = Vec::new();
    let package_mappings = Vec::new();
    let mut required_changes = Vec::new();
    let mut recommendations = Vec::new();
    let mut steps = Vec::new();

    // Check if suitable for containerization
    let has_stateful_services = source
        .services
        .iter()
        .any(|s| s.name.contains("mysql") || s.name.contains("postgresql"));

    if has_stateful_services {
        issues.push(MigrationIssue {
            severity: RiskLevel::High,
            category: "Stateful Services".to_string(),
            description: "Detected stateful database services".to_string(),
            impact: "Databases require persistent volume configuration in containers".to_string(),
            remediation: "Use StatefulSets in Kubernetes or volumes in Docker".to_string(),
        });
    }

    // Kernel dependencies
    required_changes.push(RequiredChange {
        category: "Kernel Features".to_string(),
        description: "Remove kernel-dependent features".to_string(),
        priority: RiskLevel::High,
        automated: false,
    });

    // Init system
    required_changes.push(RequiredChange {
        category: "Init System".to_string(),
        description: "Replace systemd with container-friendly init".to_string(),
        priority: RiskLevel::High,
        automated: false,
    });

    recommendations.push("Use multi-stage builds to minimize image size".to_string());
    recommendations.push("Externalize configuration using environment variables".to_string());
    recommendations.push("Store data in mounted volumes, not in container".to_string());
    recommendations.push("Consider microservices architecture for better scalability".to_string());

    generate_containerization_steps(source, &mut steps);

    let compatibility_score = 60.0; // Containerization requires significant changes
    let overall_risk = RiskLevel::High;
    let estimated_effort_hours = 80;

    Ok(MigrationPlan {
        source: source.clone(),
        target_os: "Container".to_string(),
        target_version: "Docker/K8s".to_string(),
        migration_type: "Containerization".to_string(),
        overall_risk,
        compatibility_score,
        issues,
        package_mappings,
        required_changes,
        recommendations,
        estimated_effort_hours,
        steps,
    })
}

fn check_migration_path(source: &str, target: &str, _target_version: &str) -> (bool, RiskLevel) {
    // Supported paths
    if (source.contains("ubuntu") || source.contains("debian")) && target.contains("ubuntu") {
        (true, RiskLevel::Low)
    } else if (source.contains("centos") || source.contains("rhel"))
        && (target.contains("rocky") || target.contains("alma"))
    {
        (true, RiskLevel::Medium)
    } else if source.contains("fedora") && target.contains("fedora") {
        (true, RiskLevel::Low)
    } else {
        (false, RiskLevel::High)
    }
}

fn analyze_package_compatibility(
    source: &SourceSystem,
    target_os: &str,
    mappings: &mut Vec<PackageMapping>,
    issues: &mut Vec<MigrationIssue>,
) {
    let _target_lower = target_os.to_lowercase();

    // Check for problematic packages
    let mut incompatible_count = 0;

    for pkg in source.packages.iter().take(50) {
        let mapping_type = if pkg.name.starts_with("lib") {
            // Libraries usually have direct mappings
            MappingType::DirectMapping
        } else if pkg.name.contains("python2") {
            // Python 2 is deprecated
            incompatible_count += 1;
            MappingType::NotAvailable
        } else {
            MappingType::DirectMapping
        };

        let notes = match mapping_type {
            MappingType::NotAvailable => {
                "Package no longer available, find alternative".to_string()
            }
            _ => "Should be available in target".to_string(),
        };

        mappings.push(PackageMapping {
            source_package: pkg.name.clone(),
            target_package: pkg.name.clone(),
            mapping_type,
            notes,
        });
    }

    if incompatible_count > 0 {
        issues.push(MigrationIssue {
            severity: RiskLevel::High,
            category: "Package Compatibility".to_string(),
            description: format!("{} packages not available in target", incompatible_count),
            impact: "Applications depending on these packages will fail".to_string(),
            remediation: "Find alternative packages or upgrade application dependencies"
                .to_string(),
        });
    }
}

fn analyze_service_compatibility(
    source: &SourceSystem,
    _issues: &mut Vec<MigrationIssue>,
    changes: &mut Vec<RequiredChange>,
) {
    for service in &source.services {
        if service.name == "sshd" {
            changes.push(RequiredChange {
                category: "Services".to_string(),
                description: "Reconfigure SSH service".to_string(),
                priority: RiskLevel::Medium,
                automated: false,
            });
        }
    }
}

fn analyze_filesystem_compatibility(source: &SourceSystem, issues: &mut Vec<MigrationIssue>) {
    for fs in &source.filesystems {
        if fs.fstype == "btrfs" || fs.fstype == "zfs" {
            issues.push(MigrationIssue {
                severity: RiskLevel::Medium,
                category: "Filesystem".to_string(),
                description: format!("Advanced filesystem detected: {}", fs.fstype),
                impact: "May require kernel modules or additional setup".to_string(),
                remediation: "Ensure target supports filesystem type".to_string(),
            });
        }
    }
}

fn generate_recommendations(
    _source: &SourceSystem,
    _target_os: &str,
    recommendations: &mut Vec<String>,
) {
    recommendations.push("Create full backup before migration".to_string());
    recommendations.push("Test migration on non-production system first".to_string());
    recommendations.push("Document all custom configurations".to_string());
    recommendations.push("Plan downtime window for migration".to_string());
    recommendations.push("Have rollback plan ready".to_string());
}

/// Determine the package manager commands for a given OS name.
/// Returns (update_cmds, upgrade_cmd, cleanup_cmd).
fn package_manager_for_os(os_name: &str) -> (&'static str, Vec<String>, String, String) {
    let os_lower = os_name.to_lowercase();
    if os_lower.contains("centos")
        || os_lower.contains("rhel")
        || os_lower.contains("rocky")
        || os_lower.contains("alma")
        || os_lower.contains("fedora")
    {
        (
            "dnf",
            vec![
                "dnf check-update || true".to_string(),
                "dnf upgrade -y".to_string(),
            ],
            "dnf system-upgrade download --releasever={}".to_string(),
            "dnf autoremove -y".to_string(),
        )
    } else if os_lower.contains("arch") {
        (
            "pacman",
            vec!["pacman -Syu --noconfirm".to_string()],
            "pacman -Syu --noconfirm".to_string(),
            "pacman -Rns $(pacman -Qdtq) --noconfirm || true".to_string(),
        )
    } else if os_lower.contains("suse") || os_lower.contains("opensuse") {
        (
            "zypper",
            vec!["zypper refresh".to_string(), "zypper update -y".to_string()],
            "zypper dup -y".to_string(),
            "zypper clean".to_string(),
        )
    } else {
        // Default to apt for debian/ubuntu and unknown
        (
            "apt",
            vec![
                "apt update && apt upgrade -y".to_string(),
                "apt dist-upgrade -y".to_string(),
            ],
            "do-release-upgrade -d".to_string(),
            "apt autoremove -y".to_string(),
        )
    }
}

fn generate_migration_steps(
    source: &SourceSystem,
    target_os: &str,
    target_version: &str,
    steps: &mut Vec<MigrationStep>,
) {
    // Determine package manager from target OS, falling back to source OS
    let (_, update_cmds, upgrade_cmd_template, cleanup_cmd) = {
        let target_lower = target_os.to_lowercase();
        if target_lower.contains("centos")
            || target_lower.contains("rhel")
            || target_lower.contains("rocky")
            || target_lower.contains("alma")
            || target_lower.contains("fedora")
            || target_lower.contains("debian")
            || target_lower.contains("ubuntu")
            || target_lower.contains("arch")
            || target_lower.contains("suse")
            || target_lower.contains("opensuse")
        {
            package_manager_for_os(target_os)
        } else {
            package_manager_for_os(&source.os_name)
        }
    };

    let upgrade_cmd = if upgrade_cmd_template.contains("{}") {
        upgrade_cmd_template.replace("{}", target_version)
    } else {
        upgrade_cmd_template
    };

    steps.push(MigrationStep {
        order: 1,
        phase: "Preparation".to_string(),
        description: "Backup current system".to_string(),
        commands: vec![
            "tar -czf /backup/system-backup.tar.gz /".to_string(),
            "rsync -av / /backup/full-backup/".to_string(),
        ],
        validation: "Verify backup integrity".to_string(),
        rollback: Some("Restore from backup".to_string()),
    });

    steps.push(MigrationStep {
        order: 2,
        phase: "Pre-upgrade".to_string(),
        description: "Update package repository".to_string(),
        commands: update_cmds,
        validation: "Check for errors in package updates".to_string(),
        rollback: Some("Restore from snapshot".to_string()),
    });

    steps.push(MigrationStep {
        order: 3,
        phase: "Upgrade".to_string(),
        description: format!("Upgrade to {} {}", target_os, target_version),
        commands: vec![upgrade_cmd],
        validation: "Verify OS version after reboot".to_string(),
        rollback: Some("Boot from previous kernel".to_string()),
    });

    steps.push(MigrationStep {
        order: 4,
        phase: "Post-upgrade".to_string(),
        description: "Verify services and applications".to_string(),
        commands: vec!["systemctl status".to_string(), cleanup_cmd],
        validation: "Check all critical services are running".to_string(),
        rollback: None,
    });
}

fn generate_cloud_migration_steps(
    provider: &str,
    _source: &SourceSystem,
    steps: &mut Vec<MigrationStep>,
) {
    steps.push(MigrationStep {
        order: 1,
        phase: "Preparation".to_string(),
        description: "Export VM image".to_string(),
        commands: vec!["qemu-img convert -f qcow2 -O raw vm.qcow2 vm.raw".to_string()],
        validation: "Verify image file integrity".to_string(),
        rollback: None,
    });

    steps.push(MigrationStep {
        order: 2,
        phase: "Upload".to_string(),
        description: format!("Upload image to {}", provider),
        commands: match provider {
            "aws" => vec!["aws ec2 import-image --disk-container file://containers.json  # Edit containers.json with your S3 bucket".to_string()],
            "azure" => vec!["az vm import --resource-group <RESOURCE_GROUP> --name <VM_NAME> --source vm.vhd".to_string()],
            "gcp" => vec!["gcloud compute images create <IMAGE_NAME> --source-uri gs://<BUCKET>/vm.raw".to_string()],
            _ => vec!["# Cloud-specific upload command — consult provider documentation".to_string()],
        },
        validation: "Verify image import success".to_string(),
        rollback: Some("Delete imported image".to_string()),
    });

    steps.push(MigrationStep {
        order: 3,
        phase: "Deploy".to_string(),
        description: "Create instance from image".to_string(),
        commands: match provider {
            "aws" => vec!["aws ec2 run-instances --image-id <AMI_ID> --instance-type <INSTANCE_TYPE>  # Replace with your AMI ID".to_string()],
            "azure" => vec!["az vm create --resource-group <RESOURCE_GROUP> --name <VM_NAME> --image <IMAGE_NAME>".to_string()],
            "gcp" => vec!["gcloud compute instances create <VM_NAME> --image <IMAGE_NAME>".to_string()],
            _ => vec!["# Cloud-specific instance creation — consult provider documentation".to_string()],
        },
        validation: "Verify instance is running".to_string(),
        rollback: Some("Terminate instance".to_string()),
    });
}

fn generate_containerization_steps(_source: &SourceSystem, steps: &mut Vec<MigrationStep>) {
    steps.push(MigrationStep {
        order: 1,
        phase: "Preparation".to_string(),
        description: "Create Dockerfile".to_string(),
        commands: vec!["# Create base Dockerfile from VM analysis".to_string()],
        validation: "Dockerfile syntax validation".to_string(),
        rollback: None,
    });

    steps.push(MigrationStep {
        order: 2,
        phase: "Build".to_string(),
        description: "Build container image".to_string(),
        commands: vec!["docker build -t app:v1 .".to_string()],
        validation: "Verify image builds successfully".to_string(),
        rollback: Some("docker rmi app:v1".to_string()),
    });

    steps.push(MigrationStep {
        order: 3,
        phase: "Test".to_string(),
        description: "Test container".to_string(),
        commands: vec![
            "docker run -d -p 80:80 app:v1".to_string(),
            "curl http://localhost:80".to_string(),
        ],
        validation: "Verify application responds correctly".to_string(),
        rollback: Some("docker stop $(docker ps -q)".to_string()),
    });
}

fn calculate_compatibility_score(
    mappings: &[PackageMapping],
    issues: &[MigrationIssue],
    total_packages: usize,
) -> f64 {
    if total_packages == 0 {
        return 100.0;
    }

    let compatible = mappings
        .iter()
        .filter(|m| m.mapping_type == MappingType::DirectMapping)
        .count();

    let critical_issues = issues
        .iter()
        .filter(|i| i.severity == RiskLevel::Critical)
        .count();

    // Denominator must match the number of packages actually checked (capped at 50)
    let checked_packages = total_packages.min(50);
    let base_score = (compatible as f64 / checked_packages as f64) * 100.0;
    let penalty = (critical_issues as f64) * 10.0;

    (base_score - penalty).clamp(0.0, 100.0)
}

fn estimate_effort(
    issues: &[MigrationIssue],
    mappings: &[PackageMapping],
    changes: &[RequiredChange],
) -> u32 {
    let base_hours = 8u32;

    let issue_hours = issues
        .iter()
        .map(|i| match i.severity {
            RiskLevel::Critical => 8,
            RiskLevel::High => 4,
            RiskLevel::Medium => 2,
            RiskLevel::Low => 1,
        })
        .sum::<u32>();

    let mapping_hours = mappings
        .iter()
        .filter(|m| m.mapping_type != MappingType::DirectMapping)
        .count() as u32;

    let change_hours = changes.len() as u32 * 2;

    base_hours + issue_hours + mapping_hours + change_hours
}
