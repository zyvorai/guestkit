// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SBOM format converters (SPDX, CycloneDX, CSV)

use super::Inventory;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// SPDX 2.3 Document
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpdxDocument {
    pub spdx_version: String,
    pub data_license: String,
    #[serde(rename = "SPDXID")]
    pub spdxid: String,
    pub name: String,
    pub document_namespace: String,
    pub creation_info: SpdxCreationInfo,
    pub packages: Vec<SpdxPackage>,
    pub relationships: Vec<SpdxRelationship>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SpdxCreationInfo {
    pub created: String,
    pub creators: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_list_version: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpdxPackage {
    #[serde(rename = "SPDXID")]
    pub spdxid: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_info: Option<String>,
    pub download_location: String,
    pub files_analyzed: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_concluded: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub license_declared: Option<String>,
    pub copyright_text: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpdxRelationship {
    pub spdx_element_id: String,
    pub relationship_type: String,
    pub related_spdx_element: String,
}

/// CycloneDX 1.5 BOM
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CycloneDxBom {
    pub bom_format: String,
    pub spec_version: String,
    pub serial_number: String,
    pub version: u32,
    pub metadata: CdxMetadata,
    pub components: Vec<CdxComponent>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub vulnerabilities: Vec<CdxVulnerability>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxMetadata {
    pub timestamp: String,
    pub tools: Vec<CdxTool>,
    pub component: CdxRootComponent,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxTool {
    pub vendor: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdxRootComponent {
    #[serde(rename = "type")]
    pub component_type: String,
    pub name: String,
    pub version: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdxComponent {
    #[serde(rename = "type")]
    pub component_type: String,
    pub bom_ref: String,
    pub name: String,
    pub version: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub purl: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub licenses: Vec<CdxLicense>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxLicense {
    pub license: CdxLicenseChoice,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxLicenseChoice {
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdxVulnerability {
    pub id: String,
    pub source: CdxSource,
    pub ratings: Vec<CdxRating>,
    pub affects: Vec<CdxAffect>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxSource {
    pub name: String,
    pub url: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CdxRating {
    pub severity: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub score: Option<f64>,
    pub method: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CdxAffect {
    #[serde(rename = "ref")]
    pub component_ref: String,
}

/// Convert inventory to SPDX format
pub fn to_spdx(inventory: &Inventory) -> Result<SpdxDocument> {
    let doc_id = "SPDXRef-DOCUMENT".to_string();
    let namespace = format!(
        "https://guestkit.dev/sbom/{}/{}",
        inventory.image_path.replace('/', "-"),
        chrono::Utc::now().format("%Y%m%d-%H%M%S")
    );

    let mut packages = Vec::new();
    let mut relationships = Vec::new();

    for (idx, pkg) in inventory.packages.iter().enumerate() {
        let pkg_id = format!("SPDXRef-Package-{}", idx);

        packages.push(SpdxPackage {
            spdxid: pkg_id.clone(),
            name: pkg.name.clone(),
            version_info: Some(pkg.version.clone()),
            download_location: "NOASSERTION".to_string(),
            files_analyzed: false,
            license_concluded: pkg.license.clone(),
            license_declared: pkg.license.clone(),
            copyright_text: "NOASSERTION".to_string(),
        });

        relationships.push(SpdxRelationship {
            spdx_element_id: doc_id.clone(),
            relationship_type: "DESCRIBES".to_string(),
            related_spdx_element: pkg_id,
        });
    }

    Ok(SpdxDocument {
        spdx_version: "SPDX-2.3".to_string(),
        data_license: "CC0-1.0".to_string(),
        spdxid: doc_id,
        name: inventory.image_path.clone(),
        document_namespace: namespace,
        creation_info: SpdxCreationInfo {
            created: inventory.scanned_at.clone(),
            creators: vec![format!("Tool: guestkit-{}", env!("CARGO_PKG_VERSION"))],
            license_list_version: Some("3.21".to_string()),
        },
        packages,
        relationships,
    })
}

/// Convert inventory to CycloneDX format
pub fn to_cyclonedx(inventory: &Inventory) -> Result<CycloneDxBom> {
    let serial_number = format!("urn:uuid:{}", Uuid::new_v4());

    let mut components = Vec::new();
    let mut vulnerabilities = Vec::new();

    for pkg in &inventory.packages {
        let bom_ref = format!(
            "pkg:{}/{}/{}@{}",
            pkg.package_type,
            inventory.os_name.to_lowercase(),
            pkg.name,
            pkg.version
        );

        let licenses = if let Some(license) = &pkg.license {
            vec![CdxLicense {
                license: CdxLicenseChoice {
                    id: license.clone(),
                },
            }]
        } else {
            Vec::new()
        };

        components.push(CdxComponent {
            component_type: "library".to_string(),
            bom_ref: bom_ref.clone(),
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            purl: Some(bom_ref.clone()),
            licenses,
        });

        // Add vulnerabilities
        for vuln in &pkg.vulnerabilities {
            vulnerabilities.push(CdxVulnerability {
                id: vuln.cve.clone(),
                source: CdxSource {
                    name: "NVD".to_string(),
                    url: format!("https://nvd.nist.gov/vuln/detail/{}", vuln.cve),
                },
                ratings: vec![CdxRating {
                    severity: vuln.severity.clone(),
                    score: vuln.score,
                    method: "CVSSv3".to_string(),
                }],
                affects: vec![CdxAffect {
                    component_ref: bom_ref.clone(),
                }],
            });
        }
    }

    Ok(CycloneDxBom {
        bom_format: "CycloneDX".to_string(),
        spec_version: "1.5".to_string(),
        serial_number,
        version: 1,
        metadata: CdxMetadata {
            timestamp: inventory.scanned_at.clone(),
            tools: vec![CdxTool {
                vendor: "guestkit".to_string(),
                name: "guestkit".to_string(),
                version: env!("CARGO_PKG_VERSION").to_string(),
            }],
            component: CdxRootComponent {
                component_type: "application".to_string(),
                name: inventory.image_path.clone(),
                version: "1.0.0".to_string(),
            },
        },
        components,
        vulnerabilities,
    })
}

/// Convert inventory to CSV format
pub fn to_csv(inventory: &Inventory) -> Result<String> {
    let mut csv = String::new();

    // Header
    csv.push_str("Package,Version,Type,License,Size,CVEs,Max Severity\n");

    // Data rows
    for pkg in &inventory.packages {
        let size_str = pkg
            .size
            .map(format_size)
            .unwrap_or_else(|| "N/A".to_string());

        let cve_count = pkg.vulnerabilities.len();
        let max_severity = pkg
            .vulnerabilities
            .iter()
            .map(|v| v.severity.as_str())
            .max_by_key(|s| severity_rank(s))
            .unwrap_or("none");

        csv.push_str(&format!(
            "\"{}\",\"{}\",\"{}\",\"{}\",\"{}\",{},\"{}\"\n",
            pkg.name,
            pkg.version,
            pkg.package_type,
            pkg.license.as_deref().unwrap_or("Unknown"),
            size_str,
            cve_count,
            max_severity
        ));
    }

    Ok(csv)
}

fn format_size(bytes: i64) -> String {
    crate::cli::output::format_size(bytes as u64)
}

fn severity_rank(severity: &str) -> u8 {
    super::cve::severity_rank(severity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::inventory::{Inventory, PackageInfo, VulnerabilityInfo};

    fn create_test_inventory() -> Inventory {
        use std::collections::HashMap;

        Inventory {
            image_path: "test.qcow2".to_string(),
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            architecture: "x86_64".to_string(),
            scanned_at: "2024-01-01T00:00:00Z".to_string(),
            packages: vec![
                PackageInfo {
                    name: "nginx".to_string(),
                    version: "1.20.0".to_string(),
                    package_type: "deb".to_string(),
                    license: Some("BSD-2-Clause".to_string()),
                    size: Some(1024 * 1024), // 1 MB
                    installed_date: None,
                    files: vec![],
                    dependencies: vec![],
                    vulnerabilities: vec![VulnerabilityInfo {
                        cve: "CVE-2023-44487".to_string(),
                        severity: "high".to_string(),
                        score: Some(7.5),
                        description: "HTTP/2 rapid reset attack".to_string(),
                        fixed_version: Some("1.20.1".to_string()),
                    }],
                    checksum: None,
                },
                PackageInfo {
                    name: "curl".to_string(),
                    version: "7.68.0".to_string(),
                    package_type: "deb".to_string(),
                    license: Some("MIT".to_string()),
                    size: Some(512 * 1024), // 512 KB
                    installed_date: None,
                    files: vec![],
                    dependencies: vec![],
                    vulnerabilities: vec![],
                    checksum: None,
                },
            ],
            statistics: crate::cli::inventory::InventoryStatistics {
                total_packages: 2,
                total_size: 1024 * 1024 + 512 * 1024,
                vulnerabilities: {
                    let mut map = HashMap::new();
                    map.insert("high".to_string(), 1);
                    map
                },
                licenses: {
                    let mut map = HashMap::new();
                    map.insert("BSD-2-Clause".to_string(), 1);
                    map.insert("MIT".to_string(), 1);
                    map
                },
            },
        }
    }

    #[test]
    fn test_format_size_bytes() {
        assert_eq!(format_size(0), "0 B");
        assert_eq!(format_size(500), "500 B");
        assert_eq!(format_size(1023), "1023 B");
    }

    #[test]
    fn test_format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.00 KB");
        assert_eq!(format_size(1536), "1.50 KB");
        assert_eq!(format_size(10240), "10.00 KB");
    }

    #[test]
    fn test_format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.00 MB");
        assert_eq!(format_size(1024 * 1024 + 512 * 1024), "1.50 MB");
        assert_eq!(format_size(10 * 1024 * 1024), "10.00 MB");
    }

    #[test]
    fn test_format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.00 GB");
        assert_eq!(format_size(2 * 1024 * 1024 * 1024), "2.00 GB");
        assert_eq!(format_size(1536 * 1024 * 1024), "1.50 GB");
    }

    #[test]
    fn test_severity_rank_ordering() {
        assert_eq!(severity_rank("critical"), 4);
        assert_eq!(severity_rank("high"), 3);
        assert_eq!(severity_rank("medium"), 2);
        assert_eq!(severity_rank("low"), 1);
        assert_eq!(severity_rank("unknown"), 0);
    }

    #[test]
    fn test_severity_rank_case_insensitive() {
        assert_eq!(severity_rank("CRITICAL"), 4);
        assert_eq!(severity_rank("High"), 3);
        assert_eq!(severity_rank("MeDiUm"), 2);
    }

    #[test]
    fn test_to_spdx_structure() {
        let inventory = create_test_inventory();
        let spdx = to_spdx(&inventory).unwrap();

        assert_eq!(spdx.spdx_version, "SPDX-2.3");
        assert_eq!(spdx.data_license, "CC0-1.0");
        assert_eq!(spdx.name, "test.qcow2");
        assert!(spdx.document_namespace.contains("guestkit.dev/sbom"));
    }

    #[test]
    fn test_to_spdx_packages() {
        let inventory = create_test_inventory();
        let spdx = to_spdx(&inventory).unwrap();

        assert_eq!(spdx.packages.len(), 2);
        assert_eq!(spdx.packages[0].name, "nginx");
        assert_eq!(spdx.packages[0].version_info, Some("1.20.0".to_string()));
        assert_eq!(
            spdx.packages[0].license_concluded,
            Some("BSD-2-Clause".to_string())
        );
    }

    #[test]
    fn test_to_spdx_relationships() {
        let inventory = create_test_inventory();
        let spdx = to_spdx(&inventory).unwrap();

        assert_eq!(spdx.relationships.len(), 2);
        assert_eq!(spdx.relationships[0].relationship_type, "DESCRIBES");
        assert_eq!(spdx.relationships[0].spdx_element_id, "SPDXRef-DOCUMENT");
    }

    #[test]
    fn test_to_spdx_creation_info() {
        let inventory = create_test_inventory();
        let spdx = to_spdx(&inventory).unwrap();

        assert_eq!(spdx.creation_info.created, "2024-01-01T00:00:00Z");
        assert!(spdx.creation_info.creators[0].contains("guestkit"));
        assert_eq!(
            spdx.creation_info.license_list_version,
            Some("3.21".to_string())
        );
    }

    #[test]
    fn test_to_cyclonedx_structure() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        assert_eq!(cdx.bom_format, "CycloneDX");
        assert_eq!(cdx.spec_version, "1.5");
        assert_eq!(cdx.version, 1);
        assert!(cdx.serial_number.starts_with("urn:uuid:"));
    }

    #[test]
    fn test_to_cyclonedx_metadata() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        assert_eq!(cdx.metadata.timestamp, "2024-01-01T00:00:00Z");
        assert_eq!(cdx.metadata.tools.len(), 1);
        assert_eq!(cdx.metadata.tools[0].name, "guestkit");
        assert_eq!(cdx.metadata.component.name, "test.qcow2");
    }

    #[test]
    fn test_to_cyclonedx_components() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        assert_eq!(cdx.components.len(), 2);
        assert_eq!(cdx.components[0].name, "nginx");
        assert_eq!(cdx.components[0].version, "1.20.0");
        assert_eq!(cdx.components[0].component_type, "library");
    }

    #[test]
    fn test_to_cyclonedx_vulnerabilities() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        assert_eq!(cdx.vulnerabilities.len(), 1);
        assert_eq!(cdx.vulnerabilities[0].id, "CVE-2023-44487");
        assert_eq!(cdx.vulnerabilities[0].source.name, "NVD");
        assert_eq!(cdx.vulnerabilities[0].ratings[0].severity, "high");
        assert_eq!(cdx.vulnerabilities[0].ratings[0].score, Some(7.5));
    }

    #[test]
    fn test_to_cyclonedx_purl() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        let nginx_component = &cdx.components[0];
        assert!(nginx_component.purl.is_some());
        let purl = nginx_component.purl.as_ref().unwrap();
        assert!(purl.contains("pkg:deb/ubuntu/nginx"));
    }

    #[test]
    fn test_to_cyclonedx_licenses() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        let nginx_component = &cdx.components[0];
        assert_eq!(nginx_component.licenses.len(), 1);
        assert_eq!(nginx_component.licenses[0].license.id, "BSD-2-Clause");
    }

    #[test]
    fn test_to_cyclonedx_no_license() {
        let mut inventory = create_test_inventory();
        inventory.packages[0].license = None;

        let cdx = to_cyclonedx(&inventory).unwrap();
        assert_eq!(cdx.components[0].licenses.len(), 0);
    }

    #[test]
    fn test_to_csv_structure() {
        let inventory = create_test_inventory();
        let csv = to_csv(&inventory).unwrap();

        assert!(csv.starts_with("Package,Version,Type,License,Size,CVEs,Max Severity\n"));
    }

    #[test]
    fn test_to_csv_package_data() {
        let inventory = create_test_inventory();
        let csv = to_csv(&inventory).unwrap();

        assert!(csv.contains("\"nginx\""));
        assert!(csv.contains("\"1.20.0\""));
        assert!(csv.contains("\"deb\""));
        assert!(csv.contains("\"BSD-2-Clause\""));
        assert!(csv.contains("\"1.00 MB\""));
    }

    #[test]
    fn test_to_csv_vulnerability_count() {
        let inventory = create_test_inventory();
        let csv = to_csv(&inventory).unwrap();

        // nginx has 1 CVE
        let lines: Vec<&str> = csv.lines().collect();
        let nginx_line = lines.iter().find(|l| l.contains("nginx")).unwrap();
        assert!(nginx_line.contains(",1,"));
    }

    #[test]
    fn test_to_csv_max_severity() {
        let inventory = create_test_inventory();
        let csv = to_csv(&inventory).unwrap();

        let lines: Vec<&str> = csv.lines().collect();
        let nginx_line = lines.iter().find(|l| l.contains("nginx")).unwrap();
        assert!(nginx_line.contains("\"high\""));
    }

    #[test]
    fn test_to_csv_no_vulnerabilities() {
        let inventory = create_test_inventory();
        let csv = to_csv(&inventory).unwrap();

        let lines: Vec<&str> = csv.lines().collect();
        let curl_line = lines.iter().find(|l| l.contains("curl")).unwrap();
        assert!(curl_line.contains(",0,"));
        assert!(curl_line.contains("\"none\""));
    }

    #[test]
    fn test_to_csv_unknown_license() {
        let mut inventory = create_test_inventory();
        inventory.packages[0].license = None;

        let csv = to_csv(&inventory).unwrap();
        assert!(csv.contains("\"Unknown\""));
    }

    #[test]
    fn test_to_csv_no_size() {
        let mut inventory = create_test_inventory();
        inventory.packages[0].size = None;

        let csv = to_csv(&inventory).unwrap();
        assert!(csv.contains("\"N/A\""));
    }

    #[test]
    fn test_spdx_serialization() {
        let inventory = create_test_inventory();
        let spdx = to_spdx(&inventory).unwrap();

        // Test that it can be serialized to JSON
        let json = serde_json::to_string(&spdx).unwrap();
        assert!(json.contains("SPDX-2.3"));
        assert!(json.contains("nginx"));
    }

    #[test]
    fn test_cyclonedx_serialization() {
        let inventory = create_test_inventory();
        let cdx = to_cyclonedx(&inventory).unwrap();

        // Test that it can be serialized to JSON
        let json = serde_json::to_string(&cdx).unwrap();
        assert!(json.contains("CycloneDX"));
        assert!(json.contains("nginx"));
    }
}
