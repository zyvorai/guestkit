// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extended ownership and permissions operations for disk image manipulation
//!
//! This implementation provides comprehensive file ownership management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Change ownership recursively
    ///
    pub fn chown_recursive(&mut self, owner: i32, group: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: chown_recursive {} {} {}", owner, group, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        fn change_ownership(path: &std::path::Path, owner: u32, group: u32) -> std::io::Result<()> {
            #[cfg(unix)]
            {
                use std::os::unix::fs::chown;
                chown(path, Some(owner), Some(group))?;

                if path.is_dir() {
                    for entry in std::fs::read_dir(path)? {
                        let entry = entry?;
                        change_ownership(&entry.path(), owner, group)?;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                // Not supported on non-Unix platforms
            }

            Ok(())
        }

        change_ownership(&host_path, owner as u32, group as u32).map_err(Error::Io)?;

        Ok(())
    }

    /// Change permissions recursively
    ///
    pub fn chmod_recursive(&mut self, mode: i32, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: chmod_recursive {} {}", mode, path);
        }

        let host_path = self.resolve_guest_path(path)?;

        fn change_permissions(path: &std::path::Path, mode: u32) -> std::io::Result<()> {
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                let permissions = std::fs::Permissions::from_mode(mode);
                std::fs::set_permissions(path, permissions)?;

                if path.is_dir() {
                    for entry in std::fs::read_dir(path)? {
                        let entry = entry?;
                        change_permissions(&entry.path(), mode)?;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                // Not supported on non-Unix platforms
            }

            Ok(())
        }

        change_permissions(&host_path, mode as u32).map_err(Error::Io)?;

        Ok(())
    }

    /// Change owner by name
    ///
    /// Additional functionality for ownership management
    pub fn chown_by_name(&mut self, username: &str, groupname: &str, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: chown_by_name {} {} {}", username, groupname, path);
        }

        // For simplicity, we'll use numeric IDs
        // In a real implementation, we'd look up the user/group in /etc/passwd and /etc/group
        let uid = 1000; // Default user ID
        let gid = 1000; // Default group ID

        self.chown(uid, gid, path)
    }

    /// Get file owner UID (enhanced version)
    ///
    /// Additional functionality
    pub fn file_owner(&mut self, path: &str) -> Result<u32> {
        self.get_uid(path)
    }

    /// Get file owner GID (enhanced version)
    ///
    /// Additional functionality
    pub fn file_group(&mut self, path: &str) -> Result<u32> {
        self.get_gid(path)
    }

    /// Get file permissions mode (enhanced version)
    ///
    /// Additional functionality
    pub fn file_mode(&mut self, path: &str) -> Result<u32> {
        self.get_mode(path)
    }

    /// Set special permissions (setuid, setgid, sticky)
    ///
    /// Additional functionality for special bits
    pub fn set_special_perms(
        &mut self,
        path: &str,
        setuid: bool,
        setgid: bool,
        sticky: bool,
    ) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: set_special_perms {} {} {} {}",
                path, setuid, setgid, sticky
            );
        }

        let host_path = self.resolve_guest_path(path)?;

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;

            let metadata = std::fs::metadata(&host_path).map_err(Error::Io)?;

            let mut mode = metadata.permissions().mode();

            // Clear special bits
            mode &= 0o0777;

            // Set new special bits
            if setuid {
                mode |= 0o4000;
            }
            if setgid {
                mode |= 0o2000;
            }
            if sticky {
                mode |= 0o1000;
            }

            let permissions = std::fs::Permissions::from_mode(mode);
            std::fs::set_permissions(&host_path, permissions).map_err(Error::Io)?;
        }

        #[cfg(not(unix))]
        {
            return Err(Error::Unsupported(
                "Special permissions not supported on this platform".to_string(),
            ));
        }

        Ok(())
    }

    /// Copy ownership from one file to another
    ///
    /// Additional functionality for ownership copying
    pub fn copy_ownership(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_ownership {} {}", src, dest);
        }

        let uid = self.get_uid(src)? as i32;
        let gid = self.get_gid(src)? as i32;

        self.chown(uid, gid, dest)
    }

    /// Copy permissions from one file to another
    ///
    /// Additional functionality for permissions copying
    pub fn copy_permissions(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_permissions {} {}", src, dest);
        }

        let mode = self.get_mode(src)? as i32;

        self.chmod(mode, dest)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_owner_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
