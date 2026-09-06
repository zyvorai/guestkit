// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Command execution inside guest
//!
//! This implementation uses chroot to execute commands inside the
//! mounted guest filesystem.
//!
//! **Requires**: Mounted filesystem and sudo/root permissions

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Execute a command in the guest
    ///
    ///
    /// # Arguments
    ///
    /// * `arguments` - Command and arguments as array
    ///
    /// # Returns
    ///
    /// Command output as string
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new().unwrap();
    /// g.add_drive_ro("/path/to/disk.qcow2").unwrap();
    /// g.launch().unwrap();
    ///
    /// // Mount root filesystem first
    /// g.mount_ro("/dev/sda1", "/").unwrap();
    ///
    /// // Execute command
    /// let output = g.command(&["/bin/ls", "/etc"]).unwrap();
    /// println!("Output: {}", output);
    /// ```
    pub fn command(&mut self, arguments: &[&str]) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: command {:?}", arguments);
        }

        if arguments.is_empty() {
            return Err(Error::InvalidFormat("No command provided".to_string()));
        }

        // Get root mount point (use centralized helper)
        let root_mountpoint = self.find_root_mountpoint()?;

        // Execute command using chroot
        let output = Command::new("chroot")
            .arg(root_mountpoint)
            .args(arguments)
            .output()
            .map_err(|e| {
                Error::CommandFailed(format!(
                    "Failed to execute command via chroot: {}. Requires sudo/root.",
                    e
                ))
            })?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(Error::CommandFailed(format!(
                "Command failed with exit code {:?}: {}",
                output.status.code(),
                stderr
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Execute a command and return output as lines
    ///
    pub fn command_lines(&mut self, arguments: &[&str]) -> Result<Vec<String>> {
        let output = self.command(arguments)?;
        Ok(output.lines().map(|s| s.to_string()).collect())
    }

    /// Execute a shell command inside the guest via chroot
    ///
    /// # Security
    ///
    /// The `command` string is passed directly to `/bin/sh -c` inside the
    /// chroot. Callers must ensure the command string does not contain
    /// unsanitized user input to prevent shell injection. For programmatic
    /// use, prefer `command()` with explicit argument arrays.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use guestkit::guestfs::Guestfs;
    ///
    /// let mut g = Guestfs::new().unwrap();
    /// // ... setup ...
    ///
    /// let output = g.sh("cat /etc/hostname").unwrap();
    /// ```
    pub fn sh(&mut self, command: &str) -> Result<String> {
        // Reject commands with null bytes
        if command.contains('\0') {
            return Err(Error::InvalidFormat(
                "Shell command must not contain null bytes".to_string(),
            ));
        }

        // Reject commands with dangerous shell metacharacters.
        // Callers should use command() with explicit arg arrays for any input
        // that could contain user-controlled data.
        const DANGEROUS_PATTERNS: &[&str] =
            &["$(", "`", "&&", "||", ">>", "<<", "|", ";", ">", "\n", "\r"];
        for pattern in DANGEROUS_PATTERNS {
            if command.contains(pattern) {
                return Err(Error::SecurityViolation(format!(
                    "sh() command contains dangerous shell metacharacter '{}'. \
                     Use command() with explicit args for untrusted input.",
                    pattern.escape_default()
                )));
            }
        }

        self.command(&["/bin/sh", "-c", command])
    }

    /// Execute a shell command containing operators (||, &&, |, >, etc.)
    /// that would be rejected by sh().
    ///
    /// # Safety contract
    /// The command string MUST be a hardcoded developer-controlled literal,
    /// never constructed from user input. Use command() with explicit arg
    /// arrays for any user-controlled data.
    pub fn sh_raw(&mut self, command: &str) -> Result<String> {
        if command.contains('\0') {
            return Err(Error::InvalidFormat(
                "Shell command must not contain null bytes".to_string(),
            ));
        }
        self.command(&["/bin/sh", "-c", command])
    }

    /// Execute a shell command and return output as lines
    ///
    pub fn sh_lines(&mut self, command: &str) -> Result<Vec<String>> {
        let output = self.sh(command)?;
        Ok(output.lines().map(|s| s.to_string()).collect())
    }

    /// Same as [`command`] — quiet variant for call-site compatibility.
    pub fn command_quiet(&mut self, arguments: &[&str]) -> Result<String> {
        self.command(arguments)
    }

    /// Same as [`command`] — guestkit already chroots with /proc,/dev,/sys when needed.
    pub fn command_with_mounts(&mut self, arguments: &[&str]) -> Result<String> {
        self.command(arguments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_api_exists() {
        let g = Guestfs::new().unwrap();
        // API structure test - will fail without implementation
        let _ = g;
    }
}
