// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Device inventory for filesystem rewriting
//!
//! This module builds a complete inventory of all block devices with their
//! UUIDs, PARTUUIDs, labels, and types. This is used for deterministic
//! fstab/crypttab rewriting during VM migration.

use crate::core::{Error, Result};
use std::collections::HashMap;
use std::process::Command;

/// Block device type from lsblk
#[derive(Debug, Clone, PartialEq)]
pub enum BlockType {
    Part,  // partition
    Lvm,   // LVM logical volume
    Crypt, // dm-crypt/LUKS
    Raid,  // mdraid
    Disk,  // whole disk
    Other(String),
}

impl BlockType {
    fn from_str(s: &str) -> Self {
        match s {
            "part" => BlockType::Part,
            "lvm" => BlockType::Lvm,
            "crypt" => BlockType::Crypt,
            "raid" => BlockType::Raid,
            "disk" => BlockType::Disk,
            other => BlockType::Other(other.to_string()),
        }
    }
}

/// Complete device information
#[derive(Debug, Clone)]
pub struct DevInfo {
    /// Device path (e.g., /dev/sda1, /dev/mapper/vg-root)
    pub dev: String,
    /// Filesystem type (ext4, xfs, btrfs, crypto_LUKS, swap, vfat, ntfs, etc.)
    pub fstype: Option<String>,
    /// Filesystem UUID
    pub uuid: Option<String>,
    /// Filesystem label
    pub label: Option<String>,
    /// Partition UUID (for GPT partitions)
    pub partuuid: Option<String>,
    /// Block device type from lsblk
    pub blk_type: BlockType,
    /// LUKS container UUID (if this is a crypto_LUKS device)
    pub luks_uuid: Option<String>,
}

impl DevInfo {
    /// Check if this is a partition (real GPT/MBR partition)
    pub fn is_partition(&self) -> bool {
        self.blk_type == BlockType::Part && self.partuuid.is_some()
    }

    /// Check if this is on LVM/md/mapper
    pub fn is_logical_volume(&self) -> bool {
        matches!(
            self.blk_type,
            BlockType::Lvm | BlockType::Raid | BlockType::Crypt
        ) || self.dev.starts_with("/dev/mapper/")
    }

    /// Get the canonical spec for this device for a given mountpoint
    ///
    /// Rules:
    /// - Root/boot on partition → PARTUUID=
    /// - Everything else with UUID → UUID=
    /// - Fallback to PARTUUID or LABEL
    pub fn canonical_spec(&self, mountpoint: &str) -> String {
        // Prefer PARTUUID for boot-critical mounts on real partitions
        if self.prefer_partuuid(mountpoint) {
            return format!("PARTUUID={}", self.partuuid.as_ref().unwrap());
        }

        // Prefer filesystem UUID for everything else
        if let Some(uuid) = &self.uuid {
            return format!("UUID={}", uuid);
        }

        // Fallbacks
        if let Some(partuuid) = &self.partuuid {
            return format!("PARTUUID={}", partuuid);
        }

        if let Some(label) = &self.label {
            return format!("LABEL={}", label);
        }

        // Last resort: device path (not ideal)
        self.dev.clone()
    }

    /// Check if PARTUUID should be preferred for this mountpoint
    fn prefer_partuuid(&self, mountpoint: &str) -> bool {
        // Only prefer PARTUUID if:
        // - it's a real partition
        // - mount is boot-critical (/, /boot, /boot/efi)
        self.is_partition() && matches!(mountpoint, "/" | "/boot" | "/boot/efi" | "/efi")
    }
}

/// Device inventory with multiple lookup indexes
#[derive(Debug, Clone)]
pub struct Inventory {
    /// Device path -> DevInfo
    pub by_dev: HashMap<String, DevInfo>,
    /// UUID -> device path
    pub by_uuid: HashMap<String, String>,
    /// PARTUUID -> device path
    pub by_partuuid: HashMap<String, String>,
    /// LABEL -> device path (only unique labels)
    pub by_label: HashMap<String, String>,
}

/// Build device inventory using blkid and lsblk
///
/// This queries all available block devices and builds a complete
/// inventory with UUIDs, labels, types, etc.
///
/// Important: Labels that appear on multiple devices are excluded
/// from the by_label index to prevent ambiguity.
pub fn build_inventory(devices: &[String]) -> Result<Inventory> {
    let mut by_dev = HashMap::new();
    let mut by_uuid = HashMap::new();
    let mut by_partuuid = HashMap::new();
    let mut by_label = HashMap::new();
    let mut label_counts: HashMap<String, u32> = HashMap::new();

    for input_dev in devices {
        // Canonicalize device path to resolve /dev/disk/by-* symlinks
        let dev = canonicalize_dev_path(input_dev).unwrap_or_else(|| input_dev.clone());

        let info = probe_device(&dev)?;

        // Track label collisions
        if let Some(label) = &info.label {
            *label_counts.entry(label.clone()).or_insert(0) += 1;
            by_label.entry(label.clone()).or_insert_with(|| dev.clone());
        }

        // Build UUID indexes
        if let Some(uuid) = &info.uuid {
            by_uuid.entry(uuid.clone()).or_insert_with(|| dev.clone());
        }
        if let Some(partuuid) = &info.partuuid {
            by_partuuid
                .entry(partuuid.clone())
                .or_insert_with(|| dev.clone());
        }

        by_dev.insert(dev, info);
    }

    // Remove colliding labels (not safe to use)
    for (label, count) in label_counts {
        if count > 1 {
            by_label.remove(&label);
        }
    }

    Ok(Inventory {
        by_dev,
        by_uuid,
        by_partuuid,
        by_label,
    })
}

