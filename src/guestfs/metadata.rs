// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! File metadata operations for disk image manipulation
//!
//! This implementation provides file metadata and stat operations.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;
use std::fs;
use std::os::unix::fs::PermissionsExt;

/// File stat information
#[derive(Debug, Clone)]
pub struct Stat {
    pub dev: u64,
    pub ino: u64,
    pub mode: u32,
    pub nlink: u64,
    pub uid: u32,
    pub gid: u32,
    pub rdev: u64,
    pub size: i64,
    pub blksize: i64,
    pub blocks: i64,
    pub atime: i64,
    pub mtime: i64,
    pub ctime: i64,
    pub atime_nsec: i64,
    pub mtime_nsec: i64,
    pub ctime_nsec: i64,
}

impl Guestfs {
    /// Get file or directory status
    ///
    pub fn stat(&mut self, path: &str) -> Result<Stat> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: stat {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;
        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        self.metadata_to_stat(&metadata)
    }

    /// Get symbolic link status (don't follow links)
    ///
    pub fn lstat(&mut self, path: &str) -> Result<Stat> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lstat {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;
        let metadata = fs::symlink_metadata(&host_path).map_err(Error::Io)?;

        self.metadata_to_stat(&metadata)
    }

    /// Convert Rust Metadata to Stat struct
    fn metadata_to_stat(&self, metadata: &fs::Metadata) -> Result<Stat> {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            Ok(Stat {
                dev: metadata.dev(),
                ino: metadata.ino(),
                mode: metadata.mode(),
                nlink: metadata.nlink(),
                uid: metadata.uid(),
                gid: metadata.gid(),
                rdev: metadata.rdev(),
                size: metadata.size() as i64,
                blksize: metadata.blksize() as i64,
                blocks: metadata.blocks() as i64,
                atime: metadata.atime(),
                mtime: metadata.mtime(),
                ctime: metadata.ctime(),
                atime_nsec: metadata.atime_nsec(),
                mtime_nsec: metadata.mtime_nsec(),
                ctime_nsec: metadata.ctime_nsec(),
            })
        }

        #[cfg(not(unix))]
        {
            Ok(Stat {
                dev: 0,
                ino: 0,
                mode: if metadata.is_dir() { 0o40755 } else { 0o100644 },
                nlink: 1,
                uid: 0,
                gid: 0,
                rdev: 0,
                size: metadata.len() as i64,
                blksize: 4096,
                blocks: (metadata.len() + 4095) / 4096,
                atime: 0,
                mtime: 0,
                ctime: 0,
                atime_nsec: 0,
                mtime_nsec: 0,
                ctime_nsec: 0,
            })
        }
    }

    /// Get file or directory status with nanosecond-resolution timestamps.
    ///
    /// Same as `stat()`, but the returned `Stat` also carries
    /// `atime_nsec`/`mtime_nsec`/`ctime_nsec`.
    pub fn statns(&mut self, path: &str) -> Result<Stat> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: statns {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;
        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        self.metadata_to_stat(&metadata)
    }

    /// Get file inode number
    ///
    pub fn get_inode(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_inode {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.ino())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "Inode numbers not supported on this platform".to_string(),
            ))
        }
    }

    /// Get file access time
    ///
    pub fn get_atime(&mut self, path: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_atime {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.atime())
        }

        #[cfg(not(unix))]
        {
            let metadata = fs::metadata(&host_path).map_err(|e| Error::Io(e))?;
            if let Ok(accessed) = metadata.accessed() {
                if let Ok(duration) = accessed.duration_since(std::time::UNIX_EPOCH) {
                    return Ok(duration.as_secs() as i64);
                }
            }
            Err(Error::Unsupported("Access time not available".to_string()))
        }
    }

    /// Get file modification time
    ///
    pub fn get_mtime(&mut self, path: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_mtime {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.mtime())
        }

        #[cfg(not(unix))]
        {
            let metadata = fs::metadata(&host_path).map_err(|e| Error::Io(e))?;
            if let Ok(modified) = metadata.modified() {
                if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                    return Ok(duration.as_secs() as i64);
                }
            }
            Err(Error::Unsupported(
                "Modification time not available".to_string(),
            ))
        }
    }

    /// Get file change time
    ///
    pub fn get_ctime(&mut self, path: &str) -> Result<i64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_ctime {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.ctime())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "Change time not supported on this platform".to_string(),
            ))
        }
    }

    /// Get file UID
    ///
    pub fn get_uid(&mut self, path: &str) -> Result<u32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_uid {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.uid())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "UID not supported on this platform".to_string(),
            ))
        }
    }

    /// Get file GID
    ///
    pub fn get_gid(&mut self, path: &str) -> Result<u32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_gid {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.gid())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "GID not supported on this platform".to_string(),
            ))
        }
    }

    /// Get file mode (permissions)
    ///
    pub fn get_mode(&mut self, path: &str) -> Result<u32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_mode {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            Ok(metadata.permissions().mode())
        }

        #[cfg(not(unix))]
        {
            // On Windows, just check read-only
            let mode = if metadata.permissions().readonly() {
                0o444
            } else {
                0o644
            };
            Ok(mode)
        }
    }

    /// Get number of hard links
    ///
    pub fn get_nlink(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_nlink {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.nlink())
        }

        #[cfg(not(unix))]
        {
            Ok(1) // Windows doesn't have hard link count in standard metadata
        }
    }

    /// Get device ID
    ///
    pub fn get_dev(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_dev {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.dev())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "Device ID not supported on this platform".to_string(),
            ))
        }
    }

    /// Get device type for special files
    ///
    pub fn get_rdev(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_rdev {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.rdev())
        }

        #[cfg(not(unix))]
        {
            Err(Error::Unsupported(
                "Device type not supported on this platform".to_string(),
            ))
        }
    }

    /// Get block count
    ///
    pub fn get_blocks(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_blocks {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.blocks())
        }

        #[cfg(not(unix))]
        {
            // Approximate from file size
            let metadata = fs::metadata(&host_path).map_err(|e| Error::Io(e))?;
            Ok((metadata.len() + 511) / 512) // Round up to 512-byte blocks
        }
    }

    /// Get block size
    ///
    pub fn get_blksize(&mut self, path: &str) -> Result<u64> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_blksize {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::metadata(&host_path).map_err(Error::Io)?;
            Ok(metadata.blksize())
        }

        #[cfg(not(unix))]
        {
            Ok(4096) // Standard block size
        }
    }

    /// Check if path is socket
    ///
    pub fn is_socket(&mut self, path: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_socket {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            Ok(metadata.file_type().is_socket())
        }

        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    /// Check if path is FIFO (named pipe)
    ///
    pub fn is_fifo(&mut self, path: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_fifo {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            Ok(metadata.file_type().is_fifo())
        }

        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    /// Check if path is block device
    ///
    pub fn is_blockdev(&mut self, path: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_blockdev {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            Ok(metadata.file_type().is_block_device())
        }

        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    /// Check if path is character device
    ///
    pub fn is_chardev(&mut self, path: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_chardev {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let metadata = fs::metadata(&host_path).map_err(Error::Io)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::FileTypeExt;
            Ok(metadata.file_type().is_char_device())
        }

        #[cfg(not(unix))]
        {
            Ok(false)
        }
    }

    /// Check if path is symbolic link
    ///
    pub fn is_symlink(&mut self, path: &str) -> Result<bool> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: is_symlink {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        // Use symlink_metadata to not follow symlinks
        let metadata = fs::symlink_metadata(&host_path).map_err(Error::Io)?;

        Ok(metadata.file_type().is_symlink())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metadata_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
