// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CVE vulnerability lookup

use super::VulnerabilityInfo;
use anyhow::Result;
use once_cell::sync::Lazy;
use std::collections::HashMap;

/// A known CVE entry: (cve_id, severity, score)
type CveEntry = (&'static str, &'static str, f64);

/// Known CVEs for demonstration (in production, this would query a CVE database)
static KNOWN_CVES: Lazy<HashMap<&'static str, Vec<CveEntry>>> = Lazy::new(|| {
    let mut m: HashMap<&'static str, Vec<CveEntry>> = HashMap::new();

    // Example CVEs (package_name -> [(cve_id, severity, score)])
    m.insert(
        "openssl",
        vec![
            ("CVE-2024-0727", "high", 7.5),
            ("CVE-2023-6129", "medium", 5.3),
        ],
    );

    m.insert("nginx", vec![("CVE-2023-44487", "high", 7.5)]);

    m.insert("curl", vec![("CVE-2023-46218", "medium", 6.5)]);

    m.insert("python3", vec![("CVE-2023-40217", "medium", 5.3)]);

    m
});

/// Lookup CVEs for a package (static + OSV API with offline cache)
pub fn lookup_cves(package_name: &str, package_version: &str) -> Result<Vec<VulnerabilityInfo>> {
    let mut vulnerabilities = Vec::new();

    // Static fallback database
    if let Some(cves) = KNOWN_CVES.get(package_name) {
        for (cve_id, severity, score) in cves {
            vulnerabilities.push(VulnerabilityInfo {
                cve: cve_id.to_string(),
                severity: severity.to_string(),
                score: Some(*score),
                description: format!("Vulnerability in {} {}", package_name, package_version),
                fixed_version: None,
            });
        }
    }

    // OSV API lookup (cached)
    if let Ok(osv_vulns) = lookup_cves_osv(package_name, package_version) {
        for v in osv_vulns {
            if !vulnerabilities.iter().any(|existing| existing.cve == v.cve) {
                vulnerabilities.push(v);
            }
        }
    }

    Ok(vulnerabilities)
}

/// Query OSV API with local file cache (~/.cache/guestkit/cve/)
pub fn lookup_cves_osv(
    package_name: &str,
    package_version: &str,
) -> Result<Vec<VulnerabilityInfo>> {
    let cache_key = format!("{}@{}", package_name, package_version);
    let cache_dir = osv_cache_dir()?;
    let cache_file = cache_dir.join(format!("{}.json", sanitize_cache_key(&cache_key)));

    if cache_file.exists() {
        if let Ok(content) = std::fs::read_to_string(&cache_file) {
            if let Ok(vulns) = serde_json::from_str::<Vec<VulnerabilityInfo>>(&content) {
                return Ok(vulns);
            }
        }
    }

    let query = serde_json::json!({
        "package": {
            "name": package_name,
            "ecosystem": "Linux"
        },
        "version": package_version
    });

    let output = std::process::Command::new("curl")
        .args([
            "-sS",
            "-X",
            "POST",
            "https://api.osv.dev/v1/query",
            "-H",
            "Content-Type: application/json",
            "-d",
            &query.to_string(),
        ])
        .output();

    let mut vulnerabilities = Vec::new();

    if let Ok(resp) = output {
        if resp.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&resp.stdout) {
                if let Some(vulns) = json.get("vulns").and_then(|v| v.as_array()) {
                    for vuln in vulns {
                        let cve_id = vuln
                            .get("id")
                            .and_then(|v| v.as_str())
                            .unwrap_or("UNKNOWN")
                            .to_string();
                        let summary = vuln
                            .get("summary")
                            .and_then(|v| v.as_str())
                            .unwrap_or("")
                            .to_string();
                        vulnerabilities.push(VulnerabilityInfo {
                            cve: cve_id,
                            severity: "unknown".to_string(),
                            score: None,
                            description: summary,
                            fixed_version: None,
                        });
                    }
                }
            }
        }
    }

    if let Ok(json) = serde_json::to_string_pretty(&vulnerabilities) {
        let _ = std::fs::write(&cache_file, json);
    }

    Ok(vulnerabilities)
}

