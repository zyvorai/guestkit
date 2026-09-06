// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Basic integration tests for guestkit
//!
//! These tests create real disk images and perform operations on them.

use guestkit::Guestfs;
use std::fs;
use std::path::Path;

/// Helper to create a test disk image
#[allow(dead_code)]
fn create_test_disk(path: &str, size_mb: i64) -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;
    g.disk_create(path, "raw", size_mb * 1024 * 1024)?;
    Ok(())
}

/// Helper to cleanup test disk
fn cleanup_disk(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_disk_creation_and_inspection() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_disk_creation.img";
    cleanup_disk(disk_path);

    // Create a 100MB disk
    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;

    // Verify disk exists
    assert!(Path::new(disk_path).exists());

    // Add and inspect
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // List devices (should have at least one)
    let devices = g.list_devices()?;
    assert!(!devices.is_empty());
    assert_eq!(devices[0], "/dev/sda");

    // Get device size
    let size = g.blockdev_getsize64("/dev/sda")?;
    assert!(size >= 100 * 1024 * 1024);

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_partition_creation() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_partition.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 200 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create GPT partition table
    g.part_init("/dev/sda", "gpt")?;

    // Verify partition table type
    let pttype = g.part_get_parttype("/dev/sda")?;
    assert_eq!(pttype, "gpt");

    // Create a partition
    g.part_add("/dev/sda", "primary", 2048, 206847)?;

    // List partitions
    let partitions = g.list_partitions()?;
    assert_eq!(partitions.len(), 1);
    assert_eq!(partitions[0], "/dev/sda1");

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_filesystem_creation_and_mount() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_filesystem.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create partition table and partition
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;

    // Create ext4 filesystem
    g.mkfs("ext4", "/dev/sda1")?;

    // Verify filesystem type
    let fstype = g.vfs_type("/dev/sda1")?;
    assert_eq!(fstype, "ext4");

    // Mount the filesystem
    g.mount("/dev/sda1", "/")?;

    // Verify it's mounted
    let mounts = g.mounts()?;
    assert!(mounts.contains(&"/dev/sda1".to_string()));

    // Unmount
    g.umount("/")?;

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_file_operations() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_file_ops.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Test file creation
    let content = b"Hello, GuestKit!";
    g.write("/test.txt", content)?;

    // Verify file exists
    assert!(g.exists("/test.txt")?);
    assert!(g.is_file("/test.txt")?);

    // Read file back
    let read_content = g.cat("/test.txt")?;
    assert_eq!(read_content, "Hello, GuestKit!");

    // Test file size
    let size = g.filesize("/test.txt")?;
    assert_eq!(size, content.len() as i64);

    // Test directory creation
    g.mkdir_p("/data/configs")?;
    assert!(g.is_dir("/data")?);
    assert!(g.is_dir("/data/configs")?);

    // Test file listing
    let files = g.ls("/")?;
    assert!(files.contains(&"test.txt".to_string()));
    assert!(files.contains(&"data".to_string()));

    // Test file deletion
    g.rm("/test.txt")?;
    assert!(!g.exists("/test.txt")?);

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_archive_operations() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_archive.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create test files
    g.mkdir_p("/data")?;
    g.write("/data/file1.txt", b"Content 1")?;
    g.write("/data/file2.txt", b"Content 2")?;

    // Create tar archive
    g.tar_out("/data", "/archive.tar")?;

    // Verify archive exists
    assert!(g.exists("/archive.tar")?);

    // Delete original directory
    g.rm_rf("/data")?;
    assert!(!g.exists("/data")?);

    // Extract archive
    g.tar_in("/archive.tar", "/")?;

    // Verify files restored
    assert!(g.exists("/data/file1.txt")?);
    assert!(g.exists("/data/file2.txt")?);

    let content1 = g.cat("/data/file1.txt")?;
    assert_eq!(content1, "Content 1");

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_checksums() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_checksum.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create test file
    g.write("/test.txt", b"Hello, World!")?;

    // Calculate checksums
    let md5 = g.checksum("md5", "/test.txt")?;
    let sha256 = g.checksum("sha256", "/test.txt")?;

    // Verify checksums are non-empty and have expected length
    assert_eq!(md5.len(), 32); // MD5 is 32 hex chars
    assert_eq!(sha256.len(), 64); // SHA256 is 64 hex chars

    // Write the same content again and verify checksums match
    g.write("/test2.txt", b"Hello, World!")?;
    let md5_2 = g.checksum("md5", "/test2.txt")?;
    let sha256_2 = g.checksum("sha256", "/test2.txt")?;

    assert_eq!(md5, md5_2);
    assert_eq!(sha256, sha256_2);

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_stat_operations() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_stat.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create file
    let content = b"Test content for stat";
    g.write("/test.txt", content)?;

    // Get file stats
    let stat = g.stat("/test.txt")?;

    // Verify stat information
    assert_eq!(stat.size, content.len() as i64);
    assert!(stat.mode & 0o100000 != 0); // Regular file flag
    assert!(stat.atime > 0);
    assert!(stat.mtime > 0);
    assert!(stat.ctime > 0);

    // Test directory stat
    g.mkdir("/testdir")?;
    let dir_stat = g.stat("/testdir")?;
    assert!(dir_stat.mode & 0o040000 != 0); // Directory flag

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_command_execution() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_command.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create test file
    g.write("/test.txt", b"Line 1\nLine 2\nLine 3\n")?;

    // Execute command
    let output = g.command(&["wc", "-l", "/test.txt"])?;
    assert!(output.contains("3"));

    // Execute shell command
    let output = g.sh("ls / | wc -l")?;
    let count: i32 = output.trim().parse()?;
    assert!(count > 0);

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_multiple_partitions() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_multi_part.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 500 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create GPT partition table
    g.part_init("/dev/sda", "gpt")?;

    // Create multiple partitions
    g.part_add("/dev/sda", "primary", 2048, 206847)?; // ~100MB
    g.part_add("/dev/sda", "primary", 206848, 411647)?; // ~100MB
    g.part_add("/dev/sda", "primary", 411648, 616447)?; // ~100MB

    // List partitions
    let partitions = g.list_partitions()?;
    assert_eq!(partitions.len(), 3);

    // Create different filesystems on each partition
    g.mkfs("ext4", "/dev/sda1")?;
    g.mkfs("xfs", "/dev/sda2")?;
    g.mkfs("ext4", "/dev/sda3")?;

    // Verify filesystem types
    assert_eq!(g.vfs_type("/dev/sda1")?, "ext4");
    assert_eq!(g.vfs_type("/dev/sda2")?, "xfs");
    assert_eq!(g.vfs_type("/dev/sda3")?, "ext4");

    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}

#[test]
fn test_copy_operations() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_copy.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Setup filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create source file
    g.write("/source.txt", b"Source content")?;

    // Test simple copy
    g.cp("/source.txt", "/dest.txt")?;
    assert!(g.exists("/dest.txt")?);

    let content = g.cat("/dest.txt")?;
    assert_eq!(content, "Source content");

    // Test directory copy
    g.mkdir_p("/srcdir")?;
    g.write("/srcdir/file1.txt", b"File 1")?;
    g.write("/srcdir/file2.txt", b"File 2")?;

    g.cp_r("/srcdir", "/destdir")?;
    assert!(g.exists("/destdir/file1.txt")?);
    assert!(g.exists("/destdir/file2.txt")?);

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);
    Ok(())
}
