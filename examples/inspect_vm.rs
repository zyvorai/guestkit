// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Inspect a VM disk image
//!
//! This demonstrates OS detection and inspection capabilities.
//!
//! Usage:
//!   cargo run --example inspect_vm /path/to/disk.qcow2

use guestkit::guestfs::Guestfs;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image>", args[0]);
        eprintln!("Example: {} /path/to/vm.qcow2", args[0]);
        std::process::exit(1);
    }

    let disk_path = &args[1];

    println!("=== GuestKit VM Inspector ===");
    println!("Analyzing: {}\n", disk_path);

    // Create GuestFS handle
    let mut g = Guestfs::new()?;

    // Enable verbose output
    g.set_verbose(true);

    // Add disk image (read-only)
    println!("Adding disk image...");
    g.add_drive_ro(disk_path)?;

    // Launch (analyzes disk structure)
    println!("Launching and analyzing disk...");
    g.launch()?;

    // List devices
    println!("\n--- Block Devices ---");
    let devices = g.list_devices()?;
    for device in &devices {
        println!("Device: {}", device);
        let size = g.blockdev_getsize64(device)?;
        println!("  Size: {} bytes ({:.2} GB)", size, size as f64 / 1e9);
    }

    // List partitions
    println!("\n--- Partitions ---");
    let partitions = g.list_partitions()?;
    for partition in &partitions {
        println!("Partition: {}", partition);

        // Get partition details
        if let Ok(part_list) = g.part_list("/dev/sda") {
            let part_num = g.part_to_partnum(partition)?;
            if let Some(p) = part_list.iter().find(|p| p.part_num == part_num) {
                println!("  Start: {} bytes", p.part_start);
                println!(
                    "  Size: {} bytes ({:.2} GB)",
                    p.part_size,
                    p.part_size as f64 / 1e9
                );
            }
        }
    }

    // List filesystems
    println!("\n--- Filesystems ---");
    let filesystems = g.list_filesystems()?;
    for (device, fstype) in &filesystems {
        println!("Filesystem: {} ({})", device, fstype);

        if fstype != "unknown" && fstype != "swap" {
            if let Ok(label) = g.vfs_label(device) {
                if !label.is_empty() {
                    println!("  Label: {}", label);
                }
            }
            if let Ok(uuid) = g.vfs_uuid(device) {
                if !uuid.is_empty() {
                    println!("  UUID: {}", uuid);
                }
            }
        }
    }

    // Inspect OS
    println!("\n--- Operating System Detection ---");
    let roots = g.inspect_os()?;

    if roots.is_empty() {
        println!("No operating system detected");
    } else {
        for root in &roots {
            println!("OS Root: {}", root);
            println!("  Type: {}", g.inspect_get_type(root)?);
            println!("  Distro: {}", g.inspect_get_distro(root)?);
            println!("  Product: {}", g.inspect_get_product_name(root)?);
            println!("  Arch: {}", g.inspect_get_arch(root)?);
            println!(
                "  Version: {}.{}",
                g.inspect_get_major_version(root)?,
                g.inspect_get_minor_version(root)?
            );
            println!("  Hostname: {}", g.inspect_get_hostname(root)?);
            println!("  Package Format: {}", g.inspect_get_package_format(root)?);

            // Get mount points
            println!("  Mount Points:");
            let mountpoints = g.inspect_get_mountpoints(root)?;
            for (mp, dev) in mountpoints {
                println!("    {} -> {}", mp, dev);
            }

            // List applications (first 10)
            let apps = g.inspect_list_applications(root)?;
            if !apps.is_empty() {
                println!("  Applications: {} installed", apps.len());
                println!("  Sample applications:");
                for app in apps.iter().take(10) {
                    println!("    - {} {} ({})", app.name, app.version, app.release);
                }
                if apps.len() > 10 {
                    println!("    ... and {} more", apps.len() - 10);
                }
            }
        }
    }

    // Cleanup
    println!("\n--- Shutting Down ---");
    g.shutdown()?;

    println!("\nInspection complete!");

    Ok(())
}
