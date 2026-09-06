// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Partition management operations for disk image manipulation
//!
//! This implementation provides partition creation, deletion, and modification.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create a partition on a device
    ///
    pub fn part_add(
        &mut self,
        device: &str,
        prlogex: &str,
        startsect: i64,
        endsect: i64,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: part_add {} {} {} {}",
                device, prlogex, startsect, endsect
            );
        }

        self.setup_nbd_if_needed()?;

        // Get NBD device
        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        // Use parted to add partition
        let part_type = match prlogex {
            "primary" | "p" => "primary",
            "logical" | "l" => "logical",
            "extended" | "e" => "extended",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid partition type: {}",
                    prlogex
                )))
            }
        };

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("mkpart")
            .arg(part_type)
            .arg(format!("{}s", startsect))
            .arg(format!("{}s", endsect))
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "parted failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Delete a partition
    ///
    pub fn part_del(&mut self, device: &str, partnum: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_del {} {}", device, partnum);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("rm")
            .arg(partnum.to_string())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "parted failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Initialize partition table
    ///
    pub fn part_init(&mut self, device: &str, parttype: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_init {} {}", device, parttype);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let label = match parttype {
            "gpt" => "gpt",
            "msdos" | "mbr" => "msdos",
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid partition table type: {}",
                    parttype
                )))
            }
        };

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("mklabel")
            .arg(label)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "parted failed on device {}: stdout='{}', stderr='{}', exit_code={:?}",
                nbd_device.device_path().display(),
                stdout,
                stderr,
                output.status.code()
            )));
        }

        Ok(())
    }

    /// Resize a partition
    ///
    pub fn part_resize(&mut self, device: &str, partnum: i32, endsect: i64) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_resize {} {} {}", device, partnum, endsect);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("resizepart")
            .arg(partnum.to_string())
            .arg(format!("{}s", endsect))
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "parted failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set partition bootable flag
    ///
    pub fn part_set_bootable(&mut self, device: &str, partnum: i32, bootable: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: part_set_bootable {} {} {}",
                device, partnum, bootable
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let flag_val = if bootable { "on" } else { "off" };

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("set")
            .arg(partnum.to_string())
            .arg("boot")
            .arg(flag_val)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "parted failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set partition name (GPT only)
    ///
    pub fn part_set_name(&mut self, device: &str, partnum: i32, name: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_set_name {} {} {}", device, partnum, name);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("parted")
            .arg("-s")
            .arg(nbd_device.device_path())
            .arg("name")
            .arg(partnum.to_string())
            .arg(name)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute parted: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "parted failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set MBR partition type
    ///
    pub fn part_set_mbr_id(&mut self, device: &str, partnum: i32, idbyte: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: part_set_mbr_id {} {} {:x}",
                device, partnum, idbyte
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("sfdisk")
            .arg("--part-type")
            .arg(nbd_device.device_path())
            .arg(partnum.to_string())
            .arg(format!("{:x}", idbyte))
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sfdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sfdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get partition disk geometry
    ///
    pub fn part_get_disk_guid(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_get_disk_guid {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("sgdisk")
            .arg("--print")
            .arg(nbd_device.device_path())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output for disk GUID
        for line in stdout.lines() {
            if line.contains("Disk identifier (GUID):") {
                if let Some(guid) = line.split(':').nth(1) {
                    return Ok(guid.trim().to_string());
                }
            }
        }

        Err(Error::NotFound("Disk GUID not found".to_string()))
    }

    /// Get partition GUID (GPT only)
    ///
    pub fn part_get_gpt_guid(&mut self, device: &str, partnum: i32) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: part_get_gpt_guid {} {}", device, partnum);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;

        let output = Command::new("sgdisk")
            .arg("--info")
            .arg(partnum.to_string())
            .arg(nbd_device.device_path())
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute sgdisk: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "sgdisk failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);

        // Parse output for partition unique GUID
        for line in stdout.lines() {
            if line.contains("Partition unique GUID:") {
                if let Some(guid) = line.split(':').nth(1) {
                    return Ok(guid.trim().to_string());
                }
            }
        }

        Err(Error::NotFound("Partition GUID not found".to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_partition_mgmt_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
