// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Syslinux operations for disk image manipulation
//!
//! This implementation provides syslinux bootloader installation functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Install syslinux bootloader
    ///
    pub fn syslinux(&mut self, device: &str, directory: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: syslinux {} {:?}", device, directory);
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

        let mut cmd = Command::new("syslinux");

        if let Some(dir) = directory {
            cmd.arg("--directory").arg(dir);
        }

        cmd.arg("--install").arg(&nbd_partition);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute syslinux: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "syslinux failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Install extlinux bootloader
    ///
    pub fn extlinux(&mut self, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: extlinux {}", directory);
        }

        let host_path = self.resolve_guest_path(directory)?;

        let output = Command::new("extlinux")
            .arg("--install")
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute extlinux: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "extlinux failed: {}",
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
    fn test_syslinux_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
