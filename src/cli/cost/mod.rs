// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cloud cost optimization analysis

pub mod analyzer;
pub mod estimator;
pub mod reporter;

use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Cloud provider
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CloudProvider {
    AWS,
    Azure,
    GCP,
}

impl CloudProvider {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aws" => Some(Self::AWS),
            "azure" => Some(Self::Azure),
            "gcp" => Some(Self::GCP),
            _ => None,
        }
    }

    pub fn as_str(&self) -> &str {
        match self {
            Self::AWS => "AWS",
            Self::Azure => "Azure",
            Self::GCP => "GCP",
        }
    }
}

/// Cost analysis report
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostAnalysis {
    pub image_path: String,
    pub provider: CloudProvider,
    pub region: String,
    pub workload_profile: WorkloadProfile,
    pub current_estimate: ResourceEstimate,
    pub optimized_estimate: ResourceEstimate,
    pub savings_opportunities: Vec<SavingsOpportunity>,
    pub recommendations: Vec<CostRecommendation>,
    pub total_monthly_savings: f64,
    pub savings_percentage: f64,
}

/// Workload profile
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkloadProfile {
    pub cpu_usage_percent: f64,
    pub memory_usage_percent: f64,
    pub storage_type: String,
    pub network_egress_gb: f64,
    pub has_database: bool,
    pub has_cache: bool,
    pub has_web_server: bool,
}

/// Resource cost estimate
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceEstimate {
    pub instance_type: String,
    pub vcpus: u32,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub compute_monthly: f64,
    pub storage_monthly: f64,
    pub network_monthly: f64,
    pub total_monthly: f64,
}

