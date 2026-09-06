// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! virtio-win tree discovery and an offline inject plan.
//!
//! Does not copy files by itself — it resolves the host directories
//! `migrate-repair --virtio-win` / plan apply already know how to use.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Drivers GuestKit treats as cutover-critical on KVM/KubeVirt.
pub const CRITICAL_DRIVERS: &[&str] = &[
    "viostor",
    "vioscsi",
    "NetKVM",
    "netkvm",
    "vioserial",
    "vioser",
    "balloon",
    "viorng",
];

const OS_DIRS: &[&str] = &[
    "2k22", "2k25", "2k19", "2k16", "w11", "w10", "w8.1", "amd64",
];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtioWinTree {
    pub root: String,
    pub drivers: Vec<ResolvedDriver>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedDriver {
    pub name: String,
    pub host_dir: String,
    pub boot_critical: bool,
    pub has_inf: bool,
    pub has_sys: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VirtioWinPlan {
    pub image: Option<String>,
    pub tree: VirtioWinTree,
    pub missing: Vec<String>,
    pub apply_hint: String,
}

pub fn discover_tree(explicit: Option<&Path>) -> Result<PathBuf> {
    if let Some(p) = explicit {
        anyhow::ensure!(
            p.is_dir(),
            "virtio-win path is not a directory: {}",
            p.display()
        );
        return Ok(p.to_path_buf());
    }
    if let Ok(env) = std::env::var("GUESTKIT_VIRTIO_WIN") {
        let p = PathBuf::from(env);
        if p.is_dir() {
            return Ok(p);
        }
    }
    for candidate in [
        "/usr/share/virtio-win",
        "/usr/share/virtio-win/drivers",
        "/opt/virtio-win",
    ] {
        let p = PathBuf::from(candidate);
        if p.is_dir() {
            return Ok(p);
        }
    }
    anyhow::bail!(
        "no virtio-win tree found; pass --tree or set GUESTKIT_VIRTIO_WIN \
         (Fedora: dnf install virtio-win)"
    )
}

pub fn inspect_tree(root: &Path) -> VirtioWinTree {
    let mut drivers = Vec::new();
    let names = collect_driver_names(root);
    for name in names {
        if let Some(dir) = resolve_driver_dir(root, &name) {
            let boot_critical =
                name.eq_ignore_ascii_case("viostor") || name.eq_ignore_ascii_case("vioscsi");
            let has_inf = dir_has_ext(&dir, "inf");
            let has_sys = dir_has_ext(&dir, "sys");
            drivers.push(ResolvedDriver {
                name,
                host_dir: dir.display().to_string(),
                boot_critical,
                has_inf,
                has_sys,
            });
        }
    }
    drivers.sort_by_key(|a| a.name.to_lowercase());
    VirtioWinTree {
        root: root.display().to_string(),
        drivers,
    }
}

pub fn plan(root: &Path, image: Option<&Path>) -> VirtioWinPlan {
    let tree = inspect_tree(root);
    let present: Vec<String> = tree.drivers.iter().map(|d| d.name.to_lowercase()).collect();
    let missing = CRITICAL_DRIVERS
        .iter()
        .filter(|n| {
            let lower = n.to_lowercase();
            // netkvm / NetKVM and vioserial / vioser are aliases
            let aliases = match lower.as_str() {
                "netkvm" => vec!["netkvm"],
                "vioserial" | "vioser" => vec!["vioserial", "vioser"],
                other => vec![other],
            };
            !aliases.iter().any(|a| present.iter().any(|p| p == a))
        })
        .map(|s| (*s).to_string())
        .collect::<Vec<_>>();
    // de-dup alias misses: if netkvm present, drop NetKVM miss etc — handled above

    let apply_hint = match image {
        Some(img) => format!(
            "guestkit migrate-repair {} --target kvm --virtio-win {} --apply",
            img.display(),
            root.display()
        ),
        None => format!(
            "guestkit migrate-repair <disk.qcow2> --target kvm --virtio-win {} --apply",
            root.display()
        ),
    };

    VirtioWinPlan {
        image: image.map(|p| p.display().to_string()),
        tree,
        missing,
        apply_hint,
    }
}

pub fn print_plan(plan: &VirtioWinPlan, json: bool) -> Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(plan)?);
        return Ok(());
    }
    println!("virtio-win tree: {}", plan.tree.root);
    if plan.tree.drivers.is_empty() {
        println!("  (no driver directories found)");
    }
    for d in &plan.tree.drivers {
        let flag = if d.boot_critical {
            " boot-critical"
        } else {
            ""
        };
        let files = match (d.has_inf, d.has_sys) {
            (true, true) => "inf+sys",
            (true, false) => "inf",
            (false, true) => "sys",
            (false, false) => "empty?",
        };
        println!("  {:<12} {} [{}{}]", d.name, d.host_dir, files, flag);
    }
    if !plan.missing.is_empty() {
        println!("missing critical: {}", plan.missing.join(", "));
    }
    println!("apply: {}", plan.apply_hint);
    Ok(())
}

