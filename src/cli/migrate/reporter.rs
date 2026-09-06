// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration report formatting

use super::*;
use crate::cli::migrate::analyzer;

/// Format migration plan as text report
pub fn format_report(plan: &MigrationPlan, detailed: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str("🚀 Migration Plan Report\n");
    output.push_str("========================\n\n");

    // Source information
    output.push_str("📊 Source System\n");
    output.push_str("----------------\n");
    output.push_str(&format!(
        "OS: {} {}\n",
        plan.source.os_name, plan.source.os_version
    ));
    output.push_str(&format!("Hostname: {}\n", plan.source.hostname));
    output.push_str(&format!("Architecture: {}\n", plan.source.arch));
    output.push_str(&format!("Packages: {}\n", plan.source.packages.len()));
    output.push_str(&format!("Services: {}\n", plan.source.services.len()));
    output.push_str(&format!(
        "Total Size: {:.1} GB\n\n",
        plan.source.total_size_gb
    ));

    // Target information
    output.push_str("🎯 Target System\n");
    output.push_str("----------------\n");
    output.push_str(&format!(
        "Target: {} {}\n",
        plan.target_os, plan.target_version
    ));
    output.push_str(&format!("Migration Type: {}\n\n", plan.migration_type));

    // Risk assessment
    output.push_str("⚠️  Risk Assessment\n");
    output.push_str("-------------------\n");
    output.push_str(&format!(
        "{} Overall Risk: {:?}\n",
        plan.overall_risk.emoji(),
        plan.overall_risk
    ));
    output.push_str(&format!(
        "📈 Compatibility Score: {:.1}%\n",
        plan.compatibility_score
    ));
    output.push_str(&format!(
        "⏱️  Estimated Effort: {} hours\n\n",
        plan.estimated_effort_hours
    ));

    // Feasibility analysis
    let feasibility = analyzer::analyze_feasibility(plan);
    output.push_str("✅ Feasibility\n");
    output.push_str("--------------\n");
    output.push_str(&format!(
        "Feasible: {}\n",
        if feasibility.is_feasible { "Yes" } else { "No" }
    ));
    output.push_str(&format!("Confidence: {}\n", feasibility.confidence));
    output.push_str(&format!(
        "Critical Blockers: {}\n",
        feasibility.critical_blockers
    ));
    output.push_str(&format!("High Risks: {}\n", feasibility.high_risks));
    output.push_str(&format!("💡 {}\n\n", feasibility.recommendation));

    // Downtime estimate
    let downtime = analyzer::estimate_downtime(plan);
    output.push_str("⏰ Downtime Estimate\n");
    output.push_str("--------------------\n");
    output.push_str(&format!("Minimum: {} minutes\n", downtime.minimum_minutes));
    output.push_str(&format!(
        "Expected: {} minutes ({:.1} hours)\n",
        downtime.expected_minutes,
        downtime.expected_hours()
    ));
    output.push_str(&format!("Maximum: {} minutes\n", downtime.maximum_minutes));
    output.push_str(&format!(
        "Rollback Available: {}\n\n",
        if downtime.can_rollback { "Yes" } else { "No" }
    ));

    // Issues
    if !plan.issues.is_empty() {
        output.push_str("🚨 Migration Issues\n");
        output.push_str("-------------------\n");

        let mut issues_by_severity = std::collections::HashMap::new();
        for issue in &plan.issues {
            issues_by_severity
                .entry(format!("{:?}", issue.severity))
                .or_insert_with(Vec::new)
                .push(issue);
        }

        for severity in &["Critical", "High", "Medium", "Low"] {
            if let Some(issues) = issues_by_severity.get(*severity) {
                for issue in issues {
                    output.push_str(&format!(
                        "\n{} [{}] {}\n",
                        issue.severity.emoji(),
                        issue.category,
                        issue.description
                    ));
                    output.push_str(&format!("   Impact: {}\n", issue.impact));
                    output.push_str(&format!("   💡 {}\n", issue.remediation));
                }
            }
        }
        output.push('\n');
    }

    // Required changes
    if !plan.required_changes.is_empty() && detailed {
        output.push_str("🔧 Required Changes\n");
        output.push_str("-------------------\n");
        for change in &plan.required_changes {
            output.push_str(&format!(
                "{} [{}] {}\n",
                change.priority.emoji(),
                change.category,
                change.description
            ));
            output.push_str(&format!(
                "   Automated: {}\n",
                if change.automated { "Yes" } else { "No" }
            ));
        }
        output.push('\n');
    }

    // Package mappings summary
    if !plan.package_mappings.is_empty() {
        output.push_str("📦 Package Compatibility\n");
        output.push_str("------------------------\n");

        let direct = plan
            .package_mappings
            .iter()
            .filter(|m| m.mapping_type == MappingType::DirectMapping)
            .count();
        let name_change = plan
            .package_mappings
            .iter()
            .filter(|m| m.mapping_type == MappingType::NameChange)
            .count();
        let not_available = plan
            .package_mappings
            .iter()
            .filter(|m| m.mapping_type == MappingType::NotAvailable)
            .count();

        output.push_str(&format!("✅ Direct Mapping: {}\n", direct));
        output.push_str(&format!("🔄 Name Changes: {}\n", name_change));
        output.push_str(&format!("❌ Not Available: {}\n", not_available));

        if detailed && not_available > 0 {
            output.push_str("\nPackages requiring attention:\n");
            for mapping in plan
                .package_mappings
                .iter()
                .filter(|m| m.mapping_type == MappingType::NotAvailable)
                .take(10)
            {
                output.push_str(&format!(
                    "  - {}: {}\n",
                    mapping.source_package, mapping.notes
                ));
            }
        }
        output.push('\n');
    }

    // Recommendations
    if !plan.recommendations.is_empty() {
        output.push_str("💡 Recommendations\n");
        output.push_str("------------------\n");
        for rec in &plan.recommendations {
            output.push_str(&format!("  • {}\n", rec));
        }
        output.push('\n');
    }

    // Migration steps
    if detailed && !plan.steps.is_empty() {
        output.push_str("📋 Migration Steps\n");
        output.push_str("------------------\n");
        for step in &plan.steps {
            output.push_str(&format!(
                "\n{}. {} - {}\n",
                step.order, step.phase, step.description
            ));
            if !step.commands.is_empty() {
                output.push_str("   Commands:\n");
                for cmd in &step.commands {
                    output.push_str(&format!("     $ {}\n", cmd));
                }
            }
            output.push_str(&format!("   Validation: {}\n", step.validation));
            if let Some(rollback) = &step.rollback {
                output.push_str(&format!("   Rollback: {}\n", rollback));
            }
        }
        output.push('\n');
    }

    // Summary
    output.push_str("📝 Summary\n");
    output.push_str("----------\n");
    if plan.compatibility_score >= 80.0
        && plan
            .issues
            .iter()
            .all(|i| i.severity != RiskLevel::Critical)
    {
        output.push_str("✅ Migration appears feasible with standard approach\n");
    } else if plan.compatibility_score >= 60.0 {
        output.push_str("⚠️  Migration is possible but requires careful planning\n");
    } else {
        output.push_str("🔴 Migration has significant challenges - consider alternatives\n");
    }

    output
}

