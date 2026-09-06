// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! License database with known licenses and their properties

use super::{LicenseType, RiskLevel};
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// License information
#[derive(Debug, Clone)]
pub struct LicenseInfo {
    pub name: String,
    pub license_type: LicenseType,
    pub risk_level: RiskLevel,
    pub compatible_with: Vec<String>,
    pub incompatible_with: Vec<String>,
}

/// Global license database
pub static LICENSE_DB: Lazy<LicenseDatabase> = Lazy::new(LicenseDatabase::new);

/// License database
pub struct LicenseDatabase {
    licenses: HashMap<String, LicenseInfo>,
}

impl Default for LicenseDatabase {
    fn default() -> Self {
        Self::new()
    }
}

impl LicenseDatabase {
    pub fn new() -> Self {
        let mut db = Self {
            licenses: HashMap::new(),
        };
        db.populate();
        db
    }

    fn populate(&mut self) {
        // Permissive licenses
        self.add(LicenseInfo {
            name: "MIT".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "Apache-2.0".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec![
                "MIT".to_string(),
                "BSD-2-Clause".to_string(),
                "BSD-3-Clause".to_string(),
            ],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "BSD-2-Clause".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "BSD-3-Clause".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "ISC".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        // Weak copyleft
        self.add(LicenseInfo {
            name: "LGPL-2.1-or-later".to_string(),
            license_type: LicenseType::Copyleft,
            risk_level: RiskLevel::Medium,
            compatible_with: vec!["GPL-2.0".to_string(), "GPL-3.0".to_string()],
            incompatible_with: vec!["Proprietary".to_string()],
        });

        self.add(LicenseInfo {
            name: "LGPL-3.0-or-later".to_string(),
            license_type: LicenseType::Copyleft,
            risk_level: RiskLevel::Medium,
            compatible_with: vec!["GPL-3.0".to_string()],
            incompatible_with: vec!["Proprietary".to_string()],
        });

        self.add(LicenseInfo {
            name: "MPL-2.0".to_string(),
            license_type: LicenseType::Copyleft,
            risk_level: RiskLevel::Medium,
            compatible_with: vec!["GPL-2.0".to_string(), "GPL-3.0".to_string()],
            incompatible_with: vec![],
        });

        // Strong copyleft
        self.add(LicenseInfo {
            name: "GPL-2.0-only".to_string(),
            license_type: LicenseType::StrongCopyleft,
            risk_level: RiskLevel::High,
            compatible_with: vec!["LGPL-2.1".to_string(), "GPL-2.0".to_string()],
            incompatible_with: vec!["Proprietary".to_string(), "Apache-2.0".to_string()],
        });

        self.add(LicenseInfo {
            name: "GPL-3.0-or-later".to_string(),
            license_type: LicenseType::StrongCopyleft,
            risk_level: RiskLevel::High,
            compatible_with: vec!["LGPL-3.0".to_string(), "Apache-2.0".to_string()],
            incompatible_with: vec!["Proprietary".to_string()],
        });

        self.add(LicenseInfo {
            name: "AGPL-3.0".to_string(),
            license_type: LicenseType::StrongCopyleft,
            risk_level: RiskLevel::Critical,
            compatible_with: vec!["GPL-3.0".to_string()],
            incompatible_with: vec!["Proprietary".to_string(), "MIT".to_string()],
        });

        // Public domain
        self.add(LicenseInfo {
            name: "Public-Domain".to_string(),
            license_type: LicenseType::PublicDomain,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "Unlicense".to_string(),
            license_type: LicenseType::PublicDomain,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        // Other permissive
        self.add(LicenseInfo {
            name: "Zlib".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });

        self.add(LicenseInfo {
            name: "PSF-2.0".to_string(),
            license_type: LicenseType::Permissive,
            risk_level: RiskLevel::Low,
            compatible_with: vec!["*".to_string()],
            incompatible_with: vec![],
        });
    }

    fn add(&mut self, info: LicenseInfo) {
        self.licenses.insert(info.name.clone(), info);
    }

    pub fn get(&self, license: &str) -> Option<&LicenseInfo> {
        self.licenses.get(license)
    }

    pub fn get_risk_level(&self, license: &str) -> RiskLevel {
        self.get(license)
            .map(|info| info.risk_level.clone())
            .unwrap_or(RiskLevel::Medium)
    }

    pub fn get_type(&self, license: &str) -> LicenseType {
        self.get(license)
            .map(|info| info.license_type.clone())
            .unwrap_or(LicenseType::Unknown)
    }
}
