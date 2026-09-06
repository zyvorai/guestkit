// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Mount a VM and explore files
//!
//! This demonstrates mounting filesystems and reading files.
//!
//! ⚠️  REQUIRES: sudo/root permissions for mounting
//!
//! Usage:
//!   sudo cargo run --example mount_and_explore /path/to/disk.qcow2

use guestkit::guestfs::Guestfs;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image>", args[0]);
        eprintln!("Example: sudo {} /path/to/vm.qcow2", args[0]);
        eprintln!("\n⚠️  This example requires sudo/root permissions for mounting");
        std::process::exit(1);
    }

    let disk_path = &args[1];

    println!("=== GuestKit Mount & Explore ===");
    println!("Image: {}\n", disk_path);

    // Create GuestFS handle
    let mut g = Guestfs::new()?;
    g.set_verbose(true);

    // Add disk and launch
    println!("Adding disk and launching...");
    g.add_drive_ro(disk_path)?;
    g.launch()?;

    // Find OS root
    println!("\nFinding OS root...");
    let roots = g.inspect_os()?;

    if roots.is_empty() {
        eprintln!("Error: No OS detected in disk image");
        return Ok(());
    }

    let root = &roots[0];
    println!(
        "Found OS: {} {}.{}",
        g.inspect_get_distro(root)?,
        g.inspect_get_major_version(root)?,
        g.inspect_get_minor_version(root)?
    );

    // Get mount points
    let mountpoints = g.inspect_get_mountpoints(root)?;

    // Mount root filesystem
    if let Some((_, device)) = mountpoints.iter().find(|(mp, _)| mp.as_str() == "/") {
        println!("\nMounting {} as /...", device);
        g.mount_ro(device, "/")?;
        println!("✓ Mounted successfully");
    } else {
        eprintln!("Error: No root filesystem found");
        return Ok(());
    }

    // Explore filesystem
    println!("\n--- Exploring Filesystem ---");

    // List /etc directory
    println!("\nContents of /etc:");
    if let Ok(entries) = g.ls("/etc") {
        for (i, entry) in entries.iter().take(20).enumerate() {
            println!("  {}", entry);
            if i == 19 && entries.len() > 20 {
                println!("  ... and {} more files", entries.len() - 20);
            }
        }
    }

    // Read /etc/hostname
    println!("\n/etc/hostname:");
    if g.exists("/etc/hostname")? && g.is_file("/etc/hostname")? {
        match g.cat("/etc/hostname") {
            Ok(content) => println!("  {}", content.trim()),
            Err(e) => println!("  Error reading: {}", e),
        }
    } else {
        println!("  File not found");
    }

    // Read /etc/os-release
    println!("\n/etc/os-release:");
    if g.exists("/etc/os-release")? && g.is_file("/etc/os-release")? {
        match g.read_lines("/etc/os-release") {
            Ok(lines) => {
                for line in lines.iter().take(10) {
                    if !line.trim().is_empty() && !line.starts_with('#') {
                        println!("  {}", line);
                    }
                }
            }
            Err(e) => println!("  Error reading: {}", e),
        }
    } else {
        println!("  File not found");
    }

    // Check for /root directory
    println!("\n/root directory:");
    if g.exists("/root")? && g.is_dir("/root")? {
        println!("  ✓ Exists (is directory)");
        if let Ok(entries) = g.ls("/root") {
            println!("  Contains {} items", entries.len());
        }
    } else {
        println!("  Not found or not a directory");
    }

    // Check for common directories
    println!("\nCommon directories:");
    for dir in &["/bin", "/usr", "/var", "/home", "/tmp"] {
        let status = if g.exists(dir)? {
            if g.is_dir(dir)? {
                "✓ exists (directory)"
            } else {
                "exists (file)"
            }
        } else {
            "not found"
        };
        println!("  {}: {}", dir, status);
    }

    // Unmount and cleanup
    println!("\n--- Cleaning Up ---");
    g.umount_all()?;
    g.shutdown()?;

    println!("\n✓ Complete!");

    Ok(())
}