fn collect_driver_names(root: &Path) -> Vec<String> {
    let mut names = Vec::new();
    if let Ok(rd) = std::fs::read_dir(root) {
        for e in rd.flatten() {
            if e.path().is_dir() {
                if let Some(n) = e.file_name().to_str() {
                    if n.starts_with('.') {
                        continue;
                    }
                    // skip ISO-layout meta dirs
                    if matches!(n, "guest-agent" | "qemu-ga" | "Licenses" | "license") {
                        continue;
                    }
                    names.push(n.to_string());
                }
            }
        }
    }
    if names.is_empty() {
        // single-driver directory
        if dir_has_ext(root, "inf") || dir_has_ext(root, "sys") {
            names.push(
                root.file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("driver")
                    .to_string(),
            );
        }
    }
    names
}

pub fn resolve_driver_dir(root: &Path, driver: &str) -> Option<PathBuf> {
    if dir_has_ext(root, "inf") && root.ends_with(driver) {
        return Some(root.to_path_buf());
    }
    let base = root.join(driver);
    let mut candidates = vec![base.clone(), base.join("amd64")];
    for os in OS_DIRS {
        candidates.push(root.join(driver).join(os).join("amd64"));
        candidates.push(root.join(driver).join(os));
    }
    candidates.push(root.to_path_buf());
    for c in candidates {
        if c.is_dir() && (dir_has_ext(&c, "inf") || dir_has_ext(&c, "sys")) {
            return Some(c);
        }
    }
    if base.is_dir() {
        Some(base)
    } else {
        None
    }
}

fn dir_has_ext(dir: &Path, ext: &str) -> bool {
    std::fs::read_dir(dir)
        .ok()
        .map(|rd| {
            rd.flatten().any(|e| {
                e.path()
                    .extension()
                    .and_then(|x| x.to_str())
                    .map(|x| x.eq_ignore_ascii_case(ext))
                    .unwrap_or(false)
            })
        })
        .unwrap_or(false)
}

pub fn run_list(tree: Option<&Path>, json: bool) -> Result<()> {
    let root = discover_tree(tree)?;
    let inspected = inspect_tree(&root);
    print_plan(
        &VirtioWinPlan {
            image: None,
            tree: inspected,
            missing: vec![],
            apply_hint: String::new(),
        },
        json,
    )
}

pub fn run_plan(tree: Option<&Path>, image: Option<&Path>, json: bool) -> Result<()> {
    let root = discover_tree(tree).context("discover virtio-win tree")?;
    print_plan(&plan(&root, image), json)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inspects_synthetic_tree() {
        let tmp = tempfile::tempdir().unwrap();
        let vio = tmp.path().join("viostor").join("w10").join("amd64");
        std::fs::create_dir_all(&vio).unwrap();
        std::fs::write(vio.join("viostor.inf"), "").unwrap();
        std::fs::write(vio.join("viostor.sys"), "").unwrap();
        let net = tmp.path().join("NetKVM").join("w10").join("amd64");
        std::fs::create_dir_all(&net).unwrap();
        std::fs::write(net.join("netkvm.inf"), "").unwrap();

        let tree = inspect_tree(tmp.path());
        assert!(tree
            .drivers
            .iter()
            .any(|d| d.name == "viostor" && d.boot_critical));
        assert!(tree.drivers.iter().any(|d| d.name == "NetKVM"));
        let p = plan(tmp.path(), Some(Path::new("win.qcow2")));
        assert!(p.apply_hint.contains("migrate-repair"));
        assert!(p
            .missing
            .iter()
            .any(|m| m.to_lowercase().contains("balloon")
                || m.to_lowercase().contains("vioser")
                || m.to_lowercase().contains("vioscsi")));
    }
}
