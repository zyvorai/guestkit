// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Disk source trait and URI resolution.

use anyhow::Result;
use std::io::{Read, Seek};
use std::path::{Path, PathBuf};

/// Metadata about a disk source.
#[derive(Debug, Clone)]
pub struct DiskSourceMetadata {
    pub uri: String,
    pub size_bytes: Option<u64>,
    pub backend: String,
}

/// Abstraction for reading disk images from local or cloud storage.
pub trait DiskSource: Read + Seek {
    fn metadata(&self) -> DiskSourceMetadata;
    fn local_path(&self) -> Option<&Path>;
}

/// Open a disk source from a URI or local path.
pub fn open_disk_source(uri: &str) -> Result<Box<dyn DiskSource>> {
    if uri.starts_with("s3://") {
        #[cfg(feature = "cloud-s3")]
        {
            return Ok(Box::new(crate::storage::s3::S3DiskSource::open(uri)?));
        }
        #[cfg(not(feature = "cloud-s3"))]
        anyhow::bail!("S3 support not enabled. Rebuild with --features cloud-s3");
    }
    if uri.starts_with("azure://")
        || (uri.starts_with("https://") && uri.contains(".blob.core.windows.net"))
    {
        #[cfg(feature = "cloud-azure")]
        {
            return Ok(Box::new(crate::storage::azure::AzureDiskSource::open(uri)?));
        }
        #[cfg(not(feature = "cloud-azure"))]
        anyhow::bail!("Azure support not enabled. Rebuild with --features cloud-azure");
    }
    if uri.starts_with("gs://") {
        #[cfg(feature = "cloud-gcs")]
        {
            return Ok(Box::new(crate::storage::gcs::GcsDiskSource::open(uri)?));
        }
        #[cfg(not(feature = "cloud-gcs"))]
        anyhow::bail!("GCS support not enabled. Rebuild with --features cloud-gcs");
    }

    let path = PathBuf::from(uri);
    if !path.exists() {
        anyhow::bail!("Disk image not found: {}", uri);
    }
    Ok(Box::new(crate::storage::local::LocalDiskSource::open(
        &path,
    )?))
}

/// Resolve a disk URI to a local path (downloads cloud objects to temp file).
pub fn resolve_to_local_path(uri: &str) -> Result<PathBuf> {
    let source = open_disk_source(uri)?;
    if let Some(path) = source.local_path() {
        return Ok(path.to_path_buf());
    }
    anyhow::bail!(
        "Cloud download requires feature-enabled backend for: {}",
        uri
    )
}
