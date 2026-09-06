// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Synchronization operations for disk image manipulation
//!
//! This implementation provides file synchronization functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Synchronize filesystem (already exists as sync, adding extended version)
    ///
    pub fn sync_all(&mut self) -> Result<()> {
        self.sync()
    }

    /// Synchronize file data
    ///
    /// Already exists as fsync in node_ops, adding alias
    pub fn file_sync(&mut self, path: &str) -> Result<()> {
        self.fsync(path)
    }

    /// Drop caches
    ///
    pub fn drop_caches(&mut self, whattodrop: i32) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: drop_caches {}", whattodrop);
        }

        // Write to /proc/sys/vm/drop_caches on host
        // This affects the host system when we're using NBD
        let drop_value = match whattodrop {
            1 => "1", // pagecache
            2 => "2", // dentries and inodes
            3 => "3", // pagecache, dentries and inodes
            _ => {
                return Err(Error::InvalidFormat(format!(
                    "Invalid drop_caches value: {}",
                    whattodrop
                )))
            }
        };

        std::fs::write("/proc/sys/vm/drop_caches", drop_value).map_err(Error::Io)?;

        Ok(())
    }

    /// Sync disks and exit
    ///
    /// Additional functionality for clean shutdown
    pub fn sync_and_close(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: sync_and_close");
        }

        // Sync all filesystems
        self.sync()?;

        // Unmount all
        self.umount_all()?;

        // Close handle
        self.shutdown()?;

        Ok(())
    }

    /// Flush all writes
    ///
    /// Additional functionality for ensuring data consistency
    pub fn flush_all(&mut self) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: flush_all");
        }

        // Sync filesystem
        self.sync()?;

        // Flush NBD device if available
        if let Some(device) = self.list_devices()?.first() {
            let _ = self.blockdev_flushbufs(device);
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sync_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
