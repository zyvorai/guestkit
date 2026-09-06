// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Mock VM disk generator for performance testing
//!
//! This module provides utilities to generate mock disk images with various
//! characteristics for testing and benchmarking purposes.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};

/// Type of mock disk to generate
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MockDiskType {
    /// Minimal disk with basic structure
    Minimal,
    /// Small disk (~10MB) with basic OS structure
    Small,
    /// Medium disk (~100MB) with typical VM content
    Medium,
    /// Large disk (~1GB) for stress testing
    Large,
    /// Custom size disk
    Custom(u64),
}

impl MockDiskType {
    /// Get size in bytes for this disk type
    pub fn size_bytes(&self) -> u64 {
        match self {
            MockDiskType::Minimal => 1024 * 1024,      // 1 MB
            MockDiskType::Small => 10 * 1024 * 1024,   // 10 MB
            MockDiskType::Medium => 100 * 1024 * 1024, // 100 MB
            MockDiskType::Large => 1024 * 1024 * 1024, // 1 GB
            MockDiskType::Custom(size) => *size,
        }
    }
}

/// Mock disk builder for configurable disk generation
pub struct MockDiskBuilder {
    disk_type: MockDiskType,
    os_type: String,
    hostname: String,
    num_files: usize,
    num_users: usize,
    num_packages: usize,
}

impl Default for MockDiskBuilder {
    fn default() -> Self {
        Self {
            disk_type: MockDiskType::Small,
            os_type: "linux".to_string(),
            hostname: "test-vm".to_string(),
            num_files: 100,
            num_users: 5,
            num_packages: 50,
        }
    }
}

impl MockDiskBuilder {
    /// Create a new mock disk builder
    pub fn new() -> Self {
        Self::default()
    }

    /// Set disk type
    pub fn disk_type(mut self, disk_type: MockDiskType) -> Self {
        self.disk_type = disk_type;
        self
    }

    /// Set OS type
    pub fn os_type(mut self, os_type: impl Into<String>) -> Self {
        self.os_type = os_type.into();
        self
    }

    /// Set hostname
    pub fn hostname(mut self, hostname: impl Into<String>) -> Self {
        self.hostname = hostname.into();
        self
    }

    /// Set number of files to generate
    pub fn num_files(mut self, num: usize) -> Self {
        self.num_files = num;
        self
    }

    /// Set number of users to generate
    pub fn num_users(mut self, num: usize) -> Self {
        self.num_users = num;
        self
    }

    /// Set number of packages to simulate
    pub fn num_packages(mut self, num: usize) -> Self {
        self.num_packages = num;
        self
    }

    /// Build the mock disk
    pub fn build(self, path: impl AsRef<Path>) -> std::io::Result<MockDisk> {
        MockDisk::create(path, self)
    }
}

/// Mock VM disk for testing
pub struct MockDisk {
    path: PathBuf,
    disk_type: MockDiskType,
    metadata: DiskMetadata,
}

#[derive(Debug, Clone)]
struct DiskMetadata {
    os_type: String,
    hostname: String,
    num_files: usize,
    num_users: usize,
    num_packages: usize,
    size_bytes: u64,
}

impl MockDisk {
    /// Create a new mock disk with default settings
    pub fn create(path: impl AsRef<Path>, builder: MockDiskBuilder) -> std::io::Result<Self> {
        let path = path.as_ref().to_path_buf();
        let size = builder.disk_type.size_bytes();

        // Create sparse file of specified size
        let mut file = File::create(&path)?;

        // Write header with metadata
        let header = format!(
            "MOCK_DISK v1.0\nOS: {}\nHostname: {}\nFiles: {}\nUsers: {}\nPackages: {}\n",
            builder.os_type,
            builder.hostname,
            builder.num_files,
            builder.num_users,
            builder.num_packages
        );
        file.write_all(header.as_bytes())?;

        // Fill with pseudo-random data to simulate disk content
        let block_size = 4096;
        let num_blocks = (size / block_size) as usize;

        // Write some data blocks (sparse to save space)
        for i in (0..num_blocks).step_by(100) {
            let block = vec![(i % 256) as u8; block_size as usize];
            file.write_all(&block)?;

            // Seek to next position for sparseness
            if i + 100 < num_blocks {
                use std::io::Seek;
                file.seek(std::io::SeekFrom::Current((99 * block_size) as i64))?;
            }
        }

        file.sync_all()?;

        let metadata = DiskMetadata {
            os_type: builder.os_type,
            hostname: builder.hostname,
            num_files: builder.num_files,
            num_users: builder.num_users,
            num_packages: builder.num_packages,
            size_bytes: size,
        };

        Ok(MockDisk {
            path,
            disk_type: builder.disk_type,
            metadata,
        })
    }

    /// Get the path to this mock disk
    pub fn path(&self) -> &Path {
        &self.path
    }

