// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cost report formatting

use super::*;

/// Format cost analysis as text report
pub fn format_report(analysis: &CostAnalysis, detailed: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str("💰 Cloud Cost Analysis\n");
    output.push_str("======================\n\n");

    // System info
    output.push_str("📊 System Information\n");
    output.push_str("---------------------\n");
    output.push_str(&format!("Image: {}\n", analysis.image_path));
    output.push_str(&format!("Cloud Provider: {}\n", analysis.provider.as_str()));
    output.push_str(&format!("Region: {}\n\n", analysis.region));

    // Workload profile
    output.push_str("🔧 Workload Profile\n");
    output.push_str("-------------------\n");
    output.push_str(&format!(
        "CPU Usage: {:.0}%\n",
        analysis.workload_profile.cpu_usage_percent
    ));
    output.push_str(&format!(
        "Memory Usage: {:.0}%\n",
        analysis.workload_profile.memory_usage_percent
    ));
    output.push_str(&format!(
        "Storage Type: {}\n",
        analysis.workload_profile.storage_type
    ));
    output.push_str(&format!(
        "Network Egress: {:.1} GB/month\n",
        analysis.workload_profile.network_egress_gb
    ));
    output.push_str(&format!(
        "Database: {}\n",
        if analysis.workload_profile.has_database {
            "Yes"
        } else {
            "No"
        }
    ));
    output.push_str(&format!(
        "Cache: {}\n",
        if analysis.workload_profile.has_cache {
            "Yes"
        } else {
            "No"
        }
    ));
    output.push_str(&format!(
        "Web Server: {}\n\n",
        if analysis.workload_profile.has_web_server {
            "Yes"
        } else {
            "No"
        }
    ));

    // Current costs
    output.push_str("💵 Current Cost Estimate\n");
    output.push_str("------------------------\n");
    format_resource_estimate(&mut output, &analysis.current_estimate);
    output.push('\n');

    // Optimized costs
    output.push_str("✨ Optimized Cost Estimate\n");
    output.push_str("--------------------------\n");
    format_resource_estimate(&mut output, &analysis.optimized_estimate);
    output.push('\n');

    // Savings summary
    output.push_str("💎 Potential Savings\n");
    output.push_str("--------------------\n");
    output.push_str(&format!(
        "Monthly Savings: ${:.2}\n",
        analysis.total_monthly_savings
    ));
    output.push_str(&format!(
        "Annual Savings: ${:.2}\n",
        analysis.total_monthly_savings * 12.0
    ));
    output.push_str(&format!(
        "Savings Percentage: {:.1}%\n\n",
        analysis.savings_percentage
    ));

    // Savings opportunities
    if !analysis.savings_opportunities.is_empty() {
        output.push_str("🎯 Savings Opportunities\n");
        output.push_str("------------------------\n");

        // Sort by savings amount (descending)
        let mut opportunities = analysis.savings_opportunities.clone();
        opportunities.sort_by(|a, b| {
            b.monthly_savings
                .partial_cmp(&a.monthly_savings)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        for opp in opportunities.iter().take(if detailed { 20 } else { 5 }) {
            output.push_str(&format!(
                "\n{} {} [{:?}]\n",
                opp.priority.emoji(),
                opp.category,
                opp.effort
            ));
            output.push_str(&format!("   {}\n", opp.description));
            output.push_str(&format!("   Current: ${:.2}/month\n", opp.current_cost));
            output.push_str(&format!("   Optimized: ${:.2}/month\n", opp.optimized_cost));
            output.push_str(&format!(
                "   💰 Savings: ${:.2}/month (${:.2}/year)\n",
                opp.monthly_savings,
                opp.monthly_savings * 12.0
            ));
        }
        output.push('\n');
    }

    // Recommendations
    if !analysis.recommendations.is_empty() {
        output.push_str("💡 Recommendations\n");
        output.push_str("------------------\n");

        for (idx, rec) in analysis.recommendations.iter().enumerate() {
            output.push_str(&format!("\n{}. {}\n", idx + 1, rec.title));
            output.push_str(&format!("   {}\n", rec.description));
            if rec.estimated_savings > 0.0 {
                output.push_str(&format!(
                    "   Potential Savings: ${:.2}/month\n",
                    rec.estimated_savings
                ));
            }

            if detailed {
                output.push_str("   Implementation Steps:\n");
                for (step_idx, step) in rec.implementation_steps.iter().enumerate() {
                    output.push_str(&format!("   {}. {}\n", step_idx + 1, step));
                }
            }
        }
        output.push('\n');
    }

    // Summary
    output.push_str("📝 Summary\n");
    output.push_str("----------\n");
    if analysis.savings_percentage > 40.0 {
        output.push_str(
            "🔥 Significant savings opportunity! Prioritize high-impact optimizations.\n",
        );
    } else if analysis.savings_percentage > 20.0 {
        output
            .push_str("✅ Good savings potential. Focus on reserved instances and right-sizing.\n");
    } else {
        output.push_str("👍 System is reasonably optimized. Focus on incremental improvements.\n");
    }

    output.push_str(&format!(
        "\nEstimated annual savings: ${:.2}\n",
        analysis.total_monthly_savings * 12.0
    ));
    output.push_str("Payback period for optimization work: < 1 month\n");

    output
}

fn format_resource_estimate(output: &mut String, estimate: &ResourceEstimate) {
    output.push_str(&format!("Instance Type: {}\n", estimate.instance_type));
    output.push_str(&format!("vCPUs: {}\n", estimate.vcpus));
    output.push_str(&format!("Memory: {:.1} GB\n", estimate.memory_gb));
    output.push_str(&format!("Storage: {:.1} GB\n\n", estimate.storage_gb));
    output.push_str("Cost Breakdown:\n");
    output.push_str(&format!(
        "  Compute: ${:.2}/month\n",
        estimate.compute_monthly
    ));
    output.push_str(&format!(
        "  Storage: ${:.2}/month\n",
        estimate.storage_monthly
    ));
    output.push_str(&format!(
        "  Network: ${:.2}/month\n",
        estimate.network_monthly
    ));
    output.push_str("  ────────────────────────\n");
    output.push_str(&format!(
        "  Total:   ${:.2}/month\n",
        estimate.total_monthly
    ));
    output.push_str(&format!(
        "  Annual:  ${:.2}/year\n",
        estimate.total_monthly * 12.0
    ));
}

/// Format as CSV
pub fn format_csv(analysis: &CostAnalysis) -> String {
    let mut csv = String::new();

    csv.push_str("Category,Description,Current Cost,Optimized Cost,Monthly Savings,Annual Savings,Effort,Priority\n");

    for opp in &analysis.savings_opportunities {
        csv.push_str(&format!(
            "\"{}\",\"{}\",{:.2},{:.2},{:.2},{:.2},{:?},{:?}\n",
            opp.category,
            opp.description,
            opp.current_cost,
            opp.optimized_cost,
            opp.monthly_savings,
            opp.monthly_savings * 12.0,
            opp.effort,
            opp.priority
        ));
    }

    csv
}
