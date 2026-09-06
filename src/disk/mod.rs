// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pure Rust disk image handling
//!
//! This module provides pure Rust implementations for reading disk images,
//! parsing partition tables, and detecting filesystems.

pub mod filesystem;
pub mod loop_device;
pub mod nbd;
pub mod partition;
pub mod reader;

pub use filesystem::{FileSystem, FileSystemType};
pub use loop_device::LoopDevice;
pub use nbd::NbdDevice;
pub use partition::{Partition, PartitionTable, PartitionType};
pub use reader::DiskReader;

/// Detect image container format (qcow2, raw, vmdk, …) from magic bytes.
pub fn detect_image_format<P: AsRef<std::path::Path>>(
    path: P,
) -> crate::core::Result<crate::core::DiskFormat> {
    DiskReader::detect_image_format(path)
}