/// Savings opportunity
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SavingsOpportunity {
    pub category: String,
    pub description: String,
    pub current_cost: f64,
    pub optimized_cost: f64,
    pub monthly_savings: f64,
    pub effort: OptimizationEffort,
    pub priority: OptimizationPriority,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationEffort {
    Low,
    Medium,
    High,
}

impl OptimizationEffort {
    pub fn emoji(&self) -> &str {
        match self {
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🔴",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum OptimizationPriority {
    Low,
    Medium,
    High,
    Critical,
}

impl OptimizationPriority {
    pub fn emoji(&self) -> &str {
        match self {
            Self::Low => "⬇️",
            Self::Medium => "➡️",
            Self::High => "⬆️",
            Self::Critical => "🔥",
        }
    }
}

/// Cost recommendation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CostRecommendation {
    pub title: String,
    pub description: String,
    pub estimated_savings: f64,
    pub implementation_steps: Vec<String>,
}

/// System metrics for cost analysis
#[derive(Debug, Clone)]
pub struct SystemMetrics {
    pub vcpu_count: u32,
    pub memory_gb: f64,
    pub storage_gb: f64,
    pub has_database: bool,
    pub has_cache: bool,
    pub has_web_server: bool,
    pub package_count: usize,
    pub service_count: usize,
}

/// Analyze image for cost optimization
pub fn analyze_costs<P: AsRef<Path>>(
    image_path: P,
    provider: CloudProvider,
    region: &str,
    verbose: bool,
) -> Result<CostAnalysis> {
    let image_path_str = image_path.as_ref().display().to_string();

    if verbose {
        println!("💰 Analyzing costs for: {}", image_path_str);
        println!("   Provider: {}", provider.as_str());
        println!("   Region: {}", region);
    }

    // Extract system metrics
    let metrics = extract_metrics(&image_path, verbose)?;

    if verbose {
        println!("   vCPUs: {}", metrics.vcpu_count);
        println!("   Memory: {:.1} GB", metrics.memory_gb);
        println!("   Storage: {:.1} GB", metrics.storage_gb);
    }

    // Determine workload profile
    let workload_profile = determine_workload_profile(&metrics);

    // Estimate current costs
    let current_estimate =
        estimator::estimate_current_costs(&metrics, provider, region, &workload_profile);

    // Find optimization opportunities
    let savings_opportunities =
        analyzer::find_savings_opportunities(&metrics, &current_estimate, provider);

    // Calculate optimized estimate
    let optimized_estimate = estimator::calculate_optimized_costs(
        &current_estimate,
        &savings_opportunities,
        provider,
        region,
    );

    // Generate recommendations
    let recommendations =
        analyzer::generate_recommendations(&metrics, &savings_opportunities, provider);

    // Calculate total savings
    let total_monthly_savings = current_estimate.total_monthly - optimized_estimate.total_monthly;
    let savings_percentage = if current_estimate.total_monthly > 0.0 {
        (total_monthly_savings / current_estimate.total_monthly) * 100.0
    } else {
        0.0
    };

    Ok(CostAnalysis {
        image_path: image_path_str,
        provider,
        region: region.to_string(),
        workload_profile,
        current_estimate,
        optimized_estimate,
        savings_opportunities,
        recommendations,
        total_monthly_savings,
        savings_percentage,
    })
}

fn extract_metrics<P: AsRef<Path>>(image_path: P, verbose: bool) -> Result<SystemMetrics> {
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

    // Get system info
    let applications = g.inspect_list_applications2(root)?;
    let package_count = applications.len();

    // Estimate vCPU requirements based on workload
    let vcpu_count = estimate_vcpu_requirements(package_count);

    // Estimate memory requirements
    let memory_gb = estimate_memory_requirements(&mut g, package_count);

    // Calculate total storage
    let filesystems = g.list_filesystems()?;
    let mut total_storage: i64 = 0;
    for (device, fstype) in filesystems {
        if fstype != "unknown" && !fstype.is_empty() {
            let size = g.blockdev_getsize64(&device).unwrap_or(0);
            total_storage += size;
        }
    }
    let storage_gb = total_storage as f64 / 1_073_741_824.0;

    // Detect services
    let has_database = applications.iter().any(|(name, _, _)| {
        name.contains("mysql")
            || name.contains("postgresql")
            || name.contains("mariadb")
            || name.contains("redis")
    });

    let has_cache = applications
        .iter()
        .any(|(name, _, _)| name.contains("redis") || name.contains("memcached"));

    let has_web_server = applications.iter().any(|(name, _, _)| {
        name.contains("nginx") || name.contains("apache") || name.contains("httpd")
    });

    // Count services
    let mut service_count = 0;
    if g.is_dir("/lib/systemd/system").unwrap_or(false) {
        if let Ok(entries) = g.ls("/lib/systemd/system") {
            service_count = entries.iter().filter(|e| e.ends_with(".service")).count();
        }
    }

    g.shutdown()?;

    if verbose {
        println!("   Packages: {}", package_count);
        println!("   Services: {}", service_count);
        println!("   Database: {}", has_database);
        println!("   Cache: {}", has_cache);
        println!("   Web server: {}", has_web_server);
    }

    Ok(SystemMetrics {
        vcpu_count,
        memory_gb,
        storage_gb,
        has_database,
        has_cache,
        has_web_server,
        package_count,
        service_count,
    })
}

fn estimate_vcpu_requirements(package_count: usize) -> u32 {
    // Simple heuristic based on package count
    if package_count > 1000 {
        4
    } else if package_count > 500 {
        2
    } else {
        1
    }
}

fn estimate_memory_requirements(g: &mut Guestfs, package_count: usize) -> f64 {
    // Check if meminfo exists to get actual memory
    if g.is_file("/proc/meminfo").unwrap_or(false) {
        if let Ok(meminfo) = g.cat("/proc/meminfo") {
            for line in meminfo.lines() {
                if line.starts_with("MemTotal:") {
                    if let Some(kb_str) = line.split_whitespace().nth(1) {
                        if let Ok(kb) = kb_str.parse::<f64>() {
                            return kb / 1_048_576.0; // Convert KB to GB
                        }
                    }
                }
            }
        }
    }

    // Fallback to estimate based on package count
    if package_count > 1000 {
        8.0
    } else if package_count > 500 {
        4.0
    } else {
        2.0
    }
}

fn determine_workload_profile(metrics: &SystemMetrics) -> WorkloadProfile {
    // Estimate CPU usage based on workload type
    let cpu_usage_percent = if metrics.has_database {
        70.0
    } else if metrics.has_web_server {
        40.0
    } else {
        20.0
    };

    // Estimate memory usage
    let memory_usage_percent = if metrics.has_database {
        80.0
    } else if metrics.has_cache {
        70.0
    } else {
        50.0
    };

    // Storage type recommendation
    let storage_type = if metrics.has_database {
        "SSD".to_string()
    } else {
        "HDD".to_string()
    };

    // Estimate network egress
    let network_egress_gb = if metrics.has_web_server { 100.0 } else { 10.0 };

    WorkloadProfile {
        cpu_usage_percent,
        memory_usage_percent,
        storage_type,
        network_egress_gb,
        has_database: metrics.has_database,
        has_cache: metrics.has_cache,
        has_web_server: metrics.has_web_server,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_workload_profile_creation() {
        let profile = WorkloadProfile {
            cpu_usage_percent: 60.0,
            memory_usage_percent: 70.0,
            storage_type: "ssd".to_string(),
            network_egress_gb: 500.0,
            has_database: true,
            has_cache: false,
            has_web_server: true,
        };

        assert_eq!(profile.cpu_usage_percent, 60.0);
        assert!(profile.has_database);
        assert_eq!(profile.storage_type, "ssd");
    }

    #[test]
    fn test_resource_estimate_monthly_calculation() {
        let estimate = ResourceEstimate {
            instance_type: "t3.medium".to_string(),
            vcpus: 2,
            memory_gb: 4.0,
            storage_gb: 50.0,
            compute_monthly: 30.0,
            storage_monthly: 5.0,
            network_monthly: 10.0,
            total_monthly: 45.0,
        };

        assert_eq!(estimate.total_monthly, 45.0);
        assert_eq!(
            estimate.total_monthly,
            estimate.compute_monthly + estimate.storage_monthly + estimate.network_monthly
        );
    }

    #[test]
    fn test_savings_opportunity_creation() {
        let opportunity = SavingsOpportunity {
            category: "Right-sizing".to_string(),
            description: "Reduce instance size".to_string(),
            current_cost: 100.0,
            optimized_cost: 50.0,
            monthly_savings: 50.0,
            effort: OptimizationEffort::Low,
            priority: OptimizationPriority::High,
        };

        assert_eq!(opportunity.category, "Right-sizing");
        assert_eq!(opportunity.monthly_savings, 50.0);
        assert_eq!(
            opportunity.current_cost - opportunity.optimized_cost,
            opportunity.monthly_savings
        );
    }

    #[test]
    fn test_cost_recommendation_creation() {
        let recommendation = CostRecommendation {
            title: "Use Reserved Instances".to_string(),
            description: "Save up to 70% with 1-year commitment".to_string(),
            estimated_savings: 200.0,
            implementation_steps: vec!["Purchase RI".to_string()],
        };

        assert_eq!(recommendation.title, "Use Reserved Instances");
        assert_eq!(recommendation.estimated_savings, 200.0);
        assert_eq!(recommendation.implementation_steps.len(), 1);
    }
}
