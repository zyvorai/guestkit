// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: System information and configuration
//!
//! This demonstrates system configuration operations.
//!
//! ⚠️  REQUIRES: sudo/root permissions for mounting
//!
//! Usage:
//!   sudo cargo run --example system_info /path/to/disk.qcow2

use guestkit::guestfs::Guestfs;
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image>", args[0]);
        eprintln!("Example: sudo {} /path/to/vm.qcow2", args[0]);
        eprintln!("\n⚠️  Requires sudo/root for mounting");
        std::process::exit(1);
    }

    let disk_path = &args[1];

    println!("=== GuestKit System Information ===");
    println!("Image: {}\n", disk_path);

    // Create GuestFS handle using builder pattern
    println!("Creating guest and launching...");
    let mut g = Guestfs::builder()
        .add_drive_ro(disk_path)
        .verbose(true)
        .build_and_launch()?;

    // Find and mount OS root
    println!("\nFinding OS root...");
    let roots = g.inspect_os()?;

    if roots.is_empty() {
        eprintln!("Error: No OS detected");
        return Ok(());
    }

    let root = &roots[0];
    let mountpoints = g.inspect_get_mountpoints(root)?;

    // Mount root
    if let Some((_, device)) = mountpoints.iter().find(|(mp, _)| mp.as_str() == "/") {
        println!("Mounting {} as /...", device);
        g.mount_ro(device, "/")?;
    } else {
        eprintln!("Error: No root filesystem");
        return Ok(());
    }

    // System Information
    println!("\n--- System Information ---");

    // Hostname
    match g.get_hostname() {
        Ok(hostname) => println!("Hostname: {}", hostname),
        Err(e) => println!("Hostname: Error - {}", e),
    }

    // Timezone
    match g.get_timezone() {
        Ok(tz) => println!("Timezone: {}", tz),
        Err(e) => println!("Timezone: Error - {}", e),
    }

    // Locale
    match g.get_locale() {
        Ok(locale) => println!("Locale: {}", locale),
        Err(e) => println!("Locale: Error - {}", e),
    }

    // Kernel version
    match g.get_kernel_version() {
        Ok(kernel) => println!("Kernel: {}", kernel),
        Err(e) => println!("Kernel: Error - {}", e),
    }

    // Machine ID
    match g.get_machine_id() {
        Ok(machine_id) => println!("Machine ID: {}", machine_id),
        Err(e) => println!("Machine ID: Error - {}", e),
    }

    // OS Information
    println!("\n--- OS Details ---");
    match g.get_osinfo() {
        Ok(osinfo) => {
            println!("{}", osinfo);
        }
        Err(e) => println!("Error: {}", e),
    }

    // Network Information
    println!("\n--- Network Configuration ---");

    match g.list_network_interfaces() {
        Ok(interfaces) => {
            println!("Network Interfaces:");
            for iface in &interfaces {
                println!("  - {}", iface);

                // Try to get configuration
                match g.get_network_config(iface) {
                    Ok(config) => {
                        if !config.trim().is_empty() {
                            println!("    Configuration:");
                            for line in config.lines().take(5) {
                                println!("      {}", line);
                            }
                        }
                    }
                    Err(_) => println!("    (No configuration found)"),
                }
            }
        }
        Err(e) => println!("Error listing interfaces: {}", e),
    }

    // DNS servers
    match g.get_dns() {
        Ok(dns) => {
            if !dns.is_empty() {
                println!("\nDNS Servers:");
                for server in dns {
                    println!("  {}", server);
                }
            }
        }
        Err(e) => println!("\nDNS: Error - {}", e),
    }

    // Package Management
    println!("\n--- Installed Packages ---");

    // Try Debian packages
    if let Ok(packages) = g.dpkg_list() {
        if !packages.is_empty() {
            println!("Debian packages: {} installed", packages.len());
            println!("Sample packages:");
            for pkg in packages.iter().take(10) {
                println!("  - {}", pkg);
            }
            if packages.len() > 10 {
                println!("  ... and {} more", packages.len() - 10);
            }
        }
    }

    // Try RPM packages
    if let Ok(packages) = g.rpm_list() {
        if !packages.is_empty() {
            println!("RPM packages: {} installed", packages.len());
            println!("Sample packages:");
            for pkg in packages.iter().take(10) {
                println!("  - {}", pkg);
            }
            if packages.len() > 10 {
                println!("  ... and {} more", packages.len() - 10);
            }
        }
    }

    // Users and Groups
    println!("\n--- Users and Groups ---");

    match g.list_users() {
        Ok(users) => {
            println!("Users ({} total):", users.len());
            for user in users.iter().take(10) {
                println!("  - {}", user);
            }
            if users.len() > 10 {
                println!("  ... and {} more", users.len() - 10);
            }
        }
        Err(e) => println!("Error listing users: {}", e),
    }

    match g.list_groups() {
        Ok(groups) => {
            println!("\nGroups ({} total):", groups.len());
            for group in groups.iter().take(10) {
                println!("  - {}", group);
            }
            if groups.len() > 10 {
                println!("  ... and {} more", groups.len() - 10);
            }
        }
        Err(e) => println!("Error listing groups: {}", e),
    }

    // Systemd units
    println!("\n--- Systemd Units ---");
    match g.list_systemd_units() {
        Ok(units) => {
            println!("Found {} systemd units", units.len());
            println!("Sample units:");
            for unit in units.iter().take(10) {
                println!("  - {}", unit);
            }
            if units.len() > 10 {
                println!("  ... and {} more", units.len() - 10);
            }
        }
        Err(e) => println!("Error: {}", e),
    }

    // Filesystem statistics
    println!("\n--- Filesystem Statistics ---");
    match g.df_h() {
        Ok(df_output) => {
            println!("{}", df_output);
        }
        Err(e) => println!("Error: {}", e),
    }

    // Cleanup
    println!("\n--- Cleaning Up ---");
    g.umount_all()?;
    g.shutdown()?;

    println!("\n✓ Complete!");

    Ok(())
}
