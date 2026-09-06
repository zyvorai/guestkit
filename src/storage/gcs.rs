// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! GCS disk source (gsutil or gcloud storage, optional local cache).

use super::cloud_cache;
use super::local::LocalDiskSource;
use super::uri::{DiskSource, DiskSourceMetadata};
use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

pub struct GcsDiskSource {
    local: LocalDiskSource,
    uri: String,
    cached: bool,
}

impl GcsDiskSource {
    pub fn open(uri: &str) -> Result<Self> {
        let cached = cloud_cache::cache_enabled();
        let local_path = cloud_cache::ensure_cached(uri, |dest| download_gcs(uri, dest))?;
        Ok(Self {
            local: LocalDiskSource::open(&local_path)?,
            uri: uri.to_string(),
            cached,
        })
    }
}

fn download_gcs(uri: &str, dest: &Path) -> Result<()> {
    let dest_s = dest.to_str().unwrap();
    if Command::new("gsutil")
        .arg("version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
    {
        let status = Command::new("gsutil")
            .args(["cp", uri, dest_s])
            .status()
            .context("Failed to run gsutil cp — install Google Cloud SDK")?;
        if status.success() {
            return Ok(());
        }
        anyhow::bail!("gsutil cp failed for {uri}");
    }

    // Fallback: `gcloud storage cp` (newer Cloud SDK installs omit gsutil).
    let status = Command::new("gcloud")
        .args(["storage", "cp", uri, dest_s])
        .status()
        .context("Failed to run gcloud storage cp — install Google Cloud SDK (gsutil or gcloud)")?;
    if !status.success() {
        anyhow::bail!("gcloud storage cp failed for {uri}");
    }
    Ok(())
}

impl Read for GcsDiskSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.local.read(buf)
    }
}

impl Seek for GcsDiskSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.local.seek(pos)
    }
}

impl DiskSource for GcsDiskSource {
    fn metadata(&self) -> DiskSourceMetadata {
        DiskSourceMetadata {
            uri: self.uri.clone(),
            size_bytes: self.local.metadata().size_bytes,
            backend: "gcs".to_string(),
        }
    }

    fn local_path(&self) -> Option<&Path> {
        self.local.local_path()
    }
}

impl Drop for GcsDiskSource {
    fn drop(&mut self) {
        if self.cached {
            return;
        }
        if let Some(p) = self.local.local_path() {
            let _ = std::fs::remove_file(p);
        }
    }
}
