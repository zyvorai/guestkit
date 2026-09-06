// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cost estimation logic

use super::*;

/// Estimate current costs
pub fn estimate_current_costs(
    metrics: &SystemMetrics,
    provider: CloudProvider,
    region: &str,
    profile: &WorkloadProfile,
) -> ResourceEstimate {
    match provider {
        CloudProvider::AWS => estimate_aws_costs(metrics, region, profile),
        CloudProvider::Azure => estimate_azure_costs(metrics, region, profile),
        CloudProvider::GCP => estimate_gcp_costs(metrics, region, profile),
    }
}

fn estimate_aws_costs(
    metrics: &SystemMetrics,
    _region: &str,
    profile: &WorkloadProfile,
) -> ResourceEstimate {
    // Determine instance type based on requirements
    let (instance_type, vcpus, memory_gb, hourly_rate) = if metrics.has_database {
        ("r6i.xlarge", 4, 32.0, 0.252)
    } else if metrics.vcpu_count >= 4 {
        ("t3.xlarge", 4, 16.0, 0.1664)
    } else if metrics.vcpu_count >= 2 {
        ("t3.medium", 2, 4.0, 0.0416)
    } else {
        ("t3.small", 2, 2.0, 0.0208)
    };

    // Compute costs (730 hours/month)
    let compute_monthly = hourly_rate * 730.0;

    // Storage costs (EBS gp3)
    let storage_monthly = if profile.storage_type == "SSD" {
        metrics.storage_gb * 0.08 // $0.08/GB/month for gp3
    } else {
        metrics.storage_gb * 0.045 // $0.045/GB/month for sc1
    };

    // Network egress costs
    let network_monthly = if profile.network_egress_gb > 100.0 {
        (profile.network_egress_gb - 100.0) * 0.09 // First 100GB free, then $0.09/GB
    } else {
        0.0
    };

    let total_monthly = compute_monthly + storage_monthly + network_monthly;

    ResourceEstimate {
        instance_type: instance_type.to_string(),
        vcpus,
        memory_gb,
        storage_gb: metrics.storage_gb,
        compute_monthly,
        storage_monthly,
        network_monthly,
        total_monthly,
    }
}

fn estimate_azure_costs(
    metrics: &SystemMetrics,
    _region: &str,
    profile: &WorkloadProfile,
) -> ResourceEstimate {
    // Determine VM size
    let (instance_type, vcpus, memory_gb, hourly_rate) = if metrics.has_database {
        ("Standard_E4s_v3", 4, 32.0, 0.252)
    } else if metrics.vcpu_count >= 4 {
        ("Standard_D4s_v3", 4, 16.0, 0.192)
    } else if metrics.vcpu_count >= 2 {
        ("Standard_B2ms", 2, 8.0, 0.083)
    } else {
        ("Standard_B1ms", 1, 2.0, 0.020)
    };

    let compute_monthly = hourly_rate * 730.0;

    // Storage costs (Premium SSD or Standard HDD)
    let storage_monthly = if profile.storage_type == "SSD" {
        metrics.storage_gb * 0.15 // Premium SSD
    } else {
        metrics.storage_gb * 0.04 // Standard HDD
    };

    // Network egress
    let network_monthly = if profile.network_egress_gb > 5.0 {
        (profile.network_egress_gb - 5.0) * 0.087
    } else {
        0.0
    };

    let total_monthly = compute_monthly + storage_monthly + network_monthly;

    ResourceEstimate {
        instance_type: instance_type.to_string(),
        vcpus,
        memory_gb,
        storage_gb: metrics.storage_gb,
        compute_monthly,
        storage_monthly,
        network_monthly,
        total_monthly,
    }
}

fn estimate_gcp_costs(
    metrics: &SystemMetrics,
    _region: &str,
    profile: &WorkloadProfile,
) -> ResourceEstimate {
    // Determine machine type
    let (instance_type, vcpus, memory_gb, hourly_rate) = if metrics.has_database {
        ("n2-highmem-4", 4, 32.0, 0.267)
    } else if metrics.vcpu_count >= 4 {
        ("n2-standard-4", 4, 16.0, 0.194)
    } else if metrics.vcpu_count >= 2 {
        ("e2-medium", 2, 4.0, 0.033)
    } else {
        ("e2-small", 2, 2.0, 0.020)
    };

    let compute_monthly = hourly_rate * 730.0;

    // Storage costs (SSD or Standard)
    let storage_monthly = if profile.storage_type == "SSD" {
        metrics.storage_gb * 0.17 // SSD persistent disk
    } else {
        metrics.storage_gb * 0.04 // Standard persistent disk
    };

    // Network egress (first 1TB in region)
    let network_monthly = if profile.network_egress_gb > 1024.0 {
        (profile.network_egress_gb - 1024.0) * 0.12
    } else {
        0.0
    };

    let total_monthly = compute_monthly + storage_monthly + network_monthly;

    ResourceEstimate {
        instance_type: instance_type.to_string(),
        vcpus,
        memory_gb,
        storage_gb: metrics.storage_gb,
        compute_monthly,
        storage_monthly,
        network_monthly,
        total_monthly,
    }
}

/// Calculate optimized costs based on savings opportunities
pub fn calculate_optimized_costs(
    current: &ResourceEstimate,
    opportunities: &[SavingsOpportunity],
    provider: CloudProvider,
    _region: &str,
) -> ResourceEstimate {
    let _total_savings: f64 = opportunities.iter().map(|o| o.monthly_savings).sum();

    // Determine optimized instance type (downsize if possible)
    let (instance_type, vcpus, memory_gb, compute_monthly) = match provider {
        CloudProvider::AWS => {
            if current.instance_type.contains("xlarge") {
                ("t3.large", 2, 8.0, current.compute_monthly * 0.5)
            } else if current.instance_type.contains("medium") {
                ("t3.small", 2, 2.0, current.compute_monthly * 0.5)
            } else {
                (
                    current.instance_type.as_str(),
                    current.vcpus,
                    current.memory_gb,
                    current.compute_monthly,
                )
            }
        }
        CloudProvider::Azure => {
            if current.vcpus > 2 {
                ("Standard_B2ms", 2, 8.0, current.compute_monthly * 0.6)
            } else {
                (
                    current.instance_type.as_str(),
                    current.vcpus,
                    current.memory_gb,
                    current.compute_monthly,
                )
            }
        }
        CloudProvider::GCP => {
            if current.vcpus > 2 {
                ("e2-medium", 2, 4.0, current.compute_monthly * 0.55)
            } else {
                (
                    current.instance_type.as_str(),
                    current.vcpus,
                    current.memory_gb,
                    current.compute_monthly,
                )
            }
        }
    };

    // Optimized storage (use cheaper tier where possible)
    let storage_monthly = current.storage_monthly * 0.6; // 40% savings by optimizing storage

    // Network optimization
    let network_monthly = current.network_monthly * 0.7; // 30% savings via optimization

    let total_monthly = compute_monthly + storage_monthly + network_monthly;

    ResourceEstimate {
        instance_type: instance_type.to_string(),
        vcpus,
        memory_gb,
        storage_gb: current.storage_gb,
        compute_monthly,
        storage_monthly,
        network_monthly,
        total_monthly,
    }
}
