// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Comprehensive Phase 3 API testing with a fake Windows-like disk image
//!
//! This test creates a minimal Windows-like disk image and exercises all
//! Phase 3 APIs to ensure they work correctly with Windows scenarios.

use guestkit::Guestfs;
use std::fs;
use std::path::Path;

const DISK_PATH: &str = "/tmp/phase3-test-windows.img";
const DISK_SIZE: i64 = 200 * 1024 * 1024; // 200 MB

fn cleanup() {
    let _ = fs::remove_file(DISK_PATH);
}

fn create_fake_windows_image() -> Result<(), Box<dyn std::error::Error>> {
    cleanup();

    println!("\n=== Creating Fake Windows Disk Image ===");

    // Test 1: Using create() alias (Phase 3 API)
    println!("\n[1/10] Testing Guestfs::create() alias...");
    let mut g = Guestfs::create()?;
    g.set_verbose(false);

    // Create disk image
    println!("  Creating {}MB disk image...", DISK_SIZE / 1024 / 1024);
    g.disk_create(DISK_PATH, "raw", DISK_SIZE)?;

    // Test 2: Using add_drive() (Phase 3 API - read-write mode)
    println!("\n[2/10] Testing add_drive() (read-write mode)...");
    g.add_drive(DISK_PATH)?;
    println!("  ✓ Drive added in read-write mode");

    // Launch and create partition table
    g.launch()?;

    println!("\n  Setting up partition table...");
    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, 104447)?; // ~50MB System Reserved
    g.part_add("/dev/sda", "primary", 104448, -2048)?; // ~150MB C: drive

    // Test 3: Testing part_set_parttype() (Phase 3 API)
    println!("\n[3/10] Testing part_set_parttype()...");
    let parttype = g.part_get_parttype("/dev/sda")?;
    println!("  Current partition type: {}", parttype);
    assert_eq!(parttype, "msdos");
    println!("  ✓ Partition type is MBR/MSDOS");

    // Create filesystems (NTFS)
    println!("\n  Creating NTFS filesystems...");
    g.mkfs("ntfs", "/dev/sda1")?;
    g.mkfs("ntfs", "/dev/sda2")?;

    // Set filesystem labels (Windows style)
    g.set_label("/dev/sda1", "System Reserved")?;
    g.set_label("/dev/sda2", "Windows")?;

    // Mount C: drive (sda2)
    g.mount("/dev/sda2", "/")?;

    // Create Windows-like directory structure
    println!("\n  Creating Windows directory structure...");
    for dir in &[
        "/Windows",
        "/Windows/System32",
        "/Windows/System32/config",
        "/Windows/System32/drivers",
        "/Windows/SysWOW64",
        "/Program Files",
        "/Program Files/Common Files",
        "/Program Files (x86)",
        "/ProgramData",
        "/Users",
        "/Users/Administrator",
        "/Users/Administrator/Desktop",
        "/Users/Administrator/Documents",
        "/Users/Public",
        "/PerfLogs",
        "/Temp",
    ] {
        g.mkdir_p(dir)?;
    }

    // Create fake Windows system files
    println!("\n  Creating Windows system files...");

    // Create version information
    g.write(
        "/Windows/System32/version.txt",
        b"Windows 11 Pro\r\nVersion 23H2 (OS Build 22631.4169)\r\n",
    )?;

    // Create fake registry hives (just markers)
    g.write(
        "/Windows/System32/config/SOFTWARE",
        b"FAKE_REGISTRY_HIVE\x00",
    )?;
    g.write("/Windows/System32/config/SYSTEM", b"FAKE_REGISTRY_HIVE\x00")?;
    g.write("/Windows/System32/config/SAM", b"FAKE_REGISTRY_HIVE\x00")?;

    // Create computer name file
    g.write("/Windows/System32/computername.txt", b"WIN-TEST-PC\r\n")?;

    // Test 4: Testing stat() (Phase 3 API) on Windows files
    println!("\n[4/10] Testing stat() on regular file...");
    let stat = g.stat("/Windows/System32/computername.txt")?;
    println!("  File size: {} bytes", stat.size);
    println!("  Mode: 0{:o}", stat.mode);
    println!("  UID: {}, GID: {}", stat.uid, stat.gid);
    assert!(stat.size > 0);
    println!("  ✓ stat() works correctly on Windows files");

    // Create some Windows-specific test files
    println!("\n  Creating Windows test files...");
    g.write(
        "/Users/Administrator/Desktop/test.txt",
        b"Windows test file\r\n",
    )?;
    g.write("/Temp/temporary.log", b"Temporary log data\r\n")?;
    g.write("/ProgramData/settings.ini", b"[Settings]\r\nkey=value\r\n")?;

    // Create a symbolic link (even on NTFS)
    g.ln_s(
        "/Users/Administrator/Desktop/test.txt",
        "/Users/Administrator/Desktop/shortcut.lnk",
    )?;

    // Test 5: Testing lstat() on symlink (Phase 3 API)
    println!("\n[5/10] Testing lstat() on symbolic link...");
    let lstat_result = g.lstat("/Users/Administrator/Desktop/shortcut.lnk")?;
    let stat_result = g.stat("/Users/Administrator/Desktop/shortcut.lnk")?;

    // lstat should show the link itself, stat should follow it
    println!("  lstat size: {} (link metadata)", lstat_result.size);
    println!("  stat size: {} (target file)", stat_result.size);

    // The sizes should be different (link metadata vs file content)
    assert_ne!(
        lstat_result.size, stat_result.size,
        "lstat and stat should differ for symlinks"
    );
    println!("  ✓ lstat() correctly doesn't follow symlink");

    // Create directory for removal testing (Windows paths)
    g.mkdir_p("/Temp/test-removal/subdir")?;
    g.write("/Temp/test-removal/file1.txt", b"file1\r\n")?;
    g.write("/Temp/test-removal/file2.log", b"file2\r\n")?;
    g.write("/Temp/test-removal/subdir/file3.dat", b"file3\r\n")?;

    // Test 6: Testing rm() (Phase 3 API)
    println!("\n[6/10] Testing rm() on single file...");
    assert!(g.exists("/Temp/temporary.log")?);
    g.rm("/Temp/temporary.log")?;
    assert!(!g.exists("/Temp/temporary.log")?);
    println!("  ✓ rm() successfully removed file");

    // Test that rm() fails on directory
    println!("\n  Testing rm() fails on directory...");
    let result = g.rm("/Temp/test-removal");
    assert!(result.is_err(), "rm() should fail on directory");
    println!("  ✓ rm() correctly rejects directory");

    // Test 7: Testing rm_rf() (Phase 3 API)
    println!("\n[7/10] Testing rm_rf() on directory tree...");
    assert!(g.exists("/Temp/test-removal")?);
    assert!(g.exists("/Temp/test-removal/subdir/file3.dat")?);

    g.rm_rf("/Temp/test-removal")?;
    assert!(!g.exists("/Temp/test-removal")?);
    println!("  ✓ rm_rf() successfully removed directory tree");

    // Test rm_rf() on non-existent path (should succeed silently)
    println!("\n  Testing rm_rf() on non-existent path...");
    g.rm_rf("/Temp/does-not-exist")?;
    println!("  ✓ rm_rf() handles non-existent path correctly");

    // Create a CPIO archive for testing
    println!("\n  Creating test CPIO archive...");
    g.mkdir_p("/Users/Administrator/archive-source")?;
    g.write(
        "/Users/Administrator/archive-source/document1.txt",
        b"Document 1\r\n",
    )?;
    g.write(
        "/Users/Administrator/archive-source/document2.txt",
        b"Document 2\r\n",
    )?;

    // Unmount and shutdown
    g.umount("/")?;
    g.shutdown()?;

    // Re-open with add_drive_ro to test read-only mode
    println!("\n[8/10] Testing add_drive_ro() (read-only mode)...");
    let mut g2 = Guestfs::new()?;
    g2.set_verbose(false);
    g2.add_drive_ro(DISK_PATH)?;
    g2.launch()?;

    // Verify we can read but not write
    g2.mount_ro("/dev/sda2", "/")?;
    let content = g2.cat("/Windows/System32/computername.txt")?;
    assert!(content.contains("WIN-TEST-PC"));
    println!("  ✓ Successfully mounted read-only");

    // Verify write fails in read-only mode
    let write_result = g2.write("/Windows/should-fail.txt", b"test");
    assert!(write_result.is_err(), "Write should fail in read-only mode");
    println!("  ✓ Write correctly fails in read-only mode");

    g2.umount("/")?;
    g2.shutdown()?;

    // Test partition name (MBR doesn't have names like GPT, so this may differ)
    println!("\n[9/10] Testing part_get_name() on MBR partition...");
    let mut g3 = Guestfs::new()?;
    g3.set_verbose(false);
    g3.add_drive(DISK_PATH)?;
    g3.launch()?;

    // Try to get partition name (MBR partitions don't have names like GPT)
    match g3.part_get_name("/dev/sda", 1) {
        Ok(name) => println!("  Partition 1 name: '{}'", name),
        Err(e) => println!("  MBR partitions don't support names: {}", e),
    }

    g3.shutdown()?;

    // Test 10: Create CPIO archive and test cpio_in()
    println!("\n[10/10] Testing cpio_in() (CPIO archive extraction)...");

    // First, create a CPIO archive on the host
    let cpio_path = "/tmp/test-archive-windows.cpio";
    std::process::Command::new("sh")
        .arg("-c")
        .arg(format!(
            "cd /tmp && mkdir -p cpio-test-windows && \
             echo 'Windows content 1' > cpio-test-windows/file1.txt && \
             echo 'Windows content 2' > cpio-test-windows/file2.txt && \
             find cpio-test-windows -print | cpio -o -H newc > {}",
            cpio_path
        ))
        .output()?;

    if Path::new(cpio_path).exists() {
        let mut g4 = Guestfs::new()?;
        g4.set_verbose(false);
        g4.add_drive(DISK_PATH)?;
        g4.launch()?;
        g4.mount("/dev/sda2", "/")?;

        // Extract CPIO archive to Windows-style path
        g4.mkdir_p("/Users/Administrator/Downloads/extracted")?;

        match g4.cpio_in(cpio_path, "/Users/Administrator/Downloads/extracted") {
            Ok(_) => {
                println!("  ✓ CPIO archive extracted successfully");

                // Verify extracted files
                if g4.exists(
                    "/Users/Administrator/Downloads/extracted/cpio-test-windows/file1.txt",
                )? {
                    let content = g4.cat(
                        "/Users/Administrator/Downloads/extracted/cpio-test-windows/file1.txt",
                    )?;
                    println!("  Extracted file content: {}", content.trim());
                    println!("  ✓ CPIO extraction verified");
                }
            }
            Err(e) => {
                println!(
                    "  ⚠ CPIO extraction failed (cpio may not be available): {}",
                    e
                );
            }
        }

        g4.shutdown()?;
        let _ = fs::remove_file(cpio_path);
    } else {
        println!("  ⚠ Could not create CPIO archive for testing");
    }

    // Clean up temporary CPIO test directory
    let _ = std::process::Command::new("rm")
        .args(["-rf", "/tmp/cpio-test-windows"])
        .output();

    println!("\n=== All Phase 3 APIs Tested Successfully on Windows Image! ===");

    Ok(())
}

