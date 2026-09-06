// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! NTFS filesystem operations for disk image manipulation
//!
//! This implementation provides NTFS-specific functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Clone NTFS filesystem
    ///
    pub fn ntfsclone_out(
        &mut self,
        device: &str,
        backupfile: &str,
        metadataonly: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: ntfsclone_out {} {} {}",
                device, backupfile, metadataonly
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let mut cmd = Command::new("ntfsclone");
        cmd.arg("--save-image");
        cmd.arg("--output").arg(backupfile);

        if metadataonly {
            cmd.arg("--metadata");
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ntfsclone: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ntfsclone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Restore NTFS from clone
    ///
    pub fn ntfsclone_in(&mut self, backupfile: &str, device: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ntfsclone_in {} {}", backupfile, device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("ntfsclone")
            .arg("--restore-image")
            .arg("--overwrite")
            .arg(&nbd_partition)
            .arg(backupfile)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ntfsclone: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ntfsclone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Fix NTFS filesystem
    ///
    pub fn ntfsfix(&mut self, device: &str, clearbadsectors: bool) -> Result<()> {
        self.ntfsfix_opts(device, clearbadsectors, false)
    }

    /// Fix NTFS filesystem, optionally clearing the volume dirty flag.
    ///
    /// `clear_dirty` maps to ntfsfix's `-d`/`--clear-dirty`. Disks left by a
    /// hypervisor-level power-off or Windows Fast Startup carry a dirty NTFS
    /// flag ("Metadata kept in Windows cache") that a plain `ntfsfix` does
    /// *not* clear — ntfs-3g then silently mounts read-only (or refuses
    /// outright) and any write-mode rescue/inject operation fails. Callers
    /// that are about to mount a Windows disk read-write for offline
    /// recovery should pass `true`.
    pub fn ntfsfix_opts(
        &mut self,
        device: &str,
        clearbadsectors: bool,
        clear_dirty: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: ntfsfix {} {} clear_dirty={}",
                device, clearbadsectors, clear_dirty
            );
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let mut cmd = Command::new("ntfsfix");

        if clearbadsectors {
            cmd.arg("--clear-bad-sectors");
        }
        if clear_dirty {
            cmd.arg("--clear-dirty");
        }

        cmd.arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ntfsfix: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ntfsfix failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get NTFS volume information
    ///
    pub fn ntfs_3g_probe(&mut self, rw: bool, device: &str) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ntfs_3g_probe {} {}", rw, device);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let mode = if rw { "--readwrite" } else { "--readonly" };

        let output = Command::new("ntfs-3g.probe")
            .arg(mode)
            .arg(&nbd_partition)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ntfs-3g.probe: {}", e)))?;

        Ok(output.status.code().unwrap_or(1))
    }

    /// Set NTFS master boot record
    ///
    pub fn ntfs_set_label(&mut self, device: &str, label: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: ntfs_set_label {} {}", device, label);
        }

        self.setup_nbd_if_needed()?;

        let nbd_partition =
            if let Some(partition_number) = device.chars().last().and_then(|c| c.to_digit(10)) {
                let nbd_device = self
                    .nbd_device
                    .as_ref()
                    .ok_or_else(|| Error::InvalidState("NBD device not available".to_string()))?;
                format!(
                    "{}p{}",
                    nbd_device.device_path().display(),
                    partition_number
                )
            } else {
                return Err(Error::InvalidFormat(format!("Invalid device: {}", device)));
            };

        let output = Command::new("ntfslabel")
            .arg(&nbd_partition)
            .arg(label)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute ntfslabel: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "ntfslabel failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ntfs_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
