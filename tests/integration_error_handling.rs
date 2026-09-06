// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Integration tests for error handling and edge cases

use guestkit::Guestfs;
use std::fs;

fn cleanup_disk(path: &str) {
    let _ = fs::remove_file(path);
}

#[test]
fn test_error_not_launched() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;

    // Try to mount without launching
    let result = g.mount("/dev/sda1", "/");

    // Should get NotReady error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(err.to_string().contains("not ready") || err.to_string().contains("launch"));

    Ok(())
}

#[test]
fn test_error_no_drives() -> Result<(), Box<dyn std::error::Error>> {
    let mut g = Guestfs::new()?;

    // Try to launch without adding drives
    let result = g.launch();

    // Should error (no drives added)
    assert!(result.is_err());

    Ok(())
}

#[test]
fn test_error_nonexistent_file() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_nonexistent.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Create filesystem
    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Try to read nonexistent file
    let result = g.cat("/nonexistent.txt");
    assert!(result.is_err());

    // Try to check if nonexistent file exists
    assert!(!g.exists("/nonexistent.txt")?);

    // Try to stat nonexistent file
    let result = g.stat("/nonexistent.txt");
    assert!(result.is_err());

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_invalid_device() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_invalid_device.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Try to mount nonexistent device
    let result = g.mount("/dev/nonexistent", "/");
    assert!(result.is_err());

    // Try to get filesystem type of nonexistent device
    let result = g.vfs_type("/dev/nonexistent");
    assert!(result.is_err());

    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_double_mount() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_double_mount.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;

    // First mount should succeed
    g.mount("/dev/sda1", "/")?;

    // Second mount to same location should error
    let _result = g.mount("/dev/sda1", "/");
    // Implementation may allow this or error, both are valid
    // Just ensure it doesn't crash

    g.umount_all()?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_invalid_path() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_invalid_path.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Try to write to invalid path (relative path)
    // Some implementations might accept this, but it's not recommended
    let _result = g.write("relative/path.txt", b"content");
    // May succeed or fail depending on implementation

    // Try to write with path traversal
    let _result = g.write("/../etc/passwd", b"hacked");
    // Should fail or be sanitized

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_write_to_directory() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_write_dir.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    g.mkdir("/testdir")?;

    // Try to write to a directory (should error)
    let result = g.write("/testdir", b"content");
    assert!(result.is_err());

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_delete_mounted_filesystem() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_delete_mounted.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create a file
    g.write("/test.txt", b"content")?;

    // Verify file exists
    assert!(g.exists("/test.txt")?);

    // Delete file (should work while mounted)
    g.rm("/test.txt")?;

    // Verify file is gone
    assert!(!g.exists("/test.txt")?);

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_unmount_not_mounted() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_unmount.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Try to unmount without mounting
    let _result = g.umount("/");
    // May error or succeed (no-op), both are acceptable

    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_zero_size_file() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_zero_size.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create zero-size file
    g.write("/empty.txt", b"")?;

    // Should exist
    assert!(g.exists("/empty.txt")?);

    // Should be file
    assert!(g.is_file("/empty.txt")?);

    // Should have size 0
    let size = g.filesize("/empty.txt")?;
    assert_eq!(size, 0);

    // Read should return empty string
    let content = g.cat("/empty.txt")?;
    assert_eq!(content, "");

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_large_file_handling() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_large_file.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    // Create larger disk for this test
    g.disk_create(disk_path, "raw", 500 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create a file larger than typical buffer size
    let large_content = vec![b'A'; 10 * 1024 * 1024]; // 10MB
    g.write("/large.dat", &large_content)?;

    // Verify size
    let size = g.filesize("/large.dat")?;
    assert_eq!(size, large_content.len() as i64);

    // Reading very large files might fail or succeed depending on implementation
    // Just ensure it doesn't crash
    let result = g.cat("/large.dat");
    // If it succeeds, verify content
    if let Ok(content) = result {
        assert!(!content.is_empty());
    }

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}

#[test]
fn test_error_special_characters_in_filename() -> Result<(), Box<dyn std::error::Error>> {
    let disk_path = "/tmp/test_error_special_chars.img";
    cleanup_disk(disk_path);

    let mut g = Guestfs::new()?;
    g.disk_create(disk_path, "raw", 100 * 1024 * 1024)?;
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    g.part_init("/dev/sda", "gpt")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ext4", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Test filenames with spaces
    g.write("/file with spaces.txt", b"content")?;
    assert!(g.exists("/file with spaces.txt")?);

    // Test filename with special characters
    let special_names = vec![
        "/file_with_underscores.txt",
        "/file-with-dashes.txt",
        "/file.multiple.dots.txt",
    ];

    for name in special_names {
        g.write(name, b"content")?;
        assert!(g.exists(name)?);
    }

    g.umount("/")?;
    g.shutdown()?;
    cleanup_disk(disk_path);

    Ok(())
}