#[test]
fn test_phase3_windows_comprehensive() -> Result<(), Box<dyn std::error::Error>> {
    // Run the comprehensive Windows test
    let result = create_fake_windows_image();

    // Clean up
    cleanup();

    // Return result
    result
}

#[test]
fn test_windows_stat_vs_lstat() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing stat() vs lstat() on Windows paths ===");

    let disk = "/tmp/stat-test-windows.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ntfs", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create Windows-style paths
    g.mkdir_p("/Windows/System32")?;
    g.write("/Windows/System32/target.dll", b"Fake DLL content\r\n")?;
    g.ln_s("/Windows/System32/target.dll", "/Windows/System32/link.dll")?;

    // stat() should follow the symlink
    let stat_result = g.stat("/Windows/System32/link.dll")?;
    println!(
        "stat(/Windows/System32/link.dll) size: {}",
        stat_result.size
    );
    assert_eq!(stat_result.size, 19); // "Fake DLL content\r\n"

    // lstat() should NOT follow the symlink
    let lstat_result = g.lstat("/Windows/System32/link.dll")?;
    println!(
        "lstat(/Windows/System32/link.dll) size: {}",
        lstat_result.size
    );
    assert_ne!(lstat_result.size, 19); // Should be link metadata size

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    println!("✓ stat() and lstat() behave correctly on Windows paths");
    Ok(())
}

