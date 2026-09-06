// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! LUKS (Linux Unified Key Setup) encryption operations
//!
//! This implementation uses cryptsetup command-line tool.
//!
//! **Requires**: cryptsetup and sudo/root permissions

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Open a LUKS encrypted device
    ///
    ///
    /// # Arguments
    ///
    /// * `device` - Encrypted device (e.g., "/dev/sda1")
    /// * `key` - Passphrase/key for decryption
    /// * `mapname` - Name for the mapped device
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut g = Guestfs::new()?;
    /// g.add_drive_ro("/path/to/encrypted.qcow2")?;
    /// g.launch()?;
    ///
    /// // Open LUKS device
    /// g.luks_open("/dev/sda1", "mypassword", "cryptroot")?;
    ///
    /// // Now you can mount /dev/mapper/cryptroot
    /// g.mount_ro("/dev/mapper/cryptroot", "/")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn luks_open(&mut self, device: &str, key: &str, mapname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: luks_open {} [key hidden] {}", device, mapname);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Get NBD partition device path
        let partition_num = self.parse_device_name(device)?;
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Open LUKS device using cryptsetup
        // We need to pass the key via stdin for security
        let mut child = Command::new("cryptsetup")
            .arg("open")
            .arg(&nbd_partition)
            .arg(mapname)
            .arg("--type")
            .arg("luks")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                Error::CommandFailed(format!(
                    "Failed to run cryptsetup: {}. Is cryptsetup installed? Requires sudo/root.",
                    e
                ))
            })?;

        // Write key to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(key.as_bytes())
                .map_err(|e| Error::CommandFailed(format!("Failed to write key: {}", e)))?;
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS open failed: {}. Check passphrase and device.",
                stderr
            )));
        }

        if self.verbose {
            eprintln!("guestfs: LUKS device opened as /dev/mapper/{}", mapname);
        }

        Ok(())
    }

    /// Open a LUKS encrypted device (read-only)
    ///
    pub fn luks_open_ro(&mut self, device: &str, key: &str, mapname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: luks_open_ro {} [key hidden] {}", device, mapname);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Get NBD partition device path
        let partition_num = self.parse_device_name(device)?;
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Open LUKS device read-only
        let mut child = Command::new("cryptsetup")
            .arg("open")
            .arg(&nbd_partition)
            .arg(mapname)
            .arg("--type")
            .arg("luks")
            .arg("--readonly")
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to run cryptsetup: {}", e)))?;

        // Write key to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(key.as_bytes())
                .map_err(|e| Error::CommandFailed(format!("Failed to write key: {}", e)))?;
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS open failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Close a LUKS encrypted device
    ///
    ///
    /// # Arguments
    ///
    /// * `device` - Mapped device name (e.g., "cryptroot")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// # fn main() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut g = Guestfs::new()?;
    /// // ... setup and luks_open ...
    ///
    /// // Close LUKS device
    /// g.luks_close("cryptroot")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn luks_close(&mut self, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: luks_close {}", device);
        }

        // Close LUKS device
        let output = Command::new("cryptsetup")
            .arg("close")
            .arg(device)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS close failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Format a device as LUKS
    ///
    ///
    /// **WARNING**: This will destroy all data on the device!
    ///
    /// # Arguments
    ///
    /// * `device` - Device to format (e.g., "/dev/sda1")
    /// * `key` - Passphrase for encryption
    /// * `keyslot` - Key slot number (0-7)
    pub fn luks_format(&mut self, device: &str, key: &str, keyslot: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: luks_format {} [key hidden] slot {}",
                device, keyslot
            );
        }

        // Check if drive is readonly
        if let Some(drive) = self.drives.first() {
            if drive.readonly {
                return Err(Error::PermissionDenied(
                    "Cannot format LUKS on read-only drive".to_string(),
                ));
            }
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Get NBD partition device path
        let partition_num = self.parse_device_name(device)?;
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Format as LUKS
        let mut child = Command::new("cryptsetup")
            .arg("luksFormat")
            .arg(&nbd_partition)
            .arg("--key-slot")
            .arg(keyslot.to_string())
            .arg("--batch-mode") // Don't ask for confirmation
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to run cryptsetup: {}", e)))?;

        // Write key to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            stdin
                .write_all(key.as_bytes())
                .map_err(|e| Error::CommandFailed(format!("Failed to write key: {}", e)))?;
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS format failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Add a key to a LUKS device
    ///
    ///
    /// # Arguments
    ///
    /// * `device` - LUKS device (e.g., "/dev/sda1")
    /// * `key` - Existing passphrase
    /// * `newkey` - New passphrase to add
    /// * `keyslot` - Key slot for new key (0-7)
    pub fn luks_add_key(
        &mut self,
        device: &str,
        key: &str,
        newkey: &str,
        keyslot: i32,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: luks_add_key {} [keys hidden] slot {}",
                device, keyslot
            );
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Get NBD partition device path
        let partition_num = self.parse_device_name(device)?;
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Add key to LUKS device
        let mut child = Command::new("cryptsetup")
            .arg("luksAddKey")
            .arg(&nbd_partition)
            .arg("--key-slot")
            .arg(keyslot.to_string())
            .stdin(std::process::Stdio::piped())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to run cryptsetup: {}", e)))?;

        // Write existing key and new key to stdin
        if let Some(mut stdin) = child.stdin.take() {
            use std::io::Write;
            // First the existing key, then the new key
            stdin
                .write_all(key.as_bytes())
                .map_err(|e| Error::CommandFailed(format!("Failed to write key: {}", e)))?;
            stdin
                .write_all(b"\n")
                .map_err(|e| Error::CommandFailed(format!("Failed to write newline: {}", e)))?;
            stdin
                .write_all(newkey.as_bytes())
                .map_err(|e| Error::CommandFailed(format!("Failed to write new key: {}", e)))?;
        }

        // Wait for command to complete
        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS add key failed: {}",
                stderr
            )));
        }

        Ok(())
    }

    /// Get UUID of LUKS device
    ///
    ///
    /// # Arguments
    ///
    /// * `device` - LUKS device (e.g., "/dev/sda1")
    ///
    /// # Returns
    ///
    /// UUID string of the LUKS device
    pub fn luks_uuid(&mut self, device: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: luks_uuid {}", device);
        }

        // Ensure NBD device is set up
        self.setup_nbd_if_needed()?;

        // Get NBD partition device path
        let partition_num = self.parse_device_name(device)?;
        let nbd = self.nbd_device()?;
        let nbd_partition = if partition_num > 0 {
            nbd.partition_path(partition_num)
        } else {
            nbd.device_path().to_path_buf()
        };

        // Get LUKS UUID
        let output = Command::new("cryptsetup")
            .arg("luksUUID")
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to run cryptsetup: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "LUKS UUID failed: {}",
                stderr
            )));
        }

        let uuid = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(uuid)
    }

    /// List devices (partitions and active LVM logical volumes) carrying a
    /// LUKS signature, suitable for passing to `luks_open`.
    ///
    /// Despite the name (kept for compatibility with callers migrating off
    /// libguestfs-style backends), this returns *candidate encrypted block
    /// devices to unlock*, not already-active `/dev/mapper/*` entries.
    pub fn list_dm_devices(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        let mut luks_devices = Vec::new();
        for (dev, tags) in self.blkid_candidates()? {
            if tags.get("TYPE").map(|v| v.as_str()) == Some("crypto_LUKS") {
                luks_devices.push(dev);
            }
        }
        Ok(luks_devices)
    }

    /// Alias of [`Self::luks_open`] (libguestfs `cryptsetup_open` name).
    pub fn cryptsetup_open(&mut self, device: &str, key: &str, mapname: &str) -> Result<()> {
        self.luks_open(device, key, mapname)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_luks_api_exists() {
        let g = Guestfs::new().unwrap();
        // API structure test
        let _ = g;
    }
}
