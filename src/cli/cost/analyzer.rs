// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cost analysis and optimization

use super::*;

/// Find savings opportunities
pub fn find_savings_opportunities(
    metrics: &SystemMetrics,
    current: &ResourceEstimate,
    provider: CloudProvider,
) -> Vec<SavingsOpportunity> {
    let mut opportunities = Vec::new();

    // Right-sizing opportunity
    if current.vcpus > metrics.vcpu_count {
        let savings = current.compute_monthly * 0.3; // 30% savings from rightsizing
        opportunities.push(SavingsOpportunity {
            category: "Compute Right-sizing".to_string(),
            description: format!(
                "Instance is oversized ({} vCPUs allocated vs {} needed)",
                current.vcpus, metrics.vcpu_count
            ),
            current_cost: current.compute_monthly,
            optimized_cost: current.compute_monthly - savings,
            monthly_savings: savings,
            effort: OptimizationEffort::Low,
            priority: OptimizationPriority::High,
        });
    }

    // Storage optimization
    if metrics.storage_gb > 100.0 {
        let current_storage_cost = current.storage_monthly;
        let optimized_cost = current_storage_cost * 0.6; // 40% savings
        let savings = current_storage_cost - optimized_cost;

        opportunities.push(SavingsOpportunity {
            category: "Storage Optimization".to_string(),
            description: format!(
                "Use tiered storage ({:.1} GB total). Move cold data to cheaper tiers",
                metrics.storage_gb
            ),
            current_cost: current_storage_cost,
            optimized_cost,
            monthly_savings: savings,
            effort: OptimizationEffort::Medium,
            priority: OptimizationPriority::Medium,
        });
    }

    // Reserved instances opportunity
    let ri_savings = current.compute_monthly * 0.4; // 40% savings with 1-year RI
    opportunities.push(SavingsOpportunity {
        category: "Reserved Instances".to_string(),
        description: format!(
            "Purchase 1-year reserved instance for {} (40% savings)",
            current.instance_type
        ),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly - ri_savings,
        monthly_savings: ri_savings,
        effort: OptimizationEffort::Low,
        priority: OptimizationPriority::High,
    });

    // Spot instances for non-critical workloads
    if !metrics.has_database {
        let spot_savings = current.compute_monthly * 0.7; // 70% savings with spot
        opportunities.push(SavingsOpportunity {
            category: "Spot Instances".to_string(),
            description: "Use spot/preemptible instances for non-critical workloads".to_string(),
            current_cost: current.compute_monthly,
            optimized_cost: current.compute_monthly - spot_savings,
            monthly_savings: spot_savings,
            effort: OptimizationEffort::High,
            priority: OptimizationPriority::Medium,
        });
    }

    // Auto-scaling opportunity
    opportunities.push(SavingsOpportunity {
        category: "Auto-scaling".to_string(),
        description: "Implement auto-scaling to match demand (scale down during off-peak)"
            .to_string(),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly * 0.7,
        monthly_savings: current.compute_monthly * 0.3,
        effort: OptimizationEffort::Medium,
        priority: OptimizationPriority::Medium,
    });

    // Network optimization
    if current.network_monthly > 50.0 {
        let savings = current.network_monthly * 0.3;
        opportunities.push(SavingsOpportunity {
            category: "Network Egress".to_string(),
            description: "Optimize data transfer and use CDN to reduce egress costs".to_string(),
            current_cost: current.network_monthly,
            optimized_cost: current.network_monthly - savings,
            monthly_savings: savings,
            effort: OptimizationEffort::Medium,
            priority: OptimizationPriority::Low,
        });
    }

    // Remove unused resources
    if metrics.package_count > 500 {
        opportunities.push(SavingsOpportunity {
            category: "Unused Resources".to_string(),
            description: format!(
                "Remove {} unused packages and services to reduce resource requirements",
                metrics.package_count - 200
            ),
            current_cost: current.compute_monthly,
            optimized_cost: current.compute_monthly * 0.9,
            monthly_savings: current.compute_monthly * 0.1,
            effort: OptimizationEffort::High,
            priority: OptimizationPriority::Low,
        });
    }

    // Provider-specific opportunities
    match provider {
        CloudProvider::AWS => add_aws_opportunities(&mut opportunities, current),
        CloudProvider::Azure => add_azure_opportunities(&mut opportunities, current),
        CloudProvider::GCP => add_gcp_opportunities(&mut opportunities, current),
    }

    opportunities
}

fn add_aws_opportunities(opportunities: &mut Vec<SavingsOpportunity>, current: &ResourceEstimate) {
    // Savings Plans
    opportunities.push(SavingsOpportunity {
        category: "AWS Savings Plans".to_string(),
        description: "Commit to Compute Savings Plan for flexible instance usage".to_string(),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly * 0.66,
        monthly_savings: current.compute_monthly * 0.34,
        effort: OptimizationEffort::Low,
        priority: OptimizationPriority::High,
    });

    // Graviton instances
    if !current.instance_type.contains("graviton") && !current.instance_type.starts_with('r') {
        opportunities.push(SavingsOpportunity {
            category: "Graviton Instances".to_string(),
            description: "Migrate to ARM-based Graviton instances (20% better price-performance)"
                .to_string(),
            current_cost: current.compute_monthly,
            optimized_cost: current.compute_monthly * 0.8,
            monthly_savings: current.compute_monthly * 0.2,
            effort: OptimizationEffort::Medium,
            priority: OptimizationPriority::Medium,
        });
    }
}

