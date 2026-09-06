// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! S3 disk source (downloads via AWS CLI, optional local cache).

use super::cloud_cache;
use super::local::LocalDiskSource;
use super::uri::{DiskSource, DiskSourceMetadata};
use anyhow::{Context, Result};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

pub struct S3DiskSource {
    local: LocalDiskSource,
    uri: String,
    /// When true, Drop must not delete the on-disk cache entry.
    cached: bool,
}

impl S3DiskSource {
    pub fn open(uri: &str) -> Result<Self> {
        let object = uri.strip_prefix("s3://").context("Invalid S3 URI")?;
        let s3_uri = format!("s3://{object}");
        let cached = cloud_cache::cache_enabled();
        let local_path = cloud_cache::ensure_cached(uri, |dest| download_s3(&s3_uri, dest))?;
        Ok(Self {
            local: LocalDiskSource::open(&local_path)?,
            uri: uri.to_string(),
            cached,
        })
    }
}

fn download_s3(s3_uri: &str, dest: &Path) -> Result<()> {
    let mut cmd = Command::new("aws");
    cmd.args(["s3", "cp", s3_uri, dest.to_str().unwrap()]);
    // Prefer GuestKit-specific override, then standard AWS SDK/CLI endpoint (MinIO/localstack).
    if let Ok(endpoint) =
        std::env::var("GUESTKIT_S3_ENDPOINT").or_else(|_| std::env::var("AWS_ENDPOINT_URL"))
    {
        if !endpoint.is_empty() {
            cmd.args(["--endpoint-url", &endpoint]);
        }
    }
    let status = cmd
        .status()
        .context("Failed to run aws s3 cp — install AWS CLI and configure credentials")?;
    if !status.success() {
        anyhow::bail!("aws s3 cp failed for {s3_uri}");
    }
    Ok(())
}

impl Read for S3DiskSource {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.local.read(buf)
    }
}

impl Seek for S3DiskSource {
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        self.local.seek(pos)
    }
}

impl DiskSource for S3DiskSource {
    fn metadata(&self) -> DiskSourceMetadata {
        DiskSourceMetadata {
            uri: self.uri.clone(),
            size_bytes: self.local.metadata().size_bytes,
            backend: "s3".to_string(),
        }
    }

    fn local_path(&self) -> Option<&Path> {
        self.local.local_path()
    }
}

impl Drop for S3DiskSource {
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
    #[test]
    fn endpoint_env_names_documented() {
        // Smoke: helpers compile; live aws is integration-only.
        let _ = std::env::var("GUESTKIT_S3_ENDPOINT");
    }
}
