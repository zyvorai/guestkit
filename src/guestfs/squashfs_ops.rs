// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SquashFS operations for disk image manipulation
//!
//! This implementation provides SquashFS filesystem creation and manipulation.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create SquashFS filesystem
    ///
    pub fn mksquashfs(
        &mut self,
        path: &str,
        filename: &str,
        compress: Option<&str>,
        excludes: &[&str],
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mksquashfs {} {}", path, filename);
        }

        let host_path = self.resolve_guest_path(path)?;

        let mut cmd = Command::new("mksquashfs");
        cmd.arg(&host_path).arg(filename);

        if let Some(comp) = compress {
            cmd.arg("-comp").arg(comp);
        }

        for exclude in excludes {
            cmd.arg("-e").arg(exclude);
        }

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mksquashfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mksquashfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Extract SquashFS filesystem
    ///
    /// Additional functionality using unsquashfs
    pub fn unsquashfs(&mut self, filename: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: unsquashfs {} {}", filename, dest);
        }

        let host_dest = self.resolve_guest_path(dest)?;

        let output = Command::new("unsquashfs")
            .arg("-d")
            .arg(&host_dest)
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute unsquashfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "unsquashfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Get SquashFS info
    ///
    /// Additional functionality using unsquashfs
    pub fn squashfs_info(&mut self, filename: &str) -> Result<Vec<(String, String)>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: squashfs_info {}", filename);
        }

        let output = Command::new("unsquashfs")
            .arg("-s")
            .arg(filename)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute unsquashfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "unsquashfs failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let mut info = Vec::new();

        for line in output_str.lines() {
            if let Some((key, value)) = line.split_once(':') {
                info.push((key.trim().to_string(), value.trim().to_string()));
            }
        }

        Ok(info)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_squashfs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