fn add_azure_opportunities(
    opportunities: &mut Vec<SavingsOpportunity>,
    current: &ResourceEstimate,
) {
    // Azure Hybrid Benefit
    opportunities.push(SavingsOpportunity {
        category: "Azure Hybrid Benefit".to_string(),
        description: "Use existing licenses with Azure Hybrid Benefit (40% savings)".to_string(),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly * 0.6,
        monthly_savings: current.compute_monthly * 0.4,
        effort: OptimizationEffort::Low,
        priority: OptimizationPriority::High,
    });
}

fn add_gcp_opportunities(opportunities: &mut Vec<SavingsOpportunity>, current: &ResourceEstimate) {
    // Committed Use Discounts
    opportunities.push(SavingsOpportunity {
        category: "Committed Use Discounts".to_string(),
        description: "Commit to 1-year or 3-year usage for up to 57% discount".to_string(),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly * 0.43,
        monthly_savings: current.compute_monthly * 0.57,
        effort: OptimizationEffort::Low,
        priority: OptimizationPriority::High,
    });

    // Sustained Use Discounts (automatic)
    opportunities.push(SavingsOpportunity {
        category: "Sustained Use Discounts".to_string(),
        description: "Automatic discounts for consistent usage (already applied)".to_string(),
        current_cost: current.compute_monthly,
        optimized_cost: current.compute_monthly * 0.7,
        monthly_savings: current.compute_monthly * 0.3,
        effort: OptimizationEffort::Low,
        priority: OptimizationPriority::Low,
    });
}

/// Generate cost recommendations
pub fn generate_recommendations(
    metrics: &SystemMetrics,
    opportunities: &[SavingsOpportunity],
    provider: CloudProvider,
) -> Vec<CostRecommendation> {
    let mut recommendations = Vec::new();

    // Top opportunity
    if let Some(top_opp) = opportunities
        .iter()
        .filter(|o| o.priority == OptimizationPriority::High)
        .max_by(|a, b| {
            a.monthly_savings
                .partial_cmp(&b.monthly_savings)
                .unwrap_or(std::cmp::Ordering::Equal)
        })
    {
        recommendations.push(CostRecommendation {
            title: format!("Priority: {}", top_opp.category),
            description: top_opp.description.clone(),
            estimated_savings: top_opp.monthly_savings,
            implementation_steps: get_implementation_steps(&top_opp.category, provider),
        });
    }

    // Right-sizing recommendation
    if metrics.vcpu_count < 4 {
        recommendations.push(CostRecommendation {
            title: "Right-size Compute Resources".to_string(),
            description: "Current allocation appears oversized for detected workload".to_string(),
            estimated_savings: opportunities
                .iter()
                .find(|o| o.category == "Compute Right-sizing")
                .map(|o| o.monthly_savings)
                .unwrap_or(0.0),
            implementation_steps: vec![
                "Monitor actual CPU and memory usage over 2 weeks".to_string(),
                "Identify peak usage patterns".to_string(),
                "Select instance type matching 80th percentile usage".to_string(),
                "Test performance with smaller instance type".to_string(),
                "Implement auto-scaling for variable workloads".to_string(),
            ],
        });
    }

    // Storage optimization
    if metrics.storage_gb > 100.0 {
        recommendations.push(CostRecommendation {
            title: "Implement Storage Tiering".to_string(),
            description: "Move infrequently accessed data to cheaper storage tiers".to_string(),
            estimated_savings: opportunities
                .iter()
                .find(|o| o.category == "Storage Optimization")
                .map(|o| o.monthly_savings)
                .unwrap_or(0.0),
            implementation_steps: vec![
                "Analyze data access patterns".to_string(),
                "Identify cold data (not accessed in 30+ days)".to_string(),
                "Move cold data to cheaper storage tier".to_string(),
                "Set up lifecycle policies for automatic tiering".to_string(),
            ],
        });
    }

    // Cost monitoring
    recommendations.push(CostRecommendation {
        title: "Set Up Cost Monitoring and Alerts".to_string(),
        description: "Track costs in real-time and get alerted on anomalies".to_string(),
        estimated_savings: 0.0,
        implementation_steps: vec![
            format!("Enable {} Cost Explorer/Cost Management", provider.as_str()),
            "Set up budget alerts at 80% and 100% thresholds".to_string(),
            "Create custom dashboards for cost trends".to_string(),
            "Tag resources for cost allocation tracking".to_string(),
            "Review costs weekly and optimize continuously".to_string(),
        ],
    });

    recommendations
}

fn get_implementation_steps(category: &str, provider: CloudProvider) -> Vec<String> {
    match category {
        "Reserved Instances" => vec![
            format!(
                "Analyze usage patterns in {} Cost Explorer",
                provider.as_str()
            ),
            "Identify stable workloads running 24/7".to_string(),
            "Purchase 1-year reserved instances with partial upfront".to_string(),
            "Monitor RI utilization and adjust as needed".to_string(),
        ],
        "AWS Savings Plans" => vec![
            "Review compute usage in Savings Plans recommendations".to_string(),
            "Select Compute Savings Plan for flexibility".to_string(),
            "Commit to 1-year term with partial upfront payment".to_string(),
            "Track savings and coverage in Cost Explorer".to_string(),
        ],
        "Spot Instances" => vec![
            "Identify fault-tolerant, non-critical workloads".to_string(),
            "Implement spot instance handling (interruption notice)".to_string(),
            "Use spot instances with auto-scaling groups".to_string(),
            "Set maximum spot price to control costs".to_string(),
        ],
        "Auto-scaling" => vec![
            "Define scaling metrics (CPU, memory, requests/sec)".to_string(),
            "Set up auto-scaling group with min/max/desired capacity".to_string(),
            "Configure scale-up and scale-down policies".to_string(),
            "Test scaling behavior under load".to_string(),
        ],
        _ => vec![
            "Review current configuration".to_string(),
            "Implement recommended changes".to_string(),
            "Monitor impact on performance and costs".to_string(),
        ],
    }
}
