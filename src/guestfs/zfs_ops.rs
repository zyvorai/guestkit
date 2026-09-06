// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! ZFS operations for disk image manipulation
//!
//! This implementation provides ZFS filesystem management functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create ZFS filesystem
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_create(&mut self, name: &str, mountpoint: Option<&str>) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_create {} {:?}", name, mountpoint);
        }

        let mut cmd = Command::new("zfs");
        cmd.arg("create");

        if let Some(mp) = mountpoint {
            cmd.arg("-o").arg(format!("mountpoint={}", mp));
        }

        cmd.arg(name);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs create failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Destroy ZFS filesystem
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_destroy(&mut self, name: &str, recursive: bool) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_destroy {} {}", name, recursive);
        }

        let mut cmd = Command::new("zfs");
        cmd.arg("destroy");

        if recursive {
            cmd.arg("-r");
        }

        cmd.arg(name);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs destroy failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// List ZFS filesystems
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_list(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_list");
        }

        let output = Command::new("zfs")
            .arg("list")
            .arg("-H")
            .arg("-o")
            .arg("name")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let filesystems: Vec<String> = output_str.lines().map(|s| s.trim().to_string()).collect();

        Ok(filesystems)
    }

    /// Get ZFS properties
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_get(&mut self, name: &str, property: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_get {} {}", name, property);
        }

        let output = Command::new("zfs")
            .arg("get")
            .arg("-H")
            .arg("-o")
            .arg("value")
            .arg(property)
            .arg(name)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs get failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }

    /// Set ZFS properties
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_set(&mut self, name: &str, property: &str, value: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_set {} {}={}", name, property, value);
        }

        let output = Command::new("zfs")
            .arg("set")
            .arg(format!("{}={}", property, value))
            .arg(name)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs set failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create ZFS snapshot
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_snapshot(&mut self, name: &str, snapname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_snapshot {}@{}", name, snapname);
        }

        let output = Command::new("zfs")
            .arg("snapshot")
            .arg(format!("{}@{}", name, snapname))
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs snapshot failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Clone ZFS snapshot
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_clone(&mut self, snapshot: &str, name: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_clone {} {}", snapshot, name);
        }

        let output = Command::new("zfs")
            .arg("clone")
            .arg(snapshot)
            .arg(name)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs clone failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Rollback ZFS to snapshot
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_rollback(&mut self, snapshot: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_rollback {}", snapshot);
        }

        let output = Command::new("zfs")
            .arg("rollback")
            .arg(snapshot)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs rollback failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Send ZFS stream
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_send(&mut self, snapshot: &str, filename: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_send {} {}", snapshot, filename);
        }

        let output = Command::new("zfs")
            .arg("send")
            .arg(snapshot)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs send failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        std::fs::write(filename, output.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// Receive ZFS stream
    ///
    /// Additional functionality for ZFS support
    pub fn zfs_receive(&mut self, name: &str, filename: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: zfs_receive {} {}", name, filename);
        }

        let stream = std::fs::read(filename).map_err(Error::Io)?;

        let mut cmd = Command::new("zfs");
        cmd.arg("receive")
            .arg(name)
            .stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute zfs: {}", e)))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&stream).map_err(Error::Io)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for zfs: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "zfs receive failed: {}",
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
    fn test_zfs_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