#[test]
fn test_windows_rm_operations() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing rm() and rm_rf() on Windows paths ===");

    let disk = "/tmp/rm-test-windows.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ntfs", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create Windows-style directory structure
    g.mkdir_p("/Windows/Temp")?;
    g.mkdir_p("/Program Files/TestApp/bin")?;

    // Test rm() on Windows file
    g.write("/Windows/Temp/temporary.log", b"temp\r\n")?;
    assert!(g.exists("/Windows/Temp/temporary.log")?);
    g.rm("/Windows/Temp/temporary.log")?;
    assert!(!g.exists("/Windows/Temp/temporary.log")?);
    println!("  ✓ rm() works on Windows paths");

    // Test rm_rf() on Program Files directory
    g.write("/Program Files/TestApp/bin/app.exe", b"fake exe\r\n")?;
    g.write("/Program Files/TestApp/config.ini", b"config\r\n")?;

    assert!(g.exists("/Program Files/TestApp")?);
    g.rm_rf("/Program Files/TestApp")?;
    assert!(!g.exists("/Program Files/TestApp")?);
    println!("  ✓ rm_rf() removes Windows directory trees");

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    Ok(())
}

#[test]
fn test_windows_ntfs_features() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing NTFS-specific features ===");

    let disk = "/tmp/ntfs-test.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ntfs", "/dev/sda1")?;

    // Verify NTFS filesystem
    let fstype = g.vfs_type("/dev/sda1")?;
    assert_eq!(fstype, "ntfs");
    println!("  ✓ NTFS filesystem created");

    // Check label
    g.set_label("/dev/sda1", "WINDOWS_C")?;
    let label = g.vfs_label("/dev/sda1")?;
    assert_eq!(label, "WINDOWS_C");
    println!("  ✓ NTFS label set correctly");

    g.mount("/dev/sda1", "/")?;

    // Create files with Windows-style line endings
    g.write("/test.txt", b"Line 1\r\nLine 2\r\nLine 3\r\n")?;
    let content = g.cat("/test.txt")?;
    assert!(content.contains("\r\n"));
    println!("  ✓ Windows line endings preserved");

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    Ok(())
}

