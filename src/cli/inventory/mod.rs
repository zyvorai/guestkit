// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Software Bill of Materials (SBOM) generation module

pub mod cve;
pub mod formats;
pub mod licenses;
pub mod sbom;

use crate::Guestfs;
use anyhow::{Context, Result};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// SBOM output format
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SbomFormat {
    Spdx,
    CycloneDx,
    Json,
    Csv,
}

impl SbomFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "spdx" => Ok(Self::Spdx),
            "cyclonedx" => Ok(Self::CycloneDx),
            "json" => Ok(Self::Json),
            "csv" => Ok(Self::Csv),
            _ => anyhow::bail!("Unknown format: {}", s),
        }
    }
}

/// Package information for SBOM
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub package_type: String,
    pub license: Option<String>,
    pub size: Option<i64>,
    pub installed_date: Option<String>,
    pub files: Vec<String>,
    pub dependencies: Vec<String>,
    pub vulnerabilities: Vec<VulnerabilityInfo>,
    pub checksum: Option<String>,
}

/// Vulnerability information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VulnerabilityInfo {
    pub cve: String,
    pub severity: String,
    pub score: Option<f64>,
    pub description: String,
    pub fixed_version: Option<String>,
}

impl VulnerabilityInfo {
    /// Create a new VulnerabilityInfo with validated CVSS score (must be 0.0-10.0)
    pub fn new(
        cve: String,
        severity: String,
        score: Option<f64>,
        description: String,
        fixed_version: Option<String>,
    ) -> Self {
        let validated_score = score.map(|s| s.clamp(0.0, 10.0));
        Self {
            cve,
            severity,
            score: validated_score,
            description,
            fixed_version,
        }
    }
}

/// Complete inventory data
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    pub image_path: String,
    pub scanned_at: String,
    pub os_name: String,
    pub os_version: String,
    pub architecture: String,
    pub packages: Vec<PackageInfo>,
    pub statistics: InventoryStatistics,
}

/// Inventory statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryStatistics {
    pub total_packages: usize,
    pub total_size: i64,
    pub vulnerabilities: HashMap<String, usize>,
    pub licenses: HashMap<String, usize>,
}

/// Generate inventory from disk image
pub fn generate_inventory<P: AsRef<Path>>(
    image_path: P,
    include_licenses: bool,
    include_cves: bool,
    include_files: bool,
) -> Result<Inventory> {
    let image_path_str = image_path.as_ref().display().to_string();

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

    // Get OS information
    let os_name = g
        .inspect_get_product_name(root)
        .unwrap_or_else(|_| "Unknown".to_string());
    let os_version = g
        .inspect_get_product_variant(root)
        .unwrap_or_else(|_| "Unknown".to_string());
    let architecture = g
        .inspect_get_arch(root)
        .unwrap_or_else(|_| "Unknown".to_string());

    // Scan packages
    let packages = scan_packages(&mut g, root, include_licenses, include_cves, include_files)?;

    // Calculate statistics
    let statistics = calculate_statistics(&packages);

    let inventory = Inventory {
        image_path: image_path_str,
        scanned_at: Utc::now().to_rfc3339(),
        os_name,
        os_version,
        architecture,
        packages,
        statistics,
    };

    // Shutdown guestfs
    g.shutdown()?;

    Ok(inventory)
}

/// Scan packages from the guest OS
fn scan_packages(
    g: &mut Guestfs,
    root: &str,
    include_licenses: bool,
    include_cves: bool,
    include_files: bool,
) -> Result<Vec<PackageInfo>> {
    let package_format = g.inspect_get_package_format(root)?;

    match package_format.as_str() {
        "deb" => scan_deb_packages(g, root, include_licenses, include_cves, include_files),
        "rpm" => scan_rpm_packages(g, root, include_licenses, include_cves, include_files),
        _ => anyhow::bail!("Unsupported package format: {}", package_format),
    }
}

/// Scan Debian/Ubuntu packages
fn scan_deb_packages(
    g: &mut Guestfs,
    root: &str,
    include_licenses: bool,
    include_cves: bool,
    _include_files: bool,
) -> Result<Vec<PackageInfo>> {
    let applications = g.inspect_list_applications2(root)?;
    let mut packages = Vec::new();

    for (name, version, _release) in applications {
        let mut pkg = PackageInfo {
            name: name.clone(),
            version: version.clone(),
            package_type: "deb".to_string(),
            license: None,
            size: None,
            installed_date: None,
            files: Vec::new(),
            dependencies: Vec::new(),
            vulnerabilities: Vec::new(),
            checksum: None,
        };

        // Add license information if requested
        if include_licenses {
            pkg.license = licenses::detect_license(&name, "deb");
        }

        // Add CVE information if requested
        if include_cves {
            pkg.vulnerabilities = cve::lookup_cves(&name, &version)?;
        }

        packages.push(pkg);
    }

    Ok(packages)
}

