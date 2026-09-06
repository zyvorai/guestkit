// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Block device operations for disk image manipulation
//!
//! This implementation provides block device manipulation functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Set block device to read-only
    ///
    pub fn blockdev_setro(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_setro {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--setro")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --setro failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Set block device to read-write
    ///
    pub fn blockdev_setrw(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_setrw {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--setrw")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --setrw failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get read-only status of block device
    ///
    pub fn blockdev_getro(&mut self, device: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_getro {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--getro")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --getro failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        Ok(output_str.trim() == "1")
    }

    /// Flush block device buffers
    ///
    pub fn blockdev_flushbufs(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_flushbufs {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--flushbufs")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --flushbufs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Reread partition table
    ///
    pub fn blockdev_rereadpt(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_rereadpt {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--rereadpt")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --rereadpt failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get block size of device
    ///
    pub fn blockdev_getbsz(&mut self, device: &str) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_getbsz {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--getbsz")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --getbsz failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str
            .trim()
            .parse::<i32>()
            .map_err(|e| Error::InvalidFormat(format!("Failed to parse block size: {}", e)))
    }

    /// Set block size of device
    ///
    pub fn blockdev_setbsz(&mut self, device: &str, blocksize: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_setbsz {} {}", device, blocksize);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--setbsz")
            .arg(blocksize.to_string())
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --setbsz failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get sector count of device
    ///
    pub fn blockdev_getsectors(&mut self, device: &str) -> Result<i64> {
        self.blockdev_getsz(device)
    }

    /// Get sector size of device
    ///
    pub fn blockdev_getss(&mut self, device: &str) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: blockdev_getss {}", device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_device_path = self
            .nbd_device
            .as_ref()
            .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?
            .device_path();

        let output = Command::new("blockdev")
            .arg("--getss")
            .arg(nbd_device_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute blockdev: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "blockdev --getss failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        output_str
            .trim()
            .parse::<i32>()
            .map_err(|e| Error::InvalidFormat(format!("Failed to parse sector size: {}", e)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blockdev_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
