// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! License module integration tests

use guestkit::cli::license::*;
use std::collections::HashMap;

#[cfg(test)]
mod database_tests {
    use super::*;

    #[test]
    fn test_license_database_initialization() {
        let db = &*database::LICENSE_DB;

        // Should have common licenses
        assert!(db.get("MIT").is_some());
        assert!(db.get("Apache-2.0").is_some());
        assert!(db.get("GPL-3.0-or-later").is_some());
    }

    #[test]
    fn test_license_risk_levels() {
        let db = &*database::LICENSE_DB;

        // MIT should be low risk
        let mit_risk = db.get_risk_level("MIT");
        assert_eq!(mit_risk, RiskLevel::Low);

        // GPL should be high risk
        let gpl_risk = db.get_risk_level("GPL-3.0-or-later");
        assert_eq!(gpl_risk, RiskLevel::High);

        // AGPL should be critical risk
        let agpl_risk = db.get_risk_level("AGPL-3.0");
        assert_eq!(agpl_risk, RiskLevel::Critical);
    }

    #[test]
    fn test_license_types() {
        let db = &*database::LICENSE_DB;

        assert_eq!(db.get_type("MIT"), LicenseType::Permissive);
        assert_eq!(db.get_type("GPL-3.0-or-later"), LicenseType::StrongCopyleft);
        assert_eq!(db.get_type("LGPL-3.0-or-later"), LicenseType::Copyleft);
        assert_eq!(db.get_type("Unknown-License"), LicenseType::Unknown);
    }

    #[test]
    fn test_unknown_license() {
        let db = &*database::LICENSE_DB;

        let unknown_risk = db.get_risk_level("NonExistentLicense");
        assert_eq!(unknown_risk, RiskLevel::Medium);

        let unknown_type = db.get_type("NonExistentLicense");
        assert_eq!(unknown_type, LicenseType::Unknown);
    }
}

#[cfg(test)]
mod analyzer_tests {
    use super::*;

    #[test]
    fn test_find_violations_prohibited() {
        let analyzer = analyzer::LicenseAnalyzer::new();

        let packages = vec![PackageLicense {
            package_name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            license: "AGPL-3.0".to_string(),
            license_type: LicenseType::StrongCopyleft,
            risk_level: RiskLevel::Critical,
            compatible_with: vec![],
            incompatible_with: vec![],
        }];

        let prohibited = vec!["AGPL-3.0".to_string()];
        let violations = analyzer.find_violations(&packages, &prohibited);

        assert_eq!(violations.len(), 2); // Prohibited + AGPL network copyleft
        assert!(violations
            .iter()
            .any(|v| matches!(v.violation_type, ViolationType::ProhibitedLicense)));
    }

    #[test]
    fn test_find_violations_missing_license() {
        let analyzer = analyzer::LicenseAnalyzer::new();

        let packages = vec![PackageLicense {
            package_name: "test-package".to_string(),
            version: "1.0.0".to_string(),
            license: "Unknown".to_string(),
            license_type: LicenseType::Unknown,
            risk_level: RiskLevel::Medium,
            compatible_with: vec![],
            incompatible_with: vec![],
        }];

        let violations = analyzer.find_violations(&packages, &[]);

        assert_eq!(violations.len(), 1);
        assert!(matches!(
            violations[0].violation_type,
            ViolationType::MissingLicense
        ));
        assert_eq!(violations[0].risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_no_violations() {
        let analyzer = analyzer::LicenseAnalyzer::new();

        let packages = vec![PackageLicense {
            package_name: "safe-package".to_string(),
            version: "1.0.0".to_string(),
            license: "MIT".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec![],
            incompatible_with: vec![],
        }];

        let violations = analyzer.find_violations(&packages, &[]);
        assert_eq!(violations.len(), 0);
    }
}

#[cfg(test)]
mod reporter_tests {
    use super::*;

    fn create_test_report() -> LicenseReport {
        let packages = vec![PackageLicense {
            package_name: "pkg1".to_string(),
            version: "1.0.0".to_string(),
            license: "MIT".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec![],
            incompatible_with: vec![],
        }];

        let mut license_summary = HashMap::new();
        license_summary.insert("MIT".to_string(), 1);

        let mut risk_summary = HashMap::new();
        risk_summary.insert(RiskLevel::Low, 1);

        LicenseReport {
            image_path: "test.img".to_string(),
            scanned_at: "2024-01-01T00:00:00Z".to_string(),
            total_packages: 1,
            packages,
            license_summary,
            risk_summary,
            violations: vec![],
            statistics: LicenseStatistics {
                permissive_licenses: 1,
                copyleft_licenses: 0,
                strong_copyleft_licenses: 0,
                proprietary_licenses: 0,
                unknown_licenses: 0,
                compliance_score: 100.0,
            },
        }
    }

    #[test]
    fn test_format_report_contains_header() {
        let report = create_test_report();
        let output = reporter::format_report(&report, false);

        assert!(output.contains("License Compliance Report"));
        assert!(output.contains("test.img"));
    }

    #[test]
    fn test_format_csv_header() {
        let report = create_test_report();
        let csv = reporter::format_csv(&report);

        assert!(csv.starts_with("Package,Version,License,Type,Risk Level"));
    }
}