/// Scan RPM-based packages
fn scan_rpm_packages(
    g: &mut Guestfs,
    root: &str,
    include_licenses: bool,
    include_cves: bool,
    _include_files: bool,
) -> Result<Vec<PackageInfo>> {
    let applications = g.inspect_list_applications2(root)?;
    let mut packages = Vec::new();

    for (name, version, _release) in applications {
        let mut pkg = PackageInfo {
            name: name.clone(),
            version: version.clone(),
            package_type: "rpm".to_string(),
            license: None,
            size: None,
            installed_date: None,
            files: Vec::new(),
            dependencies: Vec::new(),
            vulnerabilities: Vec::new(),
            checksum: None,
        };

        // Add license information if requested
        if include_licenses {
            pkg.license = licenses::detect_license(&name, "rpm");
        }

        // Add CVE information if requested
        if include_cves {
            pkg.vulnerabilities = cve::lookup_cves(&name, &version)?;
        }

        packages.push(pkg);
    }

    Ok(packages)
}

/// Calculate inventory statistics
fn calculate_statistics(packages: &[PackageInfo]) -> InventoryStatistics {
    let mut total_size = 0i64;
    let mut vulnerabilities: HashMap<String, usize> = HashMap::new();
    let mut licenses: HashMap<String, usize> = HashMap::new();

    for pkg in packages {
        if let Some(size) = pkg.size {
            total_size += size;
        }

        for vuln in &pkg.vulnerabilities {
            *vulnerabilities.entry(vuln.severity.clone()).or_insert(0) += 1;
        }

        if let Some(license) = &pkg.license {
            *licenses.entry(license.clone()).or_insert(0) += 1;
        }
    }

    InventoryStatistics {
        total_packages: packages.len(),
        total_size,
        vulnerabilities,
        licenses,
    }
}