    /// Get the disk type
    pub fn disk_type(&self) -> MockDiskType {
        self.disk_type
    }

    /// Get the expected OS type
    pub fn os_type(&self) -> &str {
        &self.metadata.os_type
    }

    /// Get the expected hostname
    pub fn hostname(&self) -> &str {
        &self.metadata.hostname
    }

    /// Get the number of files
    pub fn num_files(&self) -> usize {
        self.metadata.num_files
    }

    /// Get the number of users
    pub fn num_users(&self) -> usize {
        self.metadata.num_users
    }

    /// Get the number of packages
    pub fn num_packages(&self) -> usize {
        self.metadata.num_packages
    }

    /// Get the disk size in bytes
    pub fn size_bytes(&self) -> u64 {
        self.metadata.size_bytes
    }

    /// Delete the mock disk file
    pub fn cleanup(self) -> std::io::Result<()> {
        fs::remove_file(&self.path)
    }
}

impl Drop for MockDisk {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn test_mock_disk_type_sizes() {
        assert_eq!(MockDiskType::Minimal.size_bytes(), 1024 * 1024);
        assert_eq!(MockDiskType::Small.size_bytes(), 10 * 1024 * 1024);
        assert_eq!(MockDiskType::Medium.size_bytes(), 100 * 1024 * 1024);
        assert_eq!(MockDiskType::Large.size_bytes(), 1024 * 1024 * 1024);
        assert_eq!(MockDiskType::Custom(5000).size_bytes(), 5000);
    }

    #[test]
    fn test_mock_disk_builder_default() {
        let builder = MockDiskBuilder::default();
        assert_eq!(builder.os_type, "linux");
        assert_eq!(builder.hostname, "test-vm");
        assert_eq!(builder.num_files, 100);
        assert_eq!(builder.num_users, 5);
        assert_eq!(builder.num_packages, 50);
    }

    #[test]
    fn test_mock_disk_builder_customization() {
        let builder = MockDiskBuilder::new()
            .disk_type(MockDiskType::Medium)
            .os_type("ubuntu")
            .hostname("custom-host")
            .num_files(200)
            .num_users(10)
            .num_packages(100);

        assert_eq!(builder.os_type, "ubuntu");
        assert_eq!(builder.hostname, "custom-host");
        assert_eq!(builder.num_files, 200);
    }

    #[test]
    fn test_mock_disk_creation() {
        let temp_dir = tempdir().unwrap();
        let disk_path = temp_dir.path().join("test.img");

        let disk = MockDiskBuilder::new()
            .disk_type(MockDiskType::Minimal)
            .os_type("debian")
            .hostname("test-debian")
            .build(&disk_path)
            .unwrap();

        assert!(disk.path().exists());
        assert_eq!(disk.os_type(), "debian");
        assert_eq!(disk.hostname(), "test-debian");
        assert_eq!(disk.disk_type(), MockDiskType::Minimal);
    }

    #[test]
    fn test_mock_disk_file_exists() {
        let temp_dir = tempdir().unwrap();
        let disk_path = temp_dir.path().join("exists.img");

        let disk = MockDiskBuilder::new()
            .disk_type(MockDiskType::Small)
            .build(&disk_path)
            .unwrap();

        assert!(disk.path().exists());

        let metadata = fs::metadata(disk.path()).unwrap();
        assert!(metadata.len() > 0);
    }

    #[test]
    fn test_mock_disk_cleanup() {
        let temp_dir = tempdir().unwrap();
        let disk_path = temp_dir.path().join("cleanup.img");

        let disk = MockDiskBuilder::new()
            .disk_type(MockDiskType::Minimal)
            .build(&disk_path)
            .unwrap();

        let path = disk.path().to_path_buf();
        assert!(path.exists());

        disk.cleanup().unwrap();
        assert!(!path.exists());
    }

    #[test]
    fn test_mock_disk_drop_cleanup() {
        let temp_dir = tempdir().unwrap();
        let disk_path = temp_dir.path().join("drop.img");

        let path_clone = disk_path.clone();
        {
            let _disk = MockDiskBuilder::new()
                .disk_type(MockDiskType::Minimal)
                .build(&disk_path)
                .unwrap();

            assert!(path_clone.exists());
        }
        // After drop, file should be cleaned up
        assert!(!path_clone.exists());
    }

    #[test]
    fn test_mock_disk_metadata() {
        let temp_dir = tempdir().unwrap();
        let disk_path = temp_dir.path().join("metadata.img");

        let disk = MockDiskBuilder::new()
            .num_files(150)
            .num_users(8)
            .num_packages(75)
            .build(&disk_path)
            .unwrap();

        assert_eq!(disk.num_files(), 150);
        assert_eq!(disk.num_users(), 8);
        assert_eq!(disk.num_packages(), 75);
    }
}
