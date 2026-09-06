// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: OS inspection with type-safe enums
//!
//! This example shows how to use the new type-safe OS detection enums
//! instead of error-prone string comparisons.
//!
//! ⚠️  REQUIRES: sudo/root permissions
//!
//! Usage:
//!   sudo cargo run --example inspect_os_typed <disk-image>

use guestkit::guestfs::{Distro, Guestfs, OsType, PackageManager};
use std::env;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <disk-image>", args[0]);
        eprintln!("Example: sudo {} /tmp/ubuntu-efi-test.img", args[0]);
        std::process::exit(1);
    }

    let disk_path = &args[1];

    println!("=== Type-Safe OS Inspection ===");
    println!("Image: {}\n", disk_path);

    // Create guest with builder
    let mut guest = Guestfs::builder()
        .add_drive_ro(disk_path)
        .build_and_launch()?;

    // Inspect operating systems
    println!("Detecting operating systems...\n");
    let roots = guest.inspect_os()?;

    if roots.is_empty() {
        println!("No operating systems detected.");
        return Ok(());
    }

    println!("Found {} operating system(s):\n", roots.len());

    for (i, root) in roots.iter().enumerate() {
        println!("=== OS #{} ===", i + 1);
        println!("Root device: {}", root);

        // Get OS type using type-safe enum
        let os_type_str = guest.inspect_get_type(root)?;
        let os_type = OsType::from_str(&os_type_str);

        println!("Type: {} ({})", os_type, os_type_str);

        // Handle different OS types with pattern matching
        match os_type {
            OsType::Linux => {
                handle_linux_os(&mut guest, root)?;
            }
            OsType::Windows => {
                handle_windows_os(&mut guest, root)?;
            }
            OsType::FreeBsd | OsType::NetBsd | OsType::OpenBsd => {
                handle_bsd_os(&mut guest, root)?;
            }
            OsType::Unknown => {
                println!("  Unknown OS type - limited information available");
            }
            _ => {
                println!("  OS type: {:?}", os_type);
            }
        }

        println!();
    }

    guest.shutdown()?;
    println!("✓ Inspection complete!");

    Ok(())
}

fn handle_linux_os(guest: &mut Guestfs, root: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Linux Distribution ---");

    // Get distribution using type-safe enum
    let distro_str = guest.inspect_get_distro(root)?;
    let distro = Distro::from_str(&distro_str);

    println!("Distribution: {} ({})", distro, distro_str);

    // Get version info
    if let Ok(major) = guest.inspect_get_major_version(root) {
        let minor = guest.inspect_get_minor_version(root).unwrap_or(0);
        println!("Version: {}.{}", major, minor);
    }

    // Use pattern matching for distribution-specific logic
    match distro {
        Distro::Ubuntu | Distro::Debian => {
            println!("Distribution family: Debian-based");
            println!("Package manager: dpkg/apt");

            if let Some(pkg_mgr) = distro.package_manager() {
                println!("Package format: {}", pkg_mgr.as_str());
                assert_eq!(pkg_mgr, PackageManager::Dpkg);
            }
        }

        Distro::Fedora | Distro::Rhel | Distro::CentOs => {
            println!("Distribution family: Red Hat-based");
            println!("Package manager: rpm/yum/dnf");

            if let Some(pkg_mgr) = distro.package_manager() {
                println!("Package format: {}", pkg_mgr.as_str());
                assert_eq!(pkg_mgr, PackageManager::Rpm);
            }
        }

        Distro::Archlinux => {
            println!("Distribution family: Independent");
            println!("Package manager: pacman");

            if let Some(pkg_mgr) = distro.package_manager() {
                println!("Package format: {}", pkg_mgr.as_str());
                assert_eq!(pkg_mgr, PackageManager::Pacman);
            }
        }

        Distro::Gentoo => {
            println!("Distribution family: Source-based");
            println!("Package manager: portage");
        }

        Distro::Alpine => {
            println!("Distribution family: Independent (musl-based)");
            println!("Package manager: apk");
        }

        Distro::Opensuse | Distro::Suse => {
            println!("Distribution family: SUSE");
            println!("Package manager: rpm/zypper");
        }

        Distro::Nixos => {
            println!("Distribution family: Functional");
            println!("Package manager: nix");
        }

        Distro::Unknown => {
            println!("Distribution: Unknown or not recognized");
        }

        _ => {
            println!("Distribution: Other Linux variant");
        }
    }

    // Product name
    if let Ok(product) = guest.inspect_get_product_name(root) {
        println!("Product name: {}", product);
    }

    // Hostname
    if let Ok(hostname) = guest.inspect_get_hostname(root) {
        println!("Hostname: {}", hostname);
    }

    // Mountpoints
    if let Ok(mountpoints) = guest.inspect_get_mountpoints(root) {
        println!("\nMountpoints:");
        for (mount, device) in mountpoints {
            println!("  {} -> {}", mount, device);
        }
    }

    Ok(())
}

fn handle_windows_os(guest: &mut Guestfs, root: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- Windows OS ---");

    // Windows version
    if let Ok(major) = guest.inspect_get_major_version(root) {
        let minor = guest.inspect_get_minor_version(root).unwrap_or(0);
        println!("Windows version: {}.{}", major, minor);

        // Decode Windows version
        let version_name = match (major, minor) {
            (10, 0) => "Windows 10/11/Server 2016-2022",
            (6, 3) => "Windows 8.1/Server 2012 R2",
            (6, 2) => "Windows 8/Server 2012",
            (6, 1) => "Windows 7/Server 2008 R2",
            (6, 0) => "Windows Vista/Server 2008",
            (5, 2) => "Windows XP x64/Server 2003",
            (5, 1) => "Windows XP",
            _ => "Unknown Windows version",
        };
        println!("Likely: {}", version_name);
    }

    // Product name
    if let Ok(product) = guest.inspect_get_product_name(root) {
        println!("Product: {}", product);
    }

    // System root
    if let Ok(sysroot) = guest.inspect_get_windows_systemroot(root) {
        println!("System root: {}", sysroot);
    }

    // Current control set
    if let Ok(ccs) = guest.inspect_get_windows_current_control_set(root) {
        println!("Current control set: {}", ccs);
    }

    Ok(())
}

fn handle_bsd_os(guest: &mut Guestfs, root: &str) -> Result<(), Box<dyn std::error::Error>> {
    println!("\n--- BSD OS ---");

    if let Ok(product) = guest.inspect_get_product_name(root) {
        println!("Product: {}", product);
    }

    if let Ok(hostname) = guest.inspect_get_hostname(root) {
        println!("Hostname: {}", hostname);
    }

    Ok(())
}
