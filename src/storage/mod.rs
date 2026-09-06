// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cloud and local disk source abstraction.

pub mod cloud_cache;
pub mod local;
pub mod uri;

pub use local::LocalDiskSource;
pub use uri::{open_disk_source, resolve_to_local_path, DiskSource, DiskSourceMetadata};

#[cfg(feature = "cloud-azure")]
pub mod azure;
#[cfg(feature = "cloud-gcs")]
pub mod gcs;
#[cfg(feature = "cloud-s3")]
pub mod s3;
