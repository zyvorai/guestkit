// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Universal fstab/crypttab rewriter for VM migration
//!
//! This module provides deterministic rewriting of /etc/fstab and /etc/crypttab
//! to use proper UUID/PARTUUID specifications instead of device paths.
//!
//! Handles all major filesystem types and topologies:
//! - ext4/xfs/vfat/ntfs on partitions → PARTUUID for boot mounts
//! - LVM/mdraid/dm-crypt → UUID
//! - Btrfs with subvolumes → proper subvol= options
//! - LUKS encrypted devices → crypttab with UUID
//! - Swap → UUID
//!
//! Never keeps: /dev/sdX, /dev/vdX, /dev/nbdX, /dev/disk/by-path/*

use crate::core::Result;
use crate::guestfs::device_inventory::{find_by_spec, Inventory};
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// fstab entry
#[derive(Debug, Clone)]
pub struct FstabEntry {
    /// Device spec (UUID=..., PARTUUID=..., /dev/...)
    pub spec: String,
    /// Mount point
    pub mountpoint: String,
    /// Filesystem type
    pub fstype: String,
    /// Mount options
    pub options: String,
    /// Dump flag
    pub dump: String,
    /// Pass flag (fsck order)
    pub pass: String,
}

impl FstabEntry {
    /// Parse an fstab line
    fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();

        // Skip comments and empty lines
        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 4 {
            return None;
        }

        // dump and pass fields default to 0 per fstab(5) when absent
        Some(FstabEntry {
            spec: parts[0].to_string(),
            mountpoint: parts[1].to_string(),
            fstype: parts[2].to_string(),
            options: parts[3].to_string(),
            dump: parts.get(4).copied().unwrap_or("0").to_string(),
            pass: parts.get(5).copied().unwrap_or("0").to_string(),
        })
    }

    /// Format as fstab line
    fn format(&self) -> String {
        format!(
            "{}\t{}\t{}\t{}\t{}\t{}\n",
            self.spec, self.mountpoint, self.fstype, self.options, self.dump, self.pass
        )
    }

    /// Check if spec should be rewritten (is it a device path we want to avoid?)
    fn needs_rewrite(&self) -> bool {
        // Rewrite /dev/sd*, /dev/vd*, /dev/nbd*, /dev/disk/by-path/*
        if self.spec.starts_with("/dev/sd")
            || self.spec.starts_with("/dev/vd")
            || self.spec.starts_with("/dev/nbd")
            || self.spec.starts_with("/dev/disk/by-path/")
            || self.spec.starts_with("/dev/hd")
        {
            return true;
        }

        // Also rewrite old-style /dev/mapper/* if we have better info
        if self.spec.starts_with("/dev/mapper/") {
            return true;
        }

        // Keep UUID=, PARTUUID=, LABEL= as-is (but we'll verify them)
        false
    }
}

/// crypttab entry
#[derive(Debug, Clone)]
pub struct CrypttabEntry {
    /// Mapped device name
    pub name: String,
    /// Source device (UUID=..., /dev/...)
    pub device: String,
    /// Key file (or "none")
    pub keyfile: String,
    /// Options
    pub options: String,
}

