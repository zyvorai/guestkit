// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Persistent local cache for cloud-pulled disk images.

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

/// Whether cloud object downloads should be cached under `~/.cache/guestkit/cloud`.
pub fn cache_enabled() -> bool {
    match std::env::var("GUESTKIT_CLOUD_CACHE") {
        Ok(v) => !(v == "0" || v.eq_ignore_ascii_case("false") || v.eq_ignore_ascii_case("off")),
        Err(_) => true,
    }
}

fn cache_root() -> Result<PathBuf> {
    if let Ok(dir) = std::env::var("GUESTKIT_CLOUD_CACHE_DIR") {
        let p = PathBuf::from(dir);
        fs::create_dir_all(&p)?;
        return Ok(p);
    }
    let home = std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .context("Could not determine home directory for cloud cache")?;
    let p = PathBuf::from(home)
        .join(".cache")
        .join("guestkit")
        .join("cloud");
    fs::create_dir_all(&p)?;
    Ok(p)
}

fn cache_key(uri: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(uri.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn extension_hint(uri: &str) -> &'static str {
    let lower = uri.to_ascii_lowercase();
    for ext in [".qcow2", ".vmdk", ".vhd", ".vhdx", ".vdi", ".raw", ".img"] {
        if lower.contains(ext) {
            return ext;
        }
    }
    ".img"
}

/// Destination path for a cached cloud object (may not exist yet).
pub fn cache_path_for(uri: &str) -> Result<PathBuf> {
    Ok(cache_root()?.join(format!("{}{}", cache_key(uri), extension_hint(uri))))
}

/// Run `fetch(dest)` only when the URI is not already cached. Returns the local path.
pub fn ensure_cached(uri: &str, fetch: impl FnOnce(&Path) -> Result<()>) -> Result<PathBuf> {
    if !cache_enabled() {
        let tmp = tempfile::NamedTempFile::new().context("create temp download")?;
        let (_file, persisted) = tmp
            .keep()
            .map_err(|e| anyhow::anyhow!("persist temp download: {}", e.error))?;
        fetch(&persisted)?;
        return Ok(persisted);
    }

    let dest = cache_path_for(uri)?;
    if dest.exists() {
        let meta = fs::metadata(&dest)?;
        if meta.len() > 0 {
            log::debug!("cloud cache hit for {uri} → {}", dest.display());
            return Ok(dest);
        }
    }

    let partial = dest.with_extension(format!(
        "{}.partial",
        dest.extension().and_then(|e| e.to_str()).unwrap_or("img")
    ));
    if partial.exists() {
        let _ = fs::remove_file(&partial);
    }
    fetch(&partial)?;
    fs::rename(&partial, &dest)
        .with_context(|| format!("finalize cloud cache {}", dest.display()))?;
    log::debug!("cloud cache store for {uri} → {}", dest.display());
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_key_stable() {
        assert_eq!(cache_key("s3://b/a.qcow2"), cache_key("s3://b/a.qcow2"));
        assert_ne!(cache_key("s3://b/a.qcow2"), cache_key("s3://b/b.qcow2"));
    }

    #[test]
    fn extension_from_uri() {
        assert_eq!(extension_hint("s3://bucket/vm.qcow2"), ".qcow2");
        assert_eq!(extension_hint("gs://b/disk.vmdk"), ".vmdk");
        assert_eq!(extension_hint("s3://b/noext"), ".img");
    }
}
