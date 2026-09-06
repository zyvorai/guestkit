// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI binary name detection (`guestkit` vs `guestctl`).

use std::path::Path;
use std::sync::OnceLock;

static NAME: OnceLock<String> = OnceLock::new();

/// Basename of argv[0], or `guestkit` if missing.
pub fn name() -> &'static str {
    NAME.get_or_init(|| {
        std::env::args_os()
            .next()
            .and_then(|p| {
                Path::new(&p)
                    .file_name()
                    .map(|s| s.to_string_lossy().into_owned())
            })
            .filter(|s| !s.is_empty())
            .unwrap_or_else(|| "guestkit".to_string())
    })
    .as_str()
}

/// Display name for help and errors (same as [`name`]).
pub fn display() -> &'static str {
    name()
}

/// Format a copy-paste example: `{name} {cmd}`.
pub fn example(cmd: &str) -> String {
    format!("{} {}", name(), cmd)
}

const DISK_EXTENSIONS: &[&str] = &["qcow2", "raw", "vmdk", "vhd", "vdi", "img", "qcow"];

/// True if `path` looks like a disk image path (extension or existing file).
pub fn looks_like_disk_path(path: &str) -> bool {
    if path.starts_with('-') {
        return false;
    }
    let p = Path::new(path);
    if let Some(ext) = p.extension().and_then(|e| e.to_str()) {
        let ext_lower = ext.to_ascii_lowercase();
        if DISK_EXTENSIONS.iter().any(|e| *e == ext_lower) {
            return true;
        }
    }
    p.is_file()
}

/// If argv is `{bin} <disk>`, rewrite to `{bin} inspect <disk>`.
pub fn preprocess_args(args: Vec<String>) -> Vec<String> {
    if args.len() == 2 && looks_like_disk_path(&args[1]) {
        vec![args[0].clone(), "inspect".to_string(), args[1].clone()]
    } else {
        args
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_like_disk_by_extension() {
        assert!(looks_like_disk_path("/data/vm.qcow2"));
        assert!(looks_like_disk_path("disk.raw"));
        assert!(!looks_like_disk_path("--help"));
        assert!(!looks_like_disk_path("inspect"));
    }
}
