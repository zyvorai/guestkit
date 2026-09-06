// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Rsync operations for disk image manipulation
//!
//! This implementation provides rsync-based file synchronization functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::process::Command;

impl Guestfs {
    /// Synchronize files using rsync (from guest)
    ///
    pub fn rsync_out(
        &mut self,
        src: &str,
        dest: &str,
        archive: bool,
        deletedest: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: rsync_out {} {}", src, dest);
        }

        let host_src = self.resolve_guest_path(src)?;

        let mut cmd = Command::new("rsync");

        if archive {
            cmd.arg("-a");
        } else {
            cmd.arg("-r");
        }

        if deletedest {
            cmd.arg("--delete");
        }

        cmd.arg(&host_src).arg(dest);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute rsync: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "rsync failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(())
    }

    /// Synchronize files using rsync (to guest)
    ///
    pub fn rsync_in(
        &mut self,
        src: &str,
        dest: &str,
        archive: bool,
        deletedest: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: rsync_in {} {}", src, dest);
        }

        let host_dest = self.resolve_guest_path(dest)?;

        let mut cmd = Command::new("rsync");

        if archive {
            cmd.arg("-a");
        } else {
            cmd.arg("-r");
        }

        if deletedest {
            cmd.arg("--delete");
        }

        cmd.arg(src).arg(&host_dest);

        let output = cmd
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute rsync: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "rsync failed: {}",
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
    fn test_rsync_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
