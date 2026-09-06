// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Azure Blob disk source (az CLI, optional local cache).

use super::cloud_cache;
use super::local::LocalDiskSource;
use super::uri::{DiskSource, DiskSourceMetadata};
use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

pub struct AzureDiskSource {
    local: LocalDiskSource,
    uri: String,
    cached: bool,
}

impl AzureDiskSource {
    pub fn open(uri: &str) -> Result<Self> {
        let blob_url = normalize_azure_uri(uri)?;
        let cached = cloud_cache::cache_enabled();
        let local_path =
            cloud_cache::ensure_cached(&blob_url, |dest| download_azure(&blob_url, dest))?;
        Ok(Self {
            local: LocalDiskSource::open(&local_path)?,
            uri: blob_url,
            cached,
        })
    }
}

/// Accept `https://…blob.core.windows.net/…` or `azure://account/container/blob`.
pub fn normalize_azure_uri(uri: &str) -> Result<String> {
    if uri.starts_with("https://") && uri.contains(".blob.core.windows.net") {
        return Ok(uri.to_string());
    }
    if let Some(rest) = uri.strip_prefix("azure://") {
        let mut parts = rest.splitn(3, '/');
        let account = parts.next().filter(|s| !s.is_empty());
        let container = parts.next().filter(|s| !s.is_empty());
        let blob = parts.next().filter(|s| !s.is_empty());
        match (account, container, blob) {
            (Some(account), Some(container), Some(blob)) => Ok(format!(
                "https://{account}.blob.core.windows.net/{container}/{blob}"
            )),
            _ => anyhow::bail!(
                "invalid azure:// URI (expected azure://account/container/blob/path): {uri}"
            ),
        }
    } else {
        anyhow::bail!("unsupported Azure URI: {uri}");
    }
}

fn download_azure(blob_url: &str, dest: &Path) -> Result<()> {
    let status = Command::new("az")
        .args([
            "storage",
            "blob",
            "download",
            "--blob-url",
            blob_url,
            "--file",
            dest.to_str().unwrap(),
        ])
        .status()
        .context("Failed to run az storage blob download — install Azure CLI")?;
    if !status.success() {
        anyhow::bail!("Azure blob download failed for {blob_url}");
    }
    Ok(())
}

impl Read for AzureDiskSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.local.read(buf)
    }
}

impl Seek for AzureDiskSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.local.seek(pos)
    }
}

impl DiskSource for AzureDiskSource {
    fn metadata(&self) -> DiskSourceMetadata {
        DiskSourceMetadata {
            uri: self.uri.clone(),
            size_bytes: self.local.metadata().size_bytes,
            backend: "azure".to_string(),
        }
    }

    fn local_path(&self) -> Option<&Path> {
        self.local.local_path()
    }
}

impl Drop for AzureDiskSource {
    fn drop(&mut self) {
        if self.cached {
            return;
        }
        if let Some(p) = self.local.local_path() {
            let _ = std::fs::remove_file(p);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::normalize_azure_uri;

    #[test]
    fn azure_scheme_to_https() {
        let u = normalize_azure_uri("azure://myacct/container/path/vm.qcow2").unwrap();
        assert_eq!(
            u,
            "https://myacct.blob.core.windows.net/container/path/vm.qcow2"
        );
    }

    #[test]
    fn https_passthrough() {
        let u = "https://myacct.blob.core.windows.net/c/b.qcow2";
        assert_eq!(normalize_azure_uri(u).unwrap(), u);
    }
}