/// Canonicalize device path to resolve /dev/disk/by-* symlinks
fn canonicalize_dev_path(path: &str) -> Option<String> {
    if !path.starts_with("/dev/") {
        return None;
    }
    std::fs::canonicalize(path)
        .ok()
        .map(|p| p.to_string_lossy().to_string())
}

/// Probe a single device for all its information
fn probe_device(dev: &str) -> Result<DevInfo> {
    // Get blkid information
    let blkid_data = blkid_export(dev)?;

    let fstype = blkid_data.get("TYPE").cloned();
    let uuid = blkid_data.get("UUID").cloned();
    let label = blkid_data.get("LABEL").cloned();
    let partuuid = blkid_data.get("PARTUUID").cloned();

    // Get lsblk type
    let blk_type = lsblk_type(dev)?;

    // Check if this is LUKS
    let luks_uuid = if fstype.as_deref() == Some("crypto_LUKS") {
        uuid.clone()
    } else {
        None
    };

    Ok(DevInfo {
        dev: dev.to_string(),
        fstype,
        uuid,
        label,
        partuuid,
        blk_type,
        luks_uuid,
    })
}

/// Get blkid export data as key-value map
pub(crate) fn blkid_export(dev: &str) -> Result<HashMap<String, String>> {
    let output = Command::new("blkid")
        .arg("-o")
        .arg("export")
        .arg(dev)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to run blkid: {}", e)))?;

    let mut data = HashMap::new();

    if !output.status.success() {
        // Device might not have a filesystem, that's ok
        return Ok(data);
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    for line in stdout.lines() {
        if let Some((key, value)) = line.split_once('=') {
            data.insert(key.to_string(), value.to_string());
        }
    }

    Ok(data)
}

/// Get block device type from lsblk
fn lsblk_type(dev: &str) -> Result<BlockType> {
    let output = Command::new("lsblk")
        .arg("-no")
        .arg("TYPE")
        .arg(dev)
        .output()
        .map_err(|e| Error::CommandFailed(format!("Failed to run lsblk: {}", e)))?;

    if !output.status.success() {
        return Ok(BlockType::Other("unknown".to_string()));
    }

    let type_str = String::from_utf8_lossy(&output.stdout).trim().to_string();
    Ok(BlockType::from_str(&type_str))
}

/// Find device by spec (UUID=..., PARTUUID=..., LABEL=..., or /dev/...)
///
/// Handles /dev/disk/by-* specs via canonicalization.
pub fn find_by_spec<'a>(inv: &'a Inventory, spec: &str) -> Option<&'a DevInfo> {
    let spec = spec.trim();

    // UUID=...
    if let Some(uuid) = spec.strip_prefix("UUID=") {
        return inv.by_uuid.get(uuid).and_then(|dev| inv.by_dev.get(dev));
    }

    // PARTUUID=...
    if let Some(partuuid) = spec.strip_prefix("PARTUUID=") {
        return inv
            .by_partuuid
            .get(partuuid)
            .and_then(|dev| inv.by_dev.get(dev));
    }

    // LABEL=...
    if let Some(label) = spec.strip_prefix("LABEL=") {
        return inv.by_label.get(label).and_then(|dev| inv.by_dev.get(dev));
    }

    // Direct device path
    if spec.starts_with("/dev/") {
        // Try exact match first
        if let Some(info) = inv.by_dev.get(spec) {
            return Some(info);
        }
        // Canonicalize /dev/disk/by-* and try again
        if let Some(canonical) = canonicalize_dev_path(spec) {
            return inv.by_dev.get(&canonical);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_block_type_parsing() {
        assert_eq!(BlockType::from_str("part"), BlockType::Part);
        assert_eq!(BlockType::from_str("lvm"), BlockType::Lvm);
        assert_eq!(BlockType::from_str("crypt"), BlockType::Crypt);
    }

    #[test]
    fn test_canonical_spec_partition() {
        let dev = DevInfo {
            dev: "/dev/sda1".to_string(),
            fstype: Some("ext4".to_string()),
            uuid: Some("abc-123".to_string()),
            label: None,
            partuuid: Some("xyz-789".to_string()),
            blk_type: BlockType::Part,
            luks_uuid: None,
        };

        // Root on partition should prefer PARTUUID
        assert_eq!(dev.canonical_spec("/"), "PARTUUID=xyz-789");
        assert_eq!(dev.canonical_spec("/boot"), "PARTUUID=xyz-789");

        // Non-boot mount should prefer UUID
        assert_eq!(dev.canonical_spec("/home"), "UUID=abc-123");
    }

    #[test]
    fn test_canonical_spec_lvm() {
        let dev = DevInfo {
            dev: "/dev/mapper/vg-root".to_string(),
            fstype: Some("ext4".to_string()),
            uuid: Some("abc-123".to_string()),
            label: None,
            partuuid: None,
            blk_type: BlockType::Lvm,
            luks_uuid: None,
        };

        // LVM should always use UUID
        assert_eq!(dev.canonical_spec("/"), "UUID=abc-123");
        assert_eq!(dev.canonical_spec("/boot"), "UUID=abc-123");
        assert_eq!(dev.canonical_spec("/home"), "UUID=abc-123");
    }
}
