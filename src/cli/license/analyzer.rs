// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! License analyzer for finding violations and risks

use super::RiskLevel;
use super::{LicenseViolation, PackageLicense, ViolationType};

/// License analyzer
pub struct LicenseAnalyzer {}

impl Default for LicenseAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseAnalyzer {
    pub fn new() -> Self {
        Self {}
    }

    /// Find license violations
    pub fn find_violations(
        &self,
        packages: &[PackageLicense],
        prohibited: &[String],
    ) -> Vec<LicenseViolation> {
        let mut violations = Vec::new();

        for pkg in packages {
            // Check prohibited licenses
            if prohibited.iter().any(|p| pkg.license.contains(p)) {
                violations.push(LicenseViolation {
                    package_name: pkg.package_name.clone(),
                    violation_type: ViolationType::ProhibitedLicense,
                    description: format!("Package uses prohibited license: {}", pkg.license),
                    risk_level: RiskLevel::Critical,
                    remediation: format!(
                        "Remove package {} or obtain license exception",
                        pkg.package_name
                    ),
                });
            }

            // Check for missing licenses
            if pkg.license == "Unknown" {
                violations.push(LicenseViolation {
                    package_name: pkg.package_name.clone(),
                    violation_type: ViolationType::MissingLicense,
                    description: "Package license is unknown".to_string(),
                    risk_level: RiskLevel::Medium,
                    remediation: "Investigate and document package license".to_string(),
                });
            }

            // Check for AGPL (commercial restriction)
            if pkg.license.contains("AGPL") {
                violations.push(LicenseViolation {
                    package_name: pkg.package_name.clone(),
                    violation_type: ViolationType::CommercialRestriction,
                    description: "AGPL license has network copyleft requirements".to_string(),
                    risk_level: RiskLevel::Critical,
                    remediation: "Review AGPL requirements or replace package".to_string(),
                });
            }
        }

        violations
    }
}
