// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extended link operations for disk image manipulation
//!
//! This implementation provides symbolic and hard link management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Read link target (already exists as readlink, adding extended version)
    ///
    pub fn read_link(&mut self, path: &str) -> Result<String> {
        self.readlink(path)
    }

    /// Read link without following
    ///
    /// Additional functionality for link handling
    pub fn lreadlink(&mut self, path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: lreadlink {}", path);
        }

        let host_path = self.resolve_guest_path(path)?;

        let target = std::fs::read_link(&host_path).map_err(Error::Io)?;

        Ok(target.to_string_lossy().to_string())
    }

    /// List all symbolic links in directory
    ///
    /// Additional functionality for link discovery
    pub fn find_links(&mut self, directory: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: find_links {}", directory);
        }

        let host_path = self.resolve_guest_path(directory)?;
        let mut links = Vec::new();

        fn scan_directory(
            path: &std::path::Path,
            links: &mut Vec<String>,
            base: &std::path::Path,
        ) -> std::io::Result<()> {
            if path.is_dir() {
                for entry in std::fs::read_dir(path)? {
                    let entry = entry?;
                    let entry_path = entry.path();

                    if entry_path.is_symlink() {
                        if let Ok(relative) = entry_path.strip_prefix(base) {
                            links.push(format!("/{}", relative.display()));
                        }
                    }

                    if entry_path.is_dir() && !entry_path.is_symlink() {
                        scan_directory(&entry_path, links, base)?;
                    }
                }
            }
            Ok(())
        }

        scan_directory(&host_path, &mut links, &host_path).map_err(Error::Io)?;

        Ok(links)
    }

    /// Check if path is a symbolic link
    ///
    /// Already exists as is_symlink, adding alias
    pub fn is_link(&mut self, path: &str) -> Result<bool> {
        self.is_symlink(path)
    }

    /// Create symbolic link (relative)
    ///
    /// Additional functionality for relative links
    pub fn symlink_relative(&mut self, target: &str, linkname: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: symlink_relative {} {}", target, linkname);
        }

        let link_path = self.resolve_guest_path(linkname)?;

        #[cfg(unix)]
        {
            std::os::unix::fs::symlink(target, &link_path).map_err(Error::Io)?;
        }

        #[cfg(not(unix))]
        {
            return Err(Error::NotSupported(
                "Symbolic links not supported on this platform".to_string(),
            ));
        }

        Ok(())
    }

    /// Remove symbolic link
    ///
    /// Additional functionality for link removal
    pub fn remove_link(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: remove_link {}", path);
        }

        if !self.is_symlink(path)? {
            return Err(Error::InvalidFormat(format!(
                "{} is not a symbolic link",
                path
            )));
        }

        self.rm(path)
    }

    /// Copy link (preserve symlink)
    ///
    /// Additional functionality for link copying
    pub fn copy_link(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_link {} {}", src, dest);
        }

        if !self.is_symlink(src)? {
            return Err(Error::InvalidFormat(format!(
                "{} is not a symbolic link",
                src
            )));
        }

        let target = self.readlink(src)?;
        self.ln_s(&target, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_link_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