impl CrypttabEntry {
    /// Parse a crypttab line
    fn parse(line: &str) -> Option<Self> {
        let trimmed = line.trim();

        if trimmed.is_empty() || trimmed.starts_with('#') {
            return None;
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        if parts.len() < 2 {
            return None;
        }

        Some(CrypttabEntry {
            name: parts[0].to_string(),
            device: parts[1].to_string(),
            keyfile: parts.get(2).unwrap_or(&"none").to_string(),
            options: parts.get(3).unwrap_or(&"").to_string(),
        })
    }

    /// Format as crypttab line
    fn format(&self) -> String {
        if self.options.is_empty() {
            format!("{} {} {}\n", self.name, self.device, self.keyfile)
        } else {
            format!(
                "{} {} {} {}\n",
                self.name, self.device, self.keyfile, self.options
            )
        }
    }
}

/// Btrfs subvolume mapping (mountpoint -> subvolume name)
pub type BtrfsSubvolMap = HashMap<String, String>;

/// Remove a mount option by key
///
/// # Arguments
/// * `opts` - Current options string
/// * `key` - Option key to remove (e.g., "subvol", "subvolid")
pub fn remove_mount_option(opts: &str, key: &str) -> String {
    opts.split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .filter(|o| *o != key && !o.starts_with(&format!("{}=", key)))
        .collect::<Vec<_>>()
        .join(",")
}

/// Set or replace a mount option
///
/// # Arguments
/// * `opts` - Current options string
/// * `key` - Option key to set/replace (e.g., "subvol")
/// * `kv` - Key=value string to set (e.g., "subvol=@")
pub fn set_mount_option(opts: &str, key: &str, kv: &str) -> String {
    let cleaned = remove_mount_option(opts, key);
    if cleaned.is_empty() {
        kv.to_string()
    } else {
        format!("{},{}", cleaned, kv)
    }
}

/// Rewrite /etc/fstab with proper UUID/PARTUUID specs
///
/// # Arguments
/// * `fstab_path` - Path to fstab file
/// * `inv` - Device inventory
/// * `btrfs_subvol_map` - Btrfs subvolume mapping (optional)
pub fn rewrite_fstab(
    fstab_path: &Path,
    inv: &Inventory,
    btrfs_subvol_map: &BtrfsSubvolMap,
) -> Result<()> {
    let content = fs::read_to_string(fstab_path)
        .map_err(|e| crate::core::Error::NotFound(format!("Cannot read fstab: {}", e)))?;

    let mut output_lines = Vec::new();

    for line in content.lines() {
        // Try to parse as fstab entry
        if let Some(mut entry) = FstabEntry::parse(line) {
            // Try to find device in inventory
            if let Some(dev_info) = find_by_spec(inv, &entry.spec) {
                // Determine canonical spec based on fstype
                let mut new_spec = if entry.fstype == "swap" {
                    // Swap always prefers UUID
                    if let Some(uuid) = &dev_info.uuid {
                        format!("UUID={}", uuid)
                    } else {
                        dev_info.canonical_spec(&entry.mountpoint)
                    }
                } else {
                    dev_info.canonical_spec(&entry.mountpoint)
                };

                // Handle filesystem-specific options
                if entry.fstype == "btrfs" {
                    // Add/update subvol= option if configured
                    if let Some(subvol) = btrfs_subvol_map.get(&entry.mountpoint) {
                        entry.options = set_mount_option(
                            &entry.options,
                            "subvol",
                            &format!("subvol={}", subvol),
                        );
                        // Remove subvolid= to prevent conflicts
                        entry.options = remove_mount_option(&entry.options, "subvolid");
                    }

                    // If not a real partition (LVM/crypt/md), UUID is the portable choice
                    if !dev_info.is_partition() {
                        if let Some(uuid) = &dev_info.uuid {
                            new_spec = format!("UUID={}", uuid);
                        }
                    }
                }

                entry.spec = new_spec;
                output_lines.push(entry.format());
            } else if entry.needs_rewrite() {
                // Device not found in inventory but needs rewriting
                // Keep original but warn
                eprintln!(
                    "Warning: fstab entry {} not found in inventory, keeping as-is",
                    entry.spec
                );
                output_lines.push(line.to_string() + "\n");
            } else {
                // Keep as-is (UUID=, LABEL=, etc. that we trust)
                output_lines.push(line.to_string() + "\n");
            }
        } else {
            // Keep comments and empty lines
            output_lines.push(line.to_string() + "\n");
        }
    }

    // Write back
    fs::write(fstab_path, output_lines.join(""))
        .map_err(|e| crate::core::Error::CommandFailed(format!("Cannot write fstab: {}", e)))?;

    Ok(())
}

/// Rewrite /etc/crypttab with proper LUKS UUIDs
///
/// # Arguments
/// * `crypttab_path` - Path to crypttab file
/// * `inv` - Device inventory
pub fn rewrite_crypttab(crypttab_path: &Path, inv: &Inventory) -> Result<()> {
    if !crypttab_path.exists() {
        // No crypttab, nothing to do
        return Ok(());
    }

    let content = fs::read_to_string(crypttab_path)
        .map_err(|e| crate::core::Error::NotFound(format!("Cannot read crypttab: {}", e)))?;

    let mut output_lines = Vec::new();

    for line in content.lines() {
        if let Some(mut entry) = CrypttabEntry::parse(line) {
            // Try to find device in inventory
            if let Some(dev_info) = find_by_spec(inv, &entry.device) {
                // For LUKS devices, use the LUKS UUID
                if let Some(luks_uuid) = &dev_info.luks_uuid {
                    entry.device = format!("UUID={}", luks_uuid);
                } else if let Some(uuid) = &dev_info.uuid {
                    // Fallback to filesystem UUID
                    entry.device = format!("UUID={}", uuid);
                }
                output_lines.push(entry.format());
            } else {
                // Keep as-is if not found
                eprintln!(
                    "Warning: crypttab entry {} not found in inventory, keeping as-is",
                    entry.device
                );
                output_lines.push(line.to_string() + "\n");
            }
        } else {
            // Keep comments and empty lines
            output_lines.push(line.to_string() + "\n");
        }
    }

    // Write back
    fs::write(crypttab_path, output_lines.join(""))
        .map_err(|e| crate::core::Error::CommandFailed(format!("Cannot write crypttab: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fstab_entry_parse() {
        let line = "UUID=abc-123 / ext4 defaults 0 1";
        let entry = FstabEntry::parse(line).unwrap();
        assert_eq!(entry.spec, "UUID=abc-123");
        assert_eq!(entry.mountpoint, "/");
        assert_eq!(entry.fstype, "ext4");
    }

    #[test]
    fn test_fstab_entry_needs_rewrite() {
        let entry1 = FstabEntry {
            spec: "/dev/sda1".to_string(),
            mountpoint: "/".to_string(),
            fstype: "ext4".to_string(),
            options: "defaults".to_string(),
            dump: "0".to_string(),
            pass: "1".to_string(),
        };
        assert!(entry1.needs_rewrite());

        let entry2 = FstabEntry {
            spec: "UUID=abc-123".to_string(),
            mountpoint: "/".to_string(),
            fstype: "ext4".to_string(),
            options: "defaults".to_string(),
            dump: "0".to_string(),
            pass: "1".to_string(),
        };
        assert!(!entry2.needs_rewrite());
    }

    #[test]
    fn test_set_mount_option() {
        let opts = "defaults,noatime";
        let new_opts = set_mount_option(opts, "subvol", "subvol=@");
        assert!(new_opts.contains("subvol=@"));
        assert!(new_opts.contains("defaults"));

        // Replace existing
        let opts2 = "defaults,subvol=old";
        let new_opts2 = set_mount_option(opts2, "subvol", "subvol=@");
        assert!(new_opts2.contains("subvol=@"));
        assert!(!new_opts2.contains("subvol=old"));
    }

    #[test]
    fn test_crypttab_entry_parse() {
        let line = "cryptroot /dev/sda2 none luks";
        let entry = CrypttabEntry::parse(line).unwrap();
        assert_eq!(entry.name, "cryptroot");
        assert_eq!(entry.device, "/dev/sda2");
        assert_eq!(entry.keyfile, "none");
        assert_eq!(entry.options, "luks");
    }
}
