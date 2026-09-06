// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Input validation utilities for safe operations

use crate::core::error::{Error, Result};

/// Input validator for filesystem operations
pub struct Validator;

impl Validator {
    /// Validate filesystem type string
    ///
    /// Only allows known safe filesystem types
    pub fn validate_fstype(fstype: &str) -> Result<()> {
        const ALLOWED_FS: &[&str] = &[
            "ext2", "ext3", "ext4", "xfs", "btrfs", "ntfs", "vfat", "fat", "swap", "f2fs",
            "reiserfs", "jfs", "minix", "hfs", "hfsplus",
        ];

        if !ALLOWED_FS.contains(&fstype) {
            return Err(Error::InvalidFormat(format!(
                "Unsupported filesystem type: {}. Allowed: {:?}",
                fstype, ALLOWED_FS
            )));
        }
        Ok(())
    }

    /// Validate file mode (permissions)
    ///
    /// Ensures mode is within valid range (0-07777)
    pub fn validate_mode(mode: i32) -> Result<()> {
        if !(0..=0o7777).contains(&mode) {
            return Err(Error::InvalidFormat(format!(
                "Invalid mode: {:o} (must be 0-07777)",
                mode
            )));
        }
        Ok(())
    }

    /// Validate ownership (UID/GID)
    ///
    /// Ensures UID and GID are non-negative
    pub fn validate_ownership(uid: i32, gid: i32) -> Result<()> {
        if uid < 0 {
            return Err(Error::InvalidFormat(format!(
                "Invalid UID: {} (must be non-negative)",
                uid
            )));
        }
        if gid < 0 {
            return Err(Error::InvalidFormat(format!(
                "Invalid GID: {} (must be non-negative)",
                gid
            )));
        }
        Ok(())
    }

    /// Validate partition number
    ///
    /// Ensures partition number is reasonable (1-128)
    pub fn validate_partition_number(num: u32) -> Result<()> {
        if num == 0 {
            return Err(Error::InvalidFormat(
                "Partition number cannot be 0".to_string(),
            ));
        }
        if num > 128 {
            return Err(Error::InvalidFormat(format!(
                "Partition number {} is too large (max 128)",
                num
            )));
        }
        Ok(())
    }

    /// Validate archive format
    ///
    /// Only allows known safe archive formats
    pub fn validate_archive_format(format: &str) -> Result<()> {
        const ALLOWED_FORMATS: &[&str] = &[
            "tar", "tgz", "tbz", "txz", "zip", "cpio", "newc", "crc", "odc",
        ];

        if !ALLOWED_FORMATS.contains(&format) {
            return Err(Error::InvalidFormat(format!(
                "Unsupported archive format: {}. Allowed: {:?}",
                format, ALLOWED_FORMATS
            )));
        }
        Ok(())
    }

    /// Validate string length
    pub fn validate_string_length(s: &str, max_len: usize, field_name: &str) -> Result<()> {
        if s.len() > max_len {
            return Err(Error::InvalidFormat(format!(
                "{} exceeds maximum length {} (got {})",
                field_name,
                max_len,
                s.len()
            )));
        }
        Ok(())
    }

    /// Validate device name format
    pub fn validate_device_name(device: &str) -> Result<()> {
        if !device.starts_with("/dev/") {
            return Err(Error::InvalidFormat(format!(
                "Device name must start with /dev/: {}",
                device
            )));
        }

        Self::validate_string_length(device, 255, "Device name")?;

        Ok(())
    }

    /// Validate mount options string
    pub fn validate_mount_options(options: &str) -> Result<()> {
        // Check for dangerous characters
        const DANGEROUS_CHARS: &[char] = &[';', '&', '|', '$', '`', '\n', '\r'];

        for ch in DANGEROUS_CHARS {
            if options.contains(*ch) {
                return Err(Error::InvalidFormat(format!(
                    "Mount options contain dangerous character '{}'",
                    ch
                )));
            }
        }

        Self::validate_string_length(options, 4096, "Mount options")?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_fstype() {
        assert!(Validator::validate_fstype("ext4").is_ok());
        assert!(Validator::validate_fstype("xfs").is_ok());
        assert!(Validator::validate_fstype("btrfs").is_ok());
        assert!(Validator::validate_fstype("invalid").is_err());
        assert!(Validator::validate_fstype("").is_err());
    }

    #[test]
    fn test_validate_mode() {
        assert!(Validator::validate_mode(0o644).is_ok());
        assert!(Validator::validate_mode(0o755).is_ok());
        assert!(Validator::validate_mode(0).is_ok());
        assert!(Validator::validate_mode(0o7777).is_ok());
        assert!(Validator::validate_mode(-1).is_err());
        assert!(Validator::validate_mode(0o10000).is_err());
    }

    #[test]
    fn test_validate_ownership() {
        assert!(Validator::validate_ownership(0, 0).is_ok());
        assert!(Validator::validate_ownership(1000, 1000).is_ok());
        assert!(Validator::validate_ownership(-1, 0).is_err());
        assert!(Validator::validate_ownership(0, -1).is_err());
    }

    #[test]
    fn test_validate_partition_number() {
        assert!(Validator::validate_partition_number(1).is_ok());
        assert!(Validator::validate_partition_number(128).is_ok());
        assert!(Validator::validate_partition_number(0).is_err());
        assert!(Validator::validate_partition_number(129).is_err());
    }

    #[test]
    fn test_validate_archive_format() {
        assert!(Validator::validate_archive_format("tar").is_ok());
        assert!(Validator::validate_archive_format("zip").is_ok());
        assert!(Validator::validate_archive_format("cpio").is_ok());
        assert!(Validator::validate_archive_format("invalid").is_err());
    }

    #[test]
    fn test_validate_device_name() {
        assert!(Validator::validate_device_name("/dev/sda1").is_ok());
        assert!(Validator::validate_device_name("/dev/vda").is_ok());
        assert!(Validator::validate_device_name("sda1").is_err());
        assert!(Validator::validate_device_name("/home/file").is_err());
    }

    #[test]
    fn test_validate_mount_options() {
        assert!(Validator::validate_mount_options("ro,noatime").is_ok());
        assert!(Validator::validate_mount_options("rw").is_ok());
        assert!(Validator::validate_mount_options("ro;malicious").is_err());
        assert!(Validator::validate_mount_options("ro&attack").is_err());
        assert!(Validator::validate_mount_options("ro|cmd").is_err());
    }
}