fn osv_cache_dir() -> Result<std::path::PathBuf> {
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"));
    let dir = home.join(".cache").join("guestkit").join("cve");
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

fn sanitize_cache_key(key: &str) -> String {
    key.chars()
        .map(|c| if c.is_ascii_alphanumeric() { c } else { '_' })
        .collect()
}

/// Aggregate CVE counts across fleet inventory results
pub fn fleet_cve_heatmap(
    inventories: &[(String, Vec<VulnerabilityInfo>)],
) -> HashMap<String, usize> {
    let mut heatmap: HashMap<String, usize> = HashMap::new();
    for (image, vulns) in inventories {
        heatmap.insert(image.clone(), vulns.len());
    }
    heatmap
}

// Legacy static-only lookup kept for tests
#[allow(dead_code)]
fn lookup_cves_static_only(
    package_name: &str,
    package_version: &str,
) -> Result<Vec<VulnerabilityInfo>> {
    let mut vulnerabilities = Vec::new();

    // Check if we have known CVEs for this package
    if let Some(cves) = KNOWN_CVES.get(package_name) {
        for (cve_id, severity, score) in cves {
            vulnerabilities.push(VulnerabilityInfo {
                cve: cve_id.to_string(),
                severity: severity.to_string(),
                score: Some(*score),
                description: format!("Vulnerability in {} {}", package_name, package_version),
                fixed_version: None,
            });
        }
    }

    Ok(vulnerabilities)
}

/// Filter vulnerabilities by severity
#[allow(dead_code)]
pub fn filter_by_severity(
    vulnerabilities: &[VulnerabilityInfo],
    min_severity: &str,
) -> Vec<VulnerabilityInfo> {
    let min_rank = severity_rank(min_severity);

    vulnerabilities
        .iter()
        .filter(|v| severity_rank(&v.severity) >= min_rank)
        .cloned()
        .collect()
}

pub fn severity_rank(severity: &str) -> u8 {
    match severity.to_lowercase().as_str() {
        "critical" => 4,
        "high" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lookup_cves_known_package() {
        let vulns = lookup_cves("openssl", "3.0.0").unwrap();
        assert_eq!(vulns.len(), 2);
        assert_eq!(vulns[0].cve, "CVE-2024-0727");
        assert_eq!(vulns[0].severity, "high");
        assert_eq!(vulns[0].score, Some(7.5));
    }

    #[test]
    fn test_lookup_cves_nginx() {
        let vulns = lookup_cves("nginx", "1.20.0").unwrap();
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].cve, "CVE-2023-44487");
        assert_eq!(vulns[0].severity, "high");
    }

    #[test]
    fn test_lookup_cves_unknown_package() {
        let vulns = lookup_cves("unknown-package", "1.0.0").unwrap();
        assert_eq!(vulns.len(), 0);
    }

    #[test]
    fn test_lookup_cves_curl() {
        let vulns = lookup_cves("curl", "7.68.0").unwrap();
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].cve, "CVE-2023-46218");
        assert_eq!(vulns[0].severity, "medium");
        assert_eq!(vulns[0].score, Some(6.5));
    }

    #[test]
    fn test_lookup_cves_python3() {
        let vulns = lookup_cves("python3", "3.8.0").unwrap();
        assert_eq!(vulns.len(), 1);
        assert_eq!(vulns[0].cve, "CVE-2023-40217");
        assert_eq!(vulns[0].severity, "medium");
    }

    #[test]
    fn test_vulnerability_info_fields() {
        let vulns = lookup_cves("openssl", "3.0.0").unwrap();
        let vuln = &vulns[0];

        assert!(vuln.cve.starts_with("CVE-"));
        assert!(!vuln.severity.is_empty());
        assert!(vuln.score.is_some());
        assert!(vuln.description.contains("openssl"));
        assert_eq!(vuln.fixed_version, None);
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
        assert_eq!(severity_rank("LOW"), 1);
    }

    #[test]
    fn test_filter_by_severity_critical() {
        let vulns = vec![
            VulnerabilityInfo {
                cve: "CVE-001".to_string(),
                severity: "critical".to_string(),
                score: Some(9.8),
                description: "Critical vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-002".to_string(),
                severity: "high".to_string(),
                score: Some(7.5),
                description: "High vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-003".to_string(),
                severity: "medium".to_string(),
                score: Some(5.0),
                description: "Medium vuln".to_string(),
                fixed_version: None,
            },
        ];

        let filtered = filter_by_severity(&vulns, "critical");
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].cve, "CVE-001");
    }

    #[test]
    fn test_filter_by_severity_high() {
        let vulns = vec![
            VulnerabilityInfo {
                cve: "CVE-001".to_string(),
                severity: "critical".to_string(),
                score: Some(9.8),
                description: "Critical vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-002".to_string(),
                severity: "high".to_string(),
                score: Some(7.5),
                description: "High vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-003".to_string(),
                severity: "low".to_string(),
                score: Some(2.0),
                description: "Low vuln".to_string(),
                fixed_version: None,
            },
        ];

        let filtered = filter_by_severity(&vulns, "high");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].cve, "CVE-001");
        assert_eq!(filtered[1].cve, "CVE-002");
    }

    #[test]
    fn test_filter_by_severity_medium() {
        let vulns = vec![
            VulnerabilityInfo {
                cve: "CVE-001".to_string(),
                severity: "high".to_string(),
                score: Some(7.5),
                description: "High vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-002".to_string(),
                severity: "medium".to_string(),
                score: Some(5.0),
                description: "Medium vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-003".to_string(),
                severity: "low".to_string(),
                score: Some(2.0),
                description: "Low vuln".to_string(),
                fixed_version: None,
            },
        ];

        let filtered = filter_by_severity(&vulns, "medium");
        assert_eq!(filtered.len(), 2);
        assert_eq!(filtered[0].cve, "CVE-001");
        assert_eq!(filtered[1].cve, "CVE-002");
    }

    #[test]
    fn test_filter_by_severity_low() {
        let vulns = vec![
            VulnerabilityInfo {
                cve: "CVE-001".to_string(),
                severity: "critical".to_string(),
                score: Some(9.8),
                description: "Critical vuln".to_string(),
                fixed_version: None,
            },
            VulnerabilityInfo {
                cve: "CVE-002".to_string(),
                severity: "low".to_string(),
                score: Some(2.0),
                description: "Low vuln".to_string(),
                fixed_version: None,
            },
        ];

        let filtered = filter_by_severity(&vulns, "low");
        assert_eq!(filtered.len(), 2);
    }

    #[test]
    fn test_filter_by_severity_empty_list() {
        let vulns: Vec<VulnerabilityInfo> = vec![];
        let filtered = filter_by_severity(&vulns, "high");
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_filter_by_severity_all_filtered() {
        let vulns = vec![VulnerabilityInfo {
            cve: "CVE-001".to_string(),
            severity: "low".to_string(),
            score: Some(2.0),
            description: "Low vuln".to_string(),
            fixed_version: None,
        }];

        let filtered = filter_by_severity(&vulns, "critical");
        assert_eq!(filtered.len(), 0);
    }

    #[test]
    fn test_known_cves_map_contains_expected_packages() {
        // Verify the static map is initialized correctly
        let vulns_openssl = lookup_cves("openssl", "1.0.0").unwrap();
        let vulns_nginx = lookup_cves("nginx", "1.0.0").unwrap();
        let vulns_curl = lookup_cves("curl", "1.0.0").unwrap();
        let vulns_python3 = lookup_cves("python3", "1.0.0").unwrap();

        assert!(!vulns_openssl.is_empty());
        assert!(!vulns_nginx.is_empty());
        assert!(!vulns_curl.is_empty());
        assert!(!vulns_python3.is_empty());
    }
}
