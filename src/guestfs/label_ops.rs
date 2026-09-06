// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Filesystem label and UUID operations for disk image manipulation
//!
//! This implementation provides generic label/UUID management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Set filesystem label (generic)
    ///
    pub fn set_label(&mut self, mountable: &str, label: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_label {} {}", mountable, label);
        }

        // Detect filesystem type first
        let fstype = self.vfs_type(mountable)?;

        match fstype.as_str() {
            "ext2" | "ext3" | "ext4" => self.set_e2label(mountable, label),
            "xfs" => {
                self.xfs_admin(
                    mountable,
                    false,
                    false,
                    false,
                    false,
                    false,
                    Some(label),
                    None,
                )?;
                Ok(())
            }
            "btrfs" => {
                // Use btrfs filesystem label
                self.setup_nbd_if_needed()?;
                let nbd_partition = if let Some(partition_number) =
                    mountable.chars().last().and_then(|c| c.to_digit(10))
                {
                    let nbd_device = self.nbd_device.as_ref().ok_or_else(|| {
                        Error::InvalidState("NBD device not available".to_string())
                    })?;
                    format!(
                        "{}p{}",
                        nbd_device.device_path().display(),
                        partition_number
                    )
                } else {
                    return Err(Error::InvalidFormat(format!(
                        "Invalid device: {}",
                        mountable
                    )));
                };

                let output = Command::new("btrfs")
                    .arg("filesystem")
                    .arg("label")
                    .arg(&nbd_partition)
                    .arg(label)
                    .output()
                    .map_err(|e| Error::CommandFailed(format!("Failed to execute btrfs: {}", e)))?;

                if !output.status.success() {
                    return Err(Error::CommandFailed(format!(
                        "btrfs filesystem label failed: {}",
                        String::from_utf8_lossy(&output.stderr)
                    )));
                }
                Ok(())
            }
            "ntfs" => self.ntfs_set_label(mountable, label),
            "swap" => self.swap_set_label(mountable, label),
            _ => Err(Error::Unsupported(format!(
                "Setting label not supported for {}",
                fstype
            ))),
        }
    }

    /// Set filesystem UUID (generic)
    ///
    pub fn set_uuid(&mut self, device: &str, uuid: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_uuid {} {}", device, uuid);
        }

        let fstype = self.vfs_type(device)?;

        match fstype.as_str() {
            "ext2" | "ext3" | "ext4" => self.set_e2uuid(device, uuid),
            "xfs" => {
                self.xfs_admin(device, false, false, false, false, false, None, Some(uuid))?;
                Ok(())
            }
            "swap" => self.swap_set_uuid(device, uuid),
            _ => Err(Error::Unsupported(format!(
                "Setting UUID not supported for {}",
                fstype
            ))),
        }
    }

    /// Set random UUID
    ///
    pub fn set_uuid_random(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_uuid_random {}", device);
        }

        // Generate random UUID
        let uuid = uuid::Uuid::new_v4().to_string();
        self.set_uuid(device, &uuid)
    }

    /// Get filesystem label (generic, already exists as vfs_label but add alias)
    ///
    pub fn get_label(&mut self, mountable: &str) -> Result<String> {
        self.vfs_label(mountable)
    }

    /// Get filesystem UUID (generic, already exists as vfs_uuid but add alias)
    ///
    pub fn get_uuid(&mut self, device: &str) -> Result<String> {
        self.vfs_uuid(device)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
