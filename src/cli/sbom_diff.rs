// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Compare two SBOMs (SPDX, CycloneDX, or GuestKit inventory JSON).

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pkg {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SbomDiffReport {
    pub before_count: usize,
    pub after_count: usize,
    pub added: Vec<Pkg>,
    pub removed: Vec<Pkg>,
    pub updated: Vec<PkgUpdate>,
    pub unchanged: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PkgUpdate {
    pub name: String,
    pub from: String,
    pub to: String,
}

impl SbomDiffReport {
    pub fn dirty(&self) -> bool {
        !self.added.is_empty() || !self.removed.is_empty() || !self.updated.is_empty()
    }
}

/// Load name→version from SPDX / CycloneDX / GuestKit inventory / bare package list.
pub fn load_packages(path: &Path) -> anyhow::Result<BTreeMap<String, String>> {
    let raw = std::fs::read_to_string(path)
        .map_err(|e| anyhow::anyhow!("read {}: {e}", path.display()))?;
    parse_packages_json(&raw).ok_or_else(|| {
        anyhow::anyhow!(
            "{} is not SPDX, CycloneDX, or inventory JSON",
            path.display()
        )
    })
}

pub fn parse_packages_json(raw: &str) -> Option<BTreeMap<String, String>> {
    let v: serde_json::Value = serde_json::from_str(raw).ok()?;
    if let Some(arr) = v.get("packages").and_then(|x| x.as_array()) {
        // SPDX or GuestKit inventory
        return Some(collect_arr(arr));
    }
    if let Some(arr) = v.get("components").and_then(|x| x.as_array()) {
        return Some(collect_arr(arr));
    }
    if let Some(arr) = v.as_array() {
        return Some(collect_arr(arr));
    }
    None
}

fn collect_arr(arr: &[serde_json::Value]) -> BTreeMap<String, String> {
    let mut out = BTreeMap::new();
    for p in arr {
        let name = p
            .get("name")
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .trim()
            .to_string();
        if name.is_empty() {
            continue;
        }
        let ver = p
            .get("versionInfo")
            .or_else(|| p.get("version"))
            .and_then(|x| x.as_str())
            .unwrap_or("")
            .to_string();
        out.insert(name, ver);
    }
    out
}

pub fn diff_maps(
    before: &BTreeMap<String, String>,
    after: &BTreeMap<String, String>,
) -> SbomDiffReport {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut updated = Vec::new();
    let mut unchanged = 0usize;

    for (name, ver) in after {
        match before.get(name) {
            None => added.push(Pkg {
                name: name.clone(),
                version: ver.clone(),
            }),
            Some(old) if old != ver => updated.push(PkgUpdate {
                name: name.clone(),
                from: old.clone(),
                to: ver.clone(),
            }),
            Some(_) => unchanged += 1,
        }
    }
    for (name, ver) in before {
        if !after.contains_key(name) {
            removed.push(Pkg {
                name: name.clone(),
                version: ver.clone(),
            });
        }
    }
    SbomDiffReport {
        before_count: before.len(),
        after_count: after.len(),
        added,
        removed,
        updated,
        unchanged,
    }
}

pub fn diff_files(old: &Path, new: &Path) -> anyhow::Result<SbomDiffReport> {
    Ok(diff_maps(&load_packages(old)?, &load_packages(new)?))
}

pub fn print_text(r: &SbomDiffReport) {
    println!(
        "SBOM diff  before={} after={}",
        r.before_count, r.after_count
    );
    println!(
        "  added={} removed={} updated={} unchanged={}",
        r.added.len(),
        r.removed.len(),
        r.updated.len(),
        r.unchanged
    );
    for p in &r.added {
        println!("  + {} {}", p.name, p.version);
    }
    for p in &r.removed {
        println!("  - {} {}", p.name, p.version);
    }
    for p in &r.updated {
        println!("  ~ {} {} → {}", p.name, p.from, p.to);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_spdx_and_inventory() {
        let spdx = r#"{"spdxVersion":"SPDX-2.3","packages":[{"name":"bash","versionInfo":"5.1"},{"name":"coreutils","versionInfo":"8.32"}]}"#;
        let inv =
            r#"{"packages":[{"name":"bash","version":"5.2"},{"name":"curl","version":"8.0"}]}"#;
        let a = parse_packages_json(spdx).unwrap();
        let b = parse_packages_json(inv).unwrap();
        let d = diff_maps(&a, &b);
        assert_eq!(d.updated.len(), 1);
        assert_eq!(d.updated[0].name, "bash");
        assert_eq!(d.added[0].name, "curl");
        assert_eq!(d.removed[0].name, "coreutils");
    }

    #[test]
    fn cyclone_components() {
        let cdx =
            r#"{"bomFormat":"CycloneDX","components":[{"name":"openssl","version":"3.0.0"}]}"#;
        let m = parse_packages_json(cdx).unwrap();
        assert_eq!(m.get("openssl").unwrap(), "3.0.0");
    }
}
