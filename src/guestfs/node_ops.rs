// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Node creation operations for disk image manipulation
//!
//! This implementation provides special file and device node creation.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Create device node
    ///
    pub fn mknod(&mut self, mode: i32, devmajor: i32, devminor: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mknod {} {} {} {}", mode, devmajor, devminor, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let file_type = match (mode >> 12) & 0xF {
            0x6 => "b", // Block device
            0x2 => "c", // Character device
            0x1 => "p", // FIFO
            _ => return Err(Error::InvalidFormat("Invalid mode for mknod".to_string())),
        };

        let mut cmd = Command::new("mknod");
        cmd.arg(&host_path).arg(file_type);

        if file_type != "p" {
            cmd.arg(devmajor.to_string()).arg(devminor.to_string());
        }

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mknod: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mknod failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        // Set mode
        self.chmod(mode & 0o7777, path)?;

        Ok(())
    }

    /// Create block device node
    ///
    pub fn mknod_b(&mut self, mode: i32, devmajor: i32, devminor: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: mknod_b {} {} {} {}",
                mode, devmajor, devminor, path
            );
        }

        // Create block device (mode 0x6000)
        let block_mode = 0o060000 | (mode & 0o7777);
        self.mknod(block_mode, devmajor, devminor, path)
    }

    /// Create character device node
    ///
    pub fn mknod_c(&mut self, mode: i32, devmajor: i32, devminor: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: mknod_c {} {} {} {}",
                mode, devmajor, devminor, path
            );
        }

        // Create character device (mode 0x2000)
        let char_mode = 0o020000 | (mode & 0o7777);
        self.mknod(char_mode, devmajor, devminor, path)
    }

    /// Create FIFO (named pipe)
    ///
    pub fn mkfifo(&mut self, mode: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkfifo {} {}", mode, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let output = Command::new("mkfifo")
            .arg("-m")
            .arg(format!("{:o}", mode & 0o7777))
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mkfifo: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mkfifo failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create temporary directory
    ///
    pub fn mkdtemp(&mut self, tmpl: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mkdtemp {}", tmpl);
        }

        let host_tmpl = self.resolve_guest_path(tmpl)?;

        let output = Command::new("mktemp")
            .arg("-d")
            .arg(&host_tmpl)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mktemp: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mktemp failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let temp_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(temp_dir)
    }

    /// Create temporary file
    ///
    pub fn mktemp(&mut self, tmpl: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: mktemp {}", tmpl);
        }

        let host_tmpl = self.resolve_guest_path(tmpl)?;

        let output = Command::new("mktemp")
            .arg(&host_tmpl)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute mktemp: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "mktemp failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let temp_file = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(temp_file)
    }

    /// Truncate file to zero size
    ///
    pub fn truncate(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: truncate {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        std::fs::OpenOptions::new()
            .write(true)
            .truncate(true)
            .open(&host_path)
            .map_err(Error::Io)?;

        Ok(())
    }

    /// Truncate file to specific size
    ///
    pub fn truncate_size(&mut self, path: &str, size: i64) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: truncate_size {} {}", path, size);
        }

        if size < 0 {
            return Err(Error::InvalidOperation(
                "truncate size must be non-negative".to_string(),
            ));
        }

        let host_path = self.resolve_guest_path(path)?;

        let file = std::fs::OpenOptions::new()
            .write(true)
            .open(&host_path)
            .map_err(Error::Io)?;

        file.set_len(size as u64).map_err(Error::Io)?;

        Ok(())
    }

    /// Change file timestamps
    ///
    pub fn utimens(
        &mut self,
        path: &str,
        atsecs: i64,
        atnsecs: i64,
        mtsecs: i64,
        mtnsecs: i64,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: utimens {} {} {} {} {}",
                path, atsecs, atnsecs, mtsecs, mtnsecs
            );
        }

        let host_path = self.resolve_guest_path(path)?;

        // Use touch command with specific timestamp
        let atime_str = format!("{}.{:09}", atsecs, atnsecs);
        let _mtime_str = format!("{}.{:09}", mtsecs, mtnsecs);

        let output = Command::new("touch")
            .arg("-a")
            .arg("-t")
            .arg(&atime_str)
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute touch: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "touch failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Synchronize file data to disk
    ///
    pub fn fsync(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: fsync {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let file = std::fs::File::open(&host_path).map_err(Error::Io)?;

        file.sync_all().map_err(Error::Io)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_node_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