/// Export inventory to specified format
pub fn export_inventory(
    inventory: &Inventory,
    format: SbomFormat,
    output: Option<&str>,
) -> Result<()> {
    let content = match format {
        SbomFormat::Spdx => {
            let doc = formats::to_spdx(inventory)?;
            serde_json::to_string_pretty(&doc)?
        }
        SbomFormat::CycloneDx => {
            let bom = formats::to_cyclonedx(inventory)?;
            serde_json::to_string_pretty(&bom)?
        }
        SbomFormat::Json => serde_json::to_string_pretty(inventory)?,
        SbomFormat::Csv => formats::to_csv(inventory)?,
    };

    if let Some(path) = output {
        std::fs::write(path, content).context(format!("Failed to write to {}", path))?;
        println!("✅ SBOM written to: {}", path);
    } else {
        println!("{}", content);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sbom_format_from_str() {
        assert_eq!(SbomFormat::from_str("spdx").unwrap(), SbomFormat::Spdx);
        assert_eq!(SbomFormat::from_str("SPDX").unwrap(), SbomFormat::Spdx);
        assert_eq!(
            SbomFormat::from_str("cyclonedx").unwrap(),
            SbomFormat::CycloneDx
        );
        assert_eq!(SbomFormat::from_str("json").unwrap(), SbomFormat::Json);
        assert_eq!(SbomFormat::from_str("csv").unwrap(), SbomFormat::Csv);
    }

    #[test]
    fn test_sbom_format_invalid() {
        assert!(SbomFormat::from_str("invalid").is_err());
        assert!(SbomFormat::from_str("xml").is_err());
    }

    #[test]
    fn test_package_info_creation() {
        let pkg = PackageInfo {
            name: "nginx".to_string(),
            version: "1.18.0".to_string(),
            package_type: "deb".to_string(),
            license: Some("BSD-2-Clause".to_string()),
            size: Some(1024000),
            installed_date: Some("2024-01-15".to_string()),
            files: vec!["/usr/bin/nginx".to_string()],
            dependencies: vec!["libc6".to_string()],
            vulnerabilities: vec![],
            checksum: Some("abc123".to_string()),
        };

        assert_eq!(pkg.name, "nginx");
        assert_eq!(pkg.version, "1.18.0");
        assert_eq!(pkg.package_type, "deb");
        assert_eq!(pkg.license, Some("BSD-2-Clause".to_string()));
        assert_eq!(pkg.size, Some(1024000));
        assert_eq!(pkg.files.len(), 1);
        assert_eq!(pkg.dependencies.len(), 1);
        assert_eq!(pkg.vulnerabilities.len(), 0);
    }

    #[test]
    fn test_vulnerability_info_creation() {
        let vuln = VulnerabilityInfo {
            cve: "CVE-2024-1234".to_string(),
            severity: "HIGH".to_string(),
            score: Some(7.5),
            description: "Remote code execution vulnerability".to_string(),
            fixed_version: Some("1.18.1".to_string()),
        };

        assert_eq!(vuln.cve, "CVE-2024-1234");
        assert_eq!(vuln.severity, "HIGH");
        assert_eq!(vuln.score, Some(7.5));
        assert!(vuln.description.contains("Remote code execution"));
        assert_eq!(vuln.fixed_version, Some("1.18.1".to_string()));
    }

    #[test]
    fn test_inventory_creation() {
        let inventory = Inventory {
            image_path: "/path/to/image.qcow2".to_string(),
            scanned_at: "2024-01-15T10:00:00Z".to_string(),
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            packages: vec![],
            statistics: InventoryStatistics {
                total_packages: 0,
                total_size: 0,
                vulnerabilities: HashMap::new(),
                licenses: HashMap::new(),
            },
        };

        assert_eq!(inventory.os_name, "Ubuntu");
        assert_eq!(inventory.os_version, "22.04");
        assert_eq!(inventory.architecture, "x86_64");
        assert_eq!(inventory.packages.len(), 0);
    }

    #[test]
    fn test_calculate_statistics() {
        let packages = vec![
            PackageInfo {
                name: "pkg1".to_string(),
                version: "1.0".to_string(),
                package_type: "deb".to_string(),
                license: Some("MIT".to_string()),
                size: Some(1000),
                installed_date: None,
                files: vec![],
                dependencies: vec![],
                vulnerabilities: vec![VulnerabilityInfo {
                    cve: "CVE-2024-001".to_string(),
                    severity: "HIGH".to_string(),
                    score: Some(8.5),
                    description: "Test vuln".to_string(),
                    fixed_version: None,
                }],
                checksum: None,
            },
            PackageInfo {
                name: "pkg2".to_string(),
                version: "2.0".to_string(),
                package_type: "deb".to_string(),
                license: Some("Apache-2.0".to_string()),
                size: Some(2000),
                installed_date: None,
                files: vec![],
                dependencies: vec![],
                vulnerabilities: vec![
                    VulnerabilityInfo {
                        cve: "CVE-2024-002".to_string(),
                        severity: "HIGH".to_string(),
                        score: Some(7.0),
                        description: "Test vuln 2".to_string(),
                        fixed_version: None,
                    },
                    VulnerabilityInfo {
                        cve: "CVE-2024-003".to_string(),
                        severity: "MEDIUM".to_string(),
                        score: Some(5.0),
                        description: "Test vuln 3".to_string(),
                        fixed_version: None,
                    },
                ],
                checksum: None,
            },
        ];

        let stats = calculate_statistics(&packages);

        assert_eq!(stats.total_packages, 2);
        assert_eq!(stats.total_size, 3000);
        assert_eq!(*stats.vulnerabilities.get("HIGH").unwrap(), 2);
        assert_eq!(*stats.vulnerabilities.get("MEDIUM").unwrap(), 1);
        assert_eq!(*stats.licenses.get("MIT").unwrap(), 1);
        assert_eq!(*stats.licenses.get("Apache-2.0").unwrap(), 1);
    }

    #[test]
    fn test_statistics_with_empty_packages() {
        let packages = vec![];
        let stats = calculate_statistics(&packages);

        assert_eq!(stats.total_packages, 0);
        assert_eq!(stats.total_size, 0);
        assert_eq!(stats.vulnerabilities.len(), 0);
        assert_eq!(stats.licenses.len(), 0);
    }

    #[test]
    fn test_statistics_without_size() {
        let packages = vec![PackageInfo {
            name: "pkg1".to_string(),
            version: "1.0".to_string(),
            package_type: "deb".to_string(),
            license: None,
            size: None, // No size information
            installed_date: None,
            files: vec![],
            dependencies: vec![],
            vulnerabilities: vec![],
            checksum: None,
        }];

        let stats = calculate_statistics(&packages);

        assert_eq!(stats.total_packages, 1);
        assert_eq!(stats.total_size, 0);
    }
}
