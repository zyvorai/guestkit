// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! License report formatting

use super::LicenseReport;

/// Format license report as text
pub fn format_report(report: &LicenseReport, show_details: bool) -> String {
    let mut output = String::new();

    output.push_str("📋 License Compliance Report\n");
    output.push_str("============================\n\n");
    output.push_str(&format!("Image: {}\n", report.image_path));
    output.push_str(&format!("Scanned: {}\n", report.scanned_at));
    output.push_str(&format!("Total Packages: {}\n\n", report.total_packages));

    // Statistics
    output.push_str("📊 License Statistics\n");
    output.push_str("--------------------\n");
    output.push_str(&format!(
        "✅ Permissive: {}\n",
        report.statistics.permissive_licenses
    ));
    output.push_str(&format!(
        "⚖️  Copyleft: {}\n",
        report.statistics.copyleft_licenses
    ));
    output.push_str(&format!(
        "🔒 Strong Copyleft: {}\n",
        report.statistics.strong_copyleft_licenses
    ));
    output.push_str(&format!(
        "💼 Proprietary: {}\n",
        report.statistics.proprietary_licenses
    ));
    output.push_str(&format!(
        "❓ Unknown: {}\n\n",
        report.statistics.unknown_licenses
    ));

    // Risk summary
    output.push_str("⚠️  Risk Summary\n");
    output.push_str("---------------\n");
    for (risk, count) in &report.risk_summary {
        output.push_str(&format!("{} {:?}: {}\n", risk.emoji(), risk, count));
    }
    output.push_str(&format!(
        "\n📈 Compliance Score: {:.1}%\n\n",
        report.statistics.compliance_score
    ));

    // Violations
    if !report.violations.is_empty() {
        output.push_str("🚨 License Violations\n");
        output.push_str("--------------------\n");
        for violation in &report.violations {
            output.push_str(&format!(
                "{} {} - {}\n",
                violation.risk_level.emoji(),
                violation.package_name,
                violation.description
            ));
            output.push_str(&format!("   💡 {}\n", violation.remediation));
        }
        output.push('\n');
    }

    // Top licenses
    output.push_str("📜 License Distribution (Top 10)\n");
    output.push_str("--------------------------------\n");
    let mut licenses: Vec<_> = report.license_summary.iter().collect();
    licenses.sort_by(|a, b| b.1.cmp(a.1));
    for (license, count) in licenses.iter().take(10) {
        output.push_str(&format!("{}: {}\n", license, count));
    }
    output.push('\n');

    // Detailed package list
    if show_details {
        output.push_str("📦 Package Details\n");
        output.push_str("------------------\n");
        for pkg in &report.packages {
            output.push_str(&format!(
                "{} {} ({}) - {}\n",
                pkg.risk_level.emoji(),
                pkg.package_name,
                pkg.version,
                pkg.license
            ));
        }
    }

    // Overall assessment
    output.push('\n');
    if report.violations.is_empty() {
        output.push_str("✅ No license violations found!\n");
    } else {
        output.push_str(&format!(
            "❌ Found {} license violations - review required\n",
            report.violations.len()
        ));
    }

    output
}

/// Format as CSV
pub fn format_csv(report: &LicenseReport) -> String {
    let mut csv = String::new();

    csv.push_str("Package,Version,License,Type,Risk Level\n");

    for pkg in &report.packages {
        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{:?}\",\"{:?}\"\n",
            pkg.package_name, pkg.version, pkg.license, pkg.license_type, pkg.risk_level
        ));
    }

    csv
}
