// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fleet mode: multiple disk images in one TUI session.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

const IMAGE_EXTS: &[&str] = &[
    "qcow2", "qcow", "qed", "img", "raw", "vmdk", "vdi", "vhd", "vhdx",
];

/// Build fleet list: primary image first, then images discovered under `fleet_root`.
pub fn build_fleet_list(primary: &Path, fleet_root: Option<&Path>) -> Result<Vec<PathBuf>> {
    let mut list = vec![primary.to_path_buf()];
    if let Some(root) = fleet_root {
        if root.is_file() {
            let p = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
            if !list.iter().any(|x| x == &p) {
                list.push(p);
            }
        } else if root.is_dir() {
            let mut extra = discover_images_in_dir(root)?;
            extra.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            for p in extra {
                if !list.iter().any(|x| x == &p) {
                    list.push(p);
                }
            }
        }
    }
    Ok(list)
}

pub fn discover_images_in_dir(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut found = Vec::new();
    let entries =
        std::fs::read_dir(dir).with_context(|| format!("read fleet dir {}", dir.display()))?;
    for entry in entries {
        let entry = entry?;
        let path = entry.path();
        if !path.is_file() {
            continue;
        }
        if is_disk_image(&path) {
            found.push(path);
        }
    }
    Ok(found)
}

fn is_disk_image(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| IMAGE_EXTS.iter().any(|x| e.eq_ignore_ascii_case(x)))
        .unwrap_or(false)
}

pub fn fleet_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("image")
        .to_string()
}