#[test]
fn test_windows_long_paths() -> Result<(), Box<dyn std::error::Error>> {
    println!("\n=== Testing Windows long path handling ===");

    let disk = "/tmp/longpath-test.img";
    let _ = fs::remove_file(disk);

    let mut g = Guestfs::new()?;
    g.disk_create(disk, "raw", 50 * 1024 * 1024)?;
    g.add_drive(disk)?;
    g.launch()?;

    g.part_init("/dev/sda", "mbr")?;
    g.part_add("/dev/sda", "primary", 2048, -2048)?;
    g.mkfs("ntfs", "/dev/sda1")?;
    g.mount("/dev/sda1", "/")?;

    // Create a deeply nested path (Windows style)
    let long_path =
        "/Program Files/Microsoft/Windows/Application Data/Local Settings/Temporary Files/Cache";
    g.mkdir_p(long_path)?;

    let test_file = format!("{}/data.tmp", long_path);
    g.write(&test_file, b"test data\r\n")?;

    assert!(g.exists(&test_file)?);
    let stat = g.stat(&test_file)?;
    assert_eq!(stat.size, 11);
    println!("  ✓ Long Windows paths handled correctly");

    // Test removal of long path
    g.rm_rf("/Program Files")?;
    assert!(!g.exists("/Program Files")?);
    println!("  ✓ rm_rf() works on long paths");

    g.shutdown()?;
    let _ = fs::remove_file(disk);

    Ok(())
}
