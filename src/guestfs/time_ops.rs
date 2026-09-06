// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Time and timestamp operations for disk image manipulation
//!
//! This implementation provides timestamp manipulation functionality.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Set file timestamps (access and modification)
    ///
    /// Already exists as utimens, adding enhanced version
    pub fn set_file_times(&mut self, path: &str, atime: i64, mtime: i64) -> Result<()> {
        self.utimens(path, atime, 0, mtime, 0)
    }

    /// Get file access time (enhanced version)
    ///
    /// Already exists as get_atime, adding alias
    pub fn file_atime(&mut self, path: &str) -> Result<i64> {
        self.get_atime(path)
    }

    /// Get file modification time (enhanced version)
    ///
    /// Already exists as get_mtime, adding alias
    pub fn file_mtime(&mut self, path: &str) -> Result<i64> {
        self.get_mtime(path)
    }

    /// Get file change time (enhanced version)
    ///
    /// Already exists as get_ctime, adding alias
    pub fn file_ctime(&mut self, path: &str) -> Result<i64> {
        self.get_ctime(path)
    }

    /// Copy timestamps from one file to another
    ///
    /// Additional functionality for timestamp copying
    pub fn copy_timestamps(&mut self, src: &str, dest: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: copy_timestamps {} {}", src, dest);
        }

        let atime = self.get_atime(src)?;
        let mtime = self.get_mtime(src)?;

        self.utimens(dest, atime, 0, mtime, 0)
    }

    /// Set timestamps to current time
    ///
    /// Additional functionality for touch-like operations
    pub fn touch_with_time(&mut self, path: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: touch_with_time {}", path);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::InvalidFormat(format!("Time error: {}", e)))?
            .as_secs() as i64;

        self.utimens(path, now, 0, now, 0)
    }

    /// Get oldest file in directory
    ///
    /// Additional functionality for time-based queries
    pub fn find_oldest_file(&mut self, directory: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: find_oldest_file {}", directory);
        }

        let files = self.find(directory)?;
        let mut oldest_file = String::new();
        let mut oldest_time = i64::MAX;

        for file in files {
            if self.is_file(&file).unwrap_or(false) {
                if let Ok(mtime) = self.get_mtime(&file) {
                    if mtime < oldest_time {
                        oldest_time = mtime;
                        oldest_file = file;
                    }
                }
            }
        }

        if oldest_file.is_empty() {
            return Err(Error::NotFound("No files found".to_string()));
        }

        Ok(oldest_file)
    }

    /// Get newest file in directory
    ///
    /// Additional functionality for time-based queries
    pub fn find_newest_file(&mut self, directory: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: find_newest_file {}", directory);
        }

        let files = self.find(directory)?;
        let mut newest_file = String::new();
        let mut newest_time = i64::MIN;

        for file in files {
            if self.is_file(&file).unwrap_or(false) {
                if let Ok(mtime) = self.get_mtime(&file) {
                    if mtime > newest_time {
                        newest_time = mtime;
                        newest_file = file;
                    }
                }
            }
        }

        if newest_file.is_empty() {
            return Err(Error::NotFound("No files found".to_string()));
        }

        Ok(newest_file)
    }

    /// Find files modified within time range
    ///
    /// Additional functionality for time-based queries
    pub fn find_by_mtime(
        &mut self,
        directory: &str,
        start_time: i64,
        end_time: i64,
    ) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!(
                "guestfs: find_by_mtime {} {} {}",
                directory, start_time, end_time
            );
        }

        let files = self.find(directory)?;
        let mut matching_files = Vec::new();

        for file in files {
            if self.is_file(&file).unwrap_or(false) {
                if let Ok(mtime) = self.get_mtime(&file) {
                    if mtime >= start_time && mtime <= end_time {
                        matching_files.push(file);
                    }
                }
            }
        }

        Ok(matching_files)
    }

    /// Find files older than N days
    ///
    /// Additional functionality for cleanup operations
    pub fn find_old_files(&mut self, directory: &str, days: i32) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: find_old_files {} {}", directory, days);
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| Error::InvalidFormat(format!("Time error: {}", e)))?
            .as_secs() as i64;

        let cutoff_time = now - (days as i64 * 86400); // 86400 seconds per day

        let files = self.find(directory)?;
        let mut old_files = Vec::new();

        for file in files {
            if self.is_file(&file).unwrap_or(false) {
                if let Ok(mtime) = self.get_mtime(&file) {
                    if mtime < cutoff_time {
                        old_files.push(file);
                    }
                }
            }
        }

        Ok(old_files)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_time_ops_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
