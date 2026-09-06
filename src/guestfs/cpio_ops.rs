// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CPIO archive operations for disk image manipulation
//!
//! This implementation provides CPIO archive creation and extraction.

use crate::core::{Error, Result};
use crate::guestfs::security_utils::PathValidator;
use crate::guestfs::Guestfs;
use std::process::{Command, Stdio};

impl Guestfs {
    /// Extract CPIO archive to directory
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_extract(&mut self, archive: &str, directory: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_extract {} {}", archive, directory);
        }

        let host_dir = self.resolve_guest_path(directory)?;

        let archive_data = std::fs::read(archive).map_err(Error::Io)?;

        let mut cmd = Command::new("cpio");
        cmd.arg("-idm")
            .arg("-D")
            .arg(&host_dir)
            .stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&archive_data).map_err(Error::Io)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "cpio extraction failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Create CPIO archive from directory
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_create(&mut self, directory: &str, archive: &str, format: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_create {} {} {}", directory, archive, format);
        }

        // Validate inputs to prevent command injection
        PathValidator::validate_fs_path(directory)?;
        PathValidator::validate_fs_path(archive)?;
        PathValidator::validate_cpio_format(format)?;

        let host_dir = self.resolve_guest_path(directory)?;

        // Use piped commands instead of shell to prevent injection
        // Start find process
        let mut find_child = Command::new("find")
            .arg(".")
            .current_dir(&host_dir)
            .arg("-print")
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute find: {}", e)))?;

        let find_stdout = find_child
            .stdout
            .take()
            .ok_or_else(|| Error::CommandFailed("Failed to capture find output".to_string()))?;

        // Start cpio process with find output as stdin
        let cpio_child = Command::new("cpio")
            .arg("-o")
            .arg("-H")
            .arg(format)
            .stdin(find_stdout)
            .stdout(Stdio::piped())
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

        // Get cpio output
        let cpio_output = cpio_child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        // Wait for find child to prevent process leak
        let _ = find_child.wait();

        if !cpio_output.status.success() {
            return Err(Error::CommandFailed(format!(
                "cpio creation failed: {}",
                String::from_utf8_lossy(&cpio_output.stderr)
            )));
        }

        // Write cpio output to archive file
        std::fs::write(archive, &cpio_output.stdout).map_err(Error::Io)?;

        Ok(())
    }

    /// List contents of CPIO archive
    ///
    /// Additional functionality for CPIO support
    pub fn cpio_list(&mut self, archive: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: cpio_list {}", archive);
        }

        let archive_data = std::fs::read(archive).map_err(Error::Io)?;

        let mut cmd = Command::new("cpio");
        cmd.arg("-t").stdin(std::process::Stdio::piped());

        let mut child = cmd
            .spawn()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute cpio: {}", e)))?;

        use std::io::Write;
        if let Some(mut stdin) = child.stdin.take() {
            stdin.write_all(&archive_data).map_err(Error::Io)?;
        }

        let output = child
            .wait_with_output()
            .map_err(|e| Error::CommandFailed(format!("Failed to wait for cpio: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "cpio list failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let output_str = String::from_utf8_lossy(&output.stdout);
        let files: Vec<String> = output_str.lines().map(|s| s.to_string()).collect();

        Ok(files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cpio_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
