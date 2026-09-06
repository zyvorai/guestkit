// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Device and filesystem operations for disk image manipulation

use crate::core::{Error, Result};
use crate::disk::FileSystem;
use crate::guestfs::Guestfs;
use std::collections::HashMap;
use std::path::PathBuf;

impl Guestfs {
    /// List all block devices
    ///
    pub fn list_devices(&self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        // Return list of drives added
        let mut devices = Vec::new();
        for (i, _) in self.drives.iter().enumerate() {
            if i >= 26 {
                return Err(Error::InvalidOperation(format!(
                    "Too many drives ({}) - maximum 26 supported",
                    self.drives.len()
                )));
            }
            let letter = (b'a' + i as u8) as char;
            devices.push(format!("/dev/sd{}", letter));
        }

        Ok(devices)
    }

    /// List all partitions
    ///
    pub fn list_partitions(&self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        let partition_table = self.partition_table()?;
        let mut partitions = Vec::new();

        for partition in partition_table.partitions() {
            partitions.push(format!("/dev/sda{}", partition.number));
        }

        Ok(partitions)
    }

    /// List all filesystems detected
    ///
    pub fn list_filesystems(&mut self) -> Result<HashMap<String, String>> {
        self.ensure_ready()?;

        let mut filesystems = HashMap::new();

        // Clone partition data to avoid borrow checker issues
        let partitions: Vec<_> = {
            let partition_table = self.partition_table()?;
            partition_table.partitions().to_vec()
        };

        for partition in &partitions {
            let device_name = format!("/dev/sda{}", partition.number);

            let reader = self
                .reader
                .as_mut()
                .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;
            if let Ok(fs) = FileSystem::detect(reader, partition) {
                let fs_type = match fs.fs_type() {
                    crate::disk::FileSystemType::Ext => "ext4",
                    crate::disk::FileSystemType::Ntfs => "ntfs",
                    crate::disk::FileSystemType::Fat32 => "vfat",
                    crate::disk::FileSystemType::ExFat => "exfat",
                    crate::disk::FileSystemType::Xfs => "xfs",
                    crate::disk::FileSystemType::Btrfs => "btrfs",
                    crate::disk::FileSystemType::Zfs => "zfs",
                    crate::disk::FileSystemType::Ufs => "ufs",
                    crate::disk::FileSystemType::HfsPlus => "hfsplus",
                    crate::disk::FileSystemType::Apfs => "apfs",
                    crate::disk::FileSystemType::Iso9660 => "iso9660",
                    crate::disk::FileSystemType::Swap => "swap",
                    crate::disk::FileSystemType::Unknown => "unknown",
                };

                filesystems.insert(device_name, fs_type.to_string());
            }
        }

        Ok(filesystems)
    }

