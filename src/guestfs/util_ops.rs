// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Additional utility operations for disk image manipulation
//!
//! This implementation provides miscellaneous utility functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    // Note: filesize, du, df_h, ping_daemon already exist in other modules

    /// Get library version
    ///
    pub fn version_info(&mut self) -> Result<(i64, i64, i64, String)> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: version_info");
        }

        // Return guestkit version
        Ok((0, 1, 0, "guestkit".to_string()))
    }

    /// Get default QEMU binary
    ///
    pub fn get_qemu(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_qemu");
        }

        // Check for qemu-system-x86_64
        let qemu_paths = vec![
            "/usr/bin/qemu-system-x86_64",
            "/usr/bin/qemu-kvm",
            "/usr/libexec/qemu-kvm",
        ];

        for path in qemu_paths {
            if std::path::Path::new(path).exists() {
                return Ok(path.to_string());
            }
        }

        Ok("/usr/bin/qemu-system-x86_64".to_string())
    }

    /// Get current umask
    ///
    pub fn get_umask(&mut self) -> Result<i32> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_umask");
        }

        // Get current umask by setting and restoring
        #[cfg(unix)]
        {
            // This is a simplified implementation
            Ok(0o022)
        }

        #[cfg(not(unix))]
        {
            Ok(0o022)
        }
    }

    // Note: is_blockdev and is_chardev already exist in metadata.rs

    /// Get file major/minor device numbers
    ///
    pub fn stat_device(&mut self, path: &str) -> Result<(i64, i64)> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: stat_device {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;
            let metadata = std::fs::metadata(&host_path).map_err(Error::Io)?;

            let rdev = metadata.rdev();
            let major = (rdev >> 8) as i64;
            let minor = (rdev & 0xff) as i64;

            Ok((major, minor))
        }

        #[cfg(not(unix))]
        {
            Ok((0, 0))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_util_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