/// Format migration plan as HTML
pub fn format_html(plan: &MigrationPlan) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Migration Plan Report</title>\n");
    html.push_str("<style>\n");
    html.push_str("body { font-family: Arial, sans-serif; max-width: 1200px; margin: 0 auto; padding: 20px; }\n");
    html.push_str("h1, h2 { color: #333; }\n");
    html.push_str(
        ".info-box { background: #f0f0f0; padding: 15px; margin: 10px 0; border-radius: 5px; }\n",
    );
    html.push_str(".risk-critical { color: #d32f2f; }\n");
    html.push_str(".risk-high { color: #f57c00; }\n");
    html.push_str(".risk-medium { color: #ffa000; }\n");
    html.push_str(".risk-low { color: #388e3c; }\n");
    html.push_str(".step { margin: 15px 0; padding: 10px; border-left: 3px solid #2196f3; }\n");
    html.push_str("table { border-collapse: collapse; width: 100%; margin: 10px 0; }\n");
    html.push_str("th, td { padding: 10px; text-align: left; border-bottom: 1px solid #ddd; }\n");
    html.push_str("th { background-color: #2196f3; color: white; }\n");
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    html.push_str("<h1>🚀 Migration Plan Report</h1>\n");

    html.push_str("<div class=\"info-box\">\n");
    html.push_str("<h2>Source System</h2>\n");
    html.push_str(&format!(
        "<p><strong>OS:</strong> {} {}</p>\n",
        plan.source.os_name, plan.source.os_version
    ));
    html.push_str(&format!(
        "<p><strong>Hostname:</strong> {}</p>\n",
        plan.source.hostname
    ));
    html.push_str(&format!(
        "<p><strong>Packages:</strong> {}</p>\n",
        plan.source.packages.len()
    ));
    html.push_str("</div>\n");

    html.push_str("<div class=\"info-box\">\n");
    html.push_str("<h2>Target System</h2>\n");
    html.push_str(&format!(
        "<p><strong>Target:</strong> {} {}</p>\n",
        plan.target_os, plan.target_version
    ));
    html.push_str(&format!(
        "<p><strong>Migration Type:</strong> {}</p>\n",
        plan.migration_type
    ));
    html.push_str(&format!(
        "<p><strong>Compatibility Score:</strong> {:.1}%</p>\n",
        plan.compatibility_score
    ));
    html.push_str("</div>\n");

    if !plan.issues.is_empty() {
        html.push_str("<h2>Issues</h2>\n");
        html.push_str("<table>\n");
        html.push_str(
            "<tr><th>Severity</th><th>Category</th><th>Description</th><th>Remediation</th></tr>\n",
        );
        for issue in &plan.issues {
            let risk_class = match issue.severity {
                RiskLevel::Critical => "risk-critical",
                RiskLevel::High => "risk-high",
                RiskLevel::Medium => "risk-medium",
                RiskLevel::Low => "risk-low",
            };
            html.push_str(&format!(
                "<tr><td class=\"{}\"><strong>{:?}</strong></td><td>{}</td><td>{}</td><td>{}</td></tr>\n",
                risk_class, issue.severity, issue.category, issue.description, issue.remediation
            ));
        }
        html.push_str("</table>\n");
    }

    if !plan.steps.is_empty() {
        html.push_str("<h2>Migration Steps</h2>\n");
        for step in &plan.steps {
            html.push_str("<div class=\"step\">\n");
            html.push_str(&format!(
                "<h3>Step {}: {} - {}</h3>\n",
                step.order, step.phase, step.description
            ));
            if !step.commands.is_empty() {
                html.push_str("<p><strong>Commands:</strong></p>\n<pre>\n");
                for cmd in &step.commands {
                    html.push_str(&format!("{}\n", cmd));
                }
                html.push_str("</pre>\n");
            }
            html.push_str("</div>\n");
        }
    }

    html.push_str("</body>\n</html>\n");

    html
}