    /// Get filesystem type
    ///
    pub fn vfs_type(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        // For LVM volumes (/dev/mapper/* or /dev/vgname/lvname), use blkid directly
        if device.starts_with("/dev/mapper/")
            || (device.starts_with("/dev/") && device.matches('/').count() >= 3)
        {
            // Use blkid to detect filesystem type on LVM volumes
            let need_sudo = crate::guestfs::mount::need_sudo();

            let mut cmd = if need_sudo {
                let mut sudo_cmd = std::process::Command::new("sudo");
                sudo_cmd.arg("blkid");
                sudo_cmd
            } else {
                std::process::Command::new("blkid")
            };

            let output = cmd
                .arg("-o")
                .arg("value")
                .arg("-s")
                .arg("TYPE")
                .arg(device)
                .output()
                .map_err(|e| Error::CommandFailed(format!("Failed to run blkid: {}", e)))?;

            if output.status.success() {
                let fs_type = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !fs_type.is_empty() {
                    return Ok(fs_type);
                }
            }

            // If blkid fails, return unknown
            return Ok("unknown".to_string());
        }

        // For regular partitions, use the existing detection logic
        let partition_num = self.parse_device_name(device)?;

        // Clone partition to avoid borrow checker issues
        let partition = {
            let partition_table = self.partition_table()?;
            partition_table
                .partitions()
                .iter()
                .find(|p| p.number == partition_num)
                .cloned()
                .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partition_num)))?
        };

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;
        let fs = FileSystem::detect(reader, &partition)?;

        let fs_type = match fs.fs_type() {
            crate::disk::FileSystemType::Ext => "ext4",
            crate::disk::FileSystemType::Ntfs => "ntfs",
            crate::disk::FileSystemType::Fat32 => "vfat",
            crate::disk::FileSystemType::ExFat => "exfat",
            crate::disk::FileSystemType::Xfs => "xfs",
            crate::disk::FileSystemType::Btrfs => "btrfs",
            crate::disk::FileSystemType::Zfs => "zfs",
            crate::disk::FileSystemType::Ufs => "ufs",
            crate::disk::FileSystemType::HfsPlus => "hfsplus",
            crate::disk::FileSystemType::Apfs => "apfs",
            crate::disk::FileSystemType::Iso9660 => "iso9660",
            crate::disk::FileSystemType::Swap => "swap",
            crate::disk::FileSystemType::Unknown => "unknown",
        };

        Ok(fs_type.to_string())
    }

    /// Get filesystem label
    ///
    pub fn vfs_label(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        let partition_num = self.parse_device_name(device)?;

        // Clone partition to avoid borrow checker issues
        let partition = {
            let partition_table = self.partition_table()?;
            partition_table
                .partitions()
                .iter()
                .find(|p| p.number == partition_num)
                .cloned()
                .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partition_num)))?
        };

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;
        let fs = FileSystem::detect(reader, &partition)?;

        fs.label()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::NotFound("No label".to_string()))
    }

    /// Get filesystem UUID
    ///
    pub fn vfs_uuid(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        let partition_num = self.parse_device_name(device)?;

        // Clone partition to avoid borrow checker issues
        let partition = {
            let partition_table = self.partition_table()?;
            partition_table
                .partitions()
                .iter()
                .find(|p| p.number == partition_num)
                .cloned()
                .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partition_num)))?
        };

        let reader = self
            .reader
            .as_mut()
            .ok_or_else(|| Error::InvalidState("Reader not initialized".to_string()))?;
        let fs = FileSystem::detect(reader, &partition)?;

        fs.uuid()
            .map(|s| s.to_string())
            .ok_or_else(|| Error::NotFound("No UUID".to_string()))
    }

    /// Get block device size in bytes
    ///
    pub fn blockdev_getsize64(&self, device: &str) -> Result<i64> {
        self.ensure_ready()?;

        let partition_num = self.parse_device_name(device)?;

        if partition_num == 0 {
            // Whole device
            let reader = self
                .reader
                .as_ref()
                .ok_or_else(|| Error::InvalidState("Not launched".to_string()))?;
            Ok(i64::try_from(reader.size()).unwrap_or(i64::MAX))
        } else {
            // Partition - calculate from partition table
            let partition_table = self.partition_table()?;

            let partition = partition_table
                .partitions()
                .iter()
                .find(|p| p.number == partition_num)
                .ok_or_else(|| Error::NotFound(format!("Partition {} not found", partition_num)))?;

            // NOTE: Assumes 512-byte sectors. This is correct for most disks but may be
            // inaccurate for devices with 4096-byte physical sectors (4Kn drives).
            Ok(i64::try_from(partition.size_sectors.saturating_mul(512)).unwrap_or(i64::MAX))
        }
    }

    /// Get block device size in 512-byte sectors
    ///
    pub fn blockdev_getsz(&self, device: &str) -> Result<i64> {
        Ok(self.blockdev_getsize64(device)? / 512)
    }

    /// Get canonical device name
    ///
    pub fn canonical_device_name(&self, device: &str) -> Result<String> {
        // Normalize device names
        let device = device.trim_start_matches("/dev/");

        // Convert variations to canonical form
        if let Some(suffix) = device
            .strip_prefix("hd")
            .or_else(|| device.strip_prefix("vd"))
        {
            Ok(format!("/dev/sd{}", suffix))
        } else {
            Ok(format!("/dev/{}", device))
        }
    }

    /// Get device index
    ///
    pub fn device_index(&self, device: &str) -> Result<i32> {
        let canonical = self.canonical_device_name(device)?;

        // Extract the drive letter
        if let Some(rest) = canonical.strip_prefix("/dev/sd") {
            if let Some(letter) = rest.chars().next() {
                return Ok((letter as u8 - b'a') as i32);
            }
        }

        Err(Error::InvalidFormat(format!(
            "Cannot parse device: {}",
            device
        )))
    }

    /// Check if device name refers to whole device (not partition)
    ///
    pub fn is_whole_device(&self, device: &str) -> Result<bool> {
        // Whole devices end with just a letter (e.g., /dev/sda)
        // Partitions have numbers (e.g., /dev/sda1)
        let canonical = self.canonical_device_name(device)?;

        Ok(!canonical
            .chars()
            .last()
            .map(|c| c.is_numeric())
            .unwrap_or(false))
    }

    /// Resolve a guest-visible device/partition specifier (e.g. `/dev/sda1`,
    /// `/dev/mapper/vg-lv`) to the real host path that external tools like
    /// `blkid` can read directly (the NBD or loop device backing this drive).
    pub(crate) fn resolve_block_device_path(&self, device: &str) -> Result<PathBuf> {
        if device.starts_with("/dev/mapper/")
            || (device.starts_with("/dev/") && device.matches('/').count() >= 3)
            || device.starts_with("/dev/md")
            || device.starts_with("/dev/dm-")
        {
            // LVM logical volume, MD RAID array, or raw dm-N node - already a real device node
            return Ok(PathBuf::from(device));
        }

        let partition_num = self.parse_device_name(device)?;

        if let Some(loop_dev) = &self.loop_device {
            return if partition_num > 0 {
                loop_dev
                    .partition_path(partition_num)
                    .ok_or_else(|| Error::InvalidState("Loop device not connected".to_string()))
            } else {
                loop_dev
                    .device_path()
                    .ok_or_else(|| Error::InvalidState("Loop device not connected".to_string()))
                    .map(|p| p.to_path_buf())
            };
        }

        if let Some(nbd) = &self.nbd_device {
            return Ok(if partition_num > 0 {
                nbd.partition_path(partition_num)
            } else {
                nbd.device_path().to_path_buf()
            });
        }

        Err(Error::InvalidState(
            "No block device available (neither loop nor NBD)".to_string(),
        ))
    }

    /// Get all blkid tags for a device (e.g. TYPE, UUID, LABEL, PARTUUID).
    ///
    /// # Arguments
    ///
    /// * `device` - Guest-visible device or partition specifier
    ///
    /// # Returns
    ///
    /// blkid tag name/value pairs (empty if the device has no recognizable
    /// filesystem/signature).
    pub fn blkid(&mut self, device: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;
        let host_path = self.resolve_block_device_path(device)?;
        let tags = crate::guestfs::device_inventory::blkid_export(&host_path.to_string_lossy())?;
        Ok(tags.into_iter().collect())
    }

    /// Build the candidate device list (physical partitions plus any active
    /// LVM logical volumes) with their blkid tags, used by `findfs_uuid`,
    /// `findfs_label`, and `list_dm_devices`.
    pub(crate) fn blkid_candidates(&mut self) -> Result<Vec<(String, HashMap<String, String>)>> {
        let mut candidates = self.list_partitions()?;
        if let Ok(lvs) = self.lvs() {
            candidates.extend(lvs);
        }

        let mut out = Vec::with_capacity(candidates.len());
        for dev in candidates {
            let Ok(host_path) = self.resolve_block_device_path(&dev) else {
                continue;
            };
            let tags = crate::guestfs::device_inventory::blkid_export(&host_path.to_string_lossy())
                .unwrap_or_default();
            out.push((dev, tags));
        }
        Ok(out)
    }

    /// Find the device with the given filesystem UUID.
    ///
    /// Scans physical partitions and active LVM logical volumes.
    pub fn findfs_uuid(&mut self, uuid: &str) -> Result<String> {
        self.ensure_ready()?;
        let uuid = uuid.trim();
        for (dev, tags) in self.blkid_candidates()? {
            if tags
                .get("UUID")
                .is_some_and(|v| v.eq_ignore_ascii_case(uuid))
            {
                return Ok(dev);
            }
        }
        Err(Error::NotFound(format!("No device with UUID={}", uuid)))
    }

    /// Find the device with the given filesystem label.
    ///
    /// Scans physical partitions and active LVM logical volumes.
    pub fn findfs_label(&mut self, label: &str) -> Result<String> {
        self.ensure_ready()?;
        for (dev, tags) in self.blkid_candidates()? {
            if tags.get("LABEL").map(|v| v.as_str()) == Some(label) {
                return Ok(dev);
            }
        }
        Err(Error::NotFound(format!("No device with LABEL={}", label)))
    }

    /// Find the device with the given GPT/MBR partition UUID (PARTUUID).
    ///
    /// Scans physical partitions and active LVM logical volumes.
    pub fn findfs_partuuid(&mut self, partuuid: &str) -> Result<String> {
        self.ensure_ready()?;
        let partuuid = partuuid.trim();
        for (dev, tags) in self.blkid_candidates()? {
            if tags
                .get("PARTUUID")
                .is_some_and(|v| v.eq_ignore_ascii_case(partuuid))
            {
                return Ok(dev);
            }
        }
        Err(Error::NotFound(format!(
            "No device with PARTUUID={}",
            partuuid
        )))
    }

    /// Resolve an fstab-style mountable (`UUID=…`, `LABEL=…`, `PARTUUID=…`, or
    /// a raw `/dev/…` path) to a guest device path GuestKit can mount.
    ///
    /// Ubuntu cloud images (and many modern distros) put `/boot` and
    /// `/boot/efi` on separate partitions referenced only by LABEL/UUID in
    /// fstab. Without this resolution, `mount_ro("LABEL=BOOT", "/boot")`
    /// fails and doctor reports empty `/boot` (BOOT-003).
    pub fn resolve_mountable(&mut self, mountable: &str) -> Result<String> {
        let spec = mountable.trim();
        if let Some(uuid) = spec.strip_prefix("UUID=") {
            return self.findfs_uuid(unquote_fs_token(uuid));
        }
        if let Some(label) = spec.strip_prefix("LABEL=") {
            return self.findfs_label(unquote_fs_token(label));
        }
        if let Some(partuuid) = spec.strip_prefix("PARTUUID=") {
            return self.findfs_partuuid(unquote_fs_token(partuuid));
        }
        Ok(spec.to_string())
    }
}

