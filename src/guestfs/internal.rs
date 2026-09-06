// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Internal operations for disk image manipulation
//!
//! This implementation provides internal/debug functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::collections::HashMap;

impl Guestfs {
    /// Get internal state for debugging
    ///
    pub fn internal_test(
        &mut self,
        str: &str,
        optstr: Option<&str>,
        strlist: &[&str],
        b: bool,
        integer: i32,
        integer64: i64,
    ) -> Result<String> {
        if self.verbose {
            eprintln!(
                "guestfs: internal_test {} {:?} {:?} {} {} {}",
                str, optstr, strlist, b, integer, integer64
            );
        }

        // Return a formatted debug string
        Ok(format!(
            "str={}, optstr={:?}, strlist={:?}, bool={}, int={}, int64={}",
            str, optstr, strlist, b, integer, integer64
        ))
    }

    /// Test only parameters
    ///
    pub fn internal_test_only_optargs(&mut self, test: i32) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: internal_test_only_optargs {}", test);
        }

        Ok(())
    }

    /// Get free disk space
    ///
    pub fn statvfs(&mut self, path: &str) -> Result<HashMap<String, i64>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: statvfs {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        // Use df command to get filesystem stats
        use std::process::Command;

        let output = Command::new("df")
            .arg("-B1") // 1-byte blocks for precision
            .arg(&host_path)
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute df: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "df failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        let stdout = String::from_utf8_lossy(&output.stdout);
        let mut info = HashMap::new();

        // Parse df output (simplified)
        if let Some(line) = stdout.lines().nth(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                info.insert("blocks".to_string(), parts[1].parse().unwrap_or(0));
                info.insert("bfree".to_string(), parts[3].parse().unwrap_or(0));
                info.insert("bavail".to_string(), parts[3].parse().unwrap_or(0));
            }
        }

        // "blocks"/"bfree"/"bavail" above are already byte counts (from
        // `df -B1`), so the block size callers multiply by must be 1, not
        // a real filesystem block size - the values above are NOT actual
        // 4096-byte-block counts.
        info.insert("bsize".to_string(), 1);
        info.insert("frsize".to_string(), 1);
        info.insert("files".to_string(), 0);
        info.insert("ffree".to_string(), 0);
        info.insert("favail".to_string(), 0);

        Ok(info)
    }

    /// Get max number of disks
    ///
    pub fn max_disks(&self) -> Result<i32> {
        // Return a reasonable maximum
        Ok(255)
    }

    /// Get number of devices
    ///
    pub fn nr_devices(&self) -> Result<i32> {
        Ok(self.drives.len() as i32)
    }

    /// Canonical device name
    ///
    pub fn device_name(&self, index: i32) -> Result<String> {
        if index < 0 || index >= self.drives.len() as i32 {
            return Err(Error::InvalidFormat("Invalid device index".to_string()));
        }

        // Return /dev/sdX style name
        let letter = (b'a' + index as u8) as char;
        Ok(format!("/dev/sd{}", letter))
    }

    /// Parse environment variable
    ///
    pub fn parse_environment(&mut self) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: parse_environment");
        }

        // Check for GUESTKIT_* environment variables
        if let Ok(debug) = std::env::var("GUESTKIT_DEBUG") {
            if debug == "1" {
                self.verbose = true;
            }
        }

        if let Ok(trace) = std::env::var("GUESTKIT_TRACE") {
            if trace == "1" {
                self.trace = true;
            }
        }

        Ok(())
    }

    /// Parse environment list
    ///
    pub fn parse_environment_list(&mut self, environment: &[&str]) -> Result<()> {
        if self.verbose {
            eprintln!("guestfs: parse_environment_list {:?}", environment);
        }

        for env in environment {
            if let Some((key, value)) = env.split_once('=') {
                match key {
                    "GUESTKIT_DEBUG" if value == "1" => self.verbose = true,
                    "GUESTKIT_TRACE" if value == "1" => self.trace = true,
                    _ => {}
                }
            }
        }

        Ok(())
    }

    /// Get state
    ///
    pub fn get_state(&self) -> Result<i32> {
        use crate::guestfs::handle::GuestfsState;

        match self.state {
            GuestfsState::Config => Ok(0),
            GuestfsState::Launching => Ok(1),
            GuestfsState::Ready => Ok(2),
            GuestfsState::Error => Ok(3),
            GuestfsState::Closed => Ok(4),
        }
    }

    /// Check if config
    ///
    pub fn is_config(&self) -> Result<bool> {
        use crate::guestfs::handle::GuestfsState;
        Ok(matches!(self.state, GuestfsState::Config))
    }

    /// Check if launching
    ///
    pub fn is_launching(&self) -> Result<bool> {
        // We don't have a separate launching state in our implementation
        Ok(false)
    }

    /// Check if ready
    ///
    pub fn is_ready(&self) -> Result<bool> {
        use crate::guestfs::handle::GuestfsState;
        Ok(matches!(self.state, GuestfsState::Ready))
    }

    /// Check if busy
    ///
    pub fn is_busy(&self) -> Result<bool> {
        // For this implementation, we're never truly "busy"
        // since operations are synchronous
        Ok(false)
    }

    /// Get PID (not applicable for NBD backend)
    ///
    pub fn get_pid(&self) -> Result<i32> {
        // Return 0 since we don't run a separate daemon process
        Ok(0)
    }

    /// User cancel (not implemented for sync operations)
    ///
    pub fn user_cancel(&mut self) -> Result<()> {
        // No-op for synchronous operations
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_internal_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }

    #[test]
    fn statvfs_errors_cleanly_before_launch() {
        // Real end-to-end: no mounted root to resolve a guest path against,
        // so this must return an error rather than panicking - matches the
        // pre-launch graceful-degradation pattern used throughout the crate.
        let mut g = Guestfs::new().unwrap();
        assert!(g.statvfs("/").is_err());
    }
}