/// Strip optional single/double quotes from an fstab UUID/LABEL/PARTUUID value.
pub(crate) fn unquote_fs_token(s: &str) -> &str {
    let s = s.trim();
    if s.len() >= 2
        && ((s.starts_with('"') && s.ends_with('"')) || (s.starts_with('\'') && s.ends_with('\'')))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canonical_device_name() {
        let g = Guestfs::new().unwrap();

        assert_eq!(g.canonical_device_name("/dev/sda").unwrap(), "/dev/sda");
        assert_eq!(g.canonical_device_name("/dev/hda").unwrap(), "/dev/sda");
        assert_eq!(g.canonical_device_name("/dev/vda").unwrap(), "/dev/sda");
        assert_eq!(g.canonical_device_name("sda").unwrap(), "/dev/sda");
    }

    #[test]
    fn unquote_fs_token_strips_quotes() {
        assert_eq!(unquote_fs_token("BOOT"), "BOOT");
        assert_eq!(unquote_fs_token("\"BOOT\""), "BOOT");
        assert_eq!(unquote_fs_token("'UEFI'"), "UEFI");
        assert_eq!(unquote_fs_token("  UUID  "), "UUID");
    }

    #[test]
    fn test_is_whole_device() {
        let g = Guestfs::new().unwrap();

        assert!(g.is_whole_device("/dev/sda").unwrap());
        assert!(!g.is_whole_device("/dev/sda1").unwrap());
        assert!(!g.is_whole_device("/dev/sda10").unwrap());
    }

    #[test]
    fn test_device_index() {
        let g = Guestfs::new().unwrap();

        assert_eq!(g.device_index("/dev/sda").unwrap(), 0);
        assert_eq!(g.device_index("/dev/sdb").unwrap(), 1);
        assert_eq!(g.device_index("/dev/vda").unwrap(), 0);
    }
}
