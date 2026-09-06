// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Inspection commands: inspect_image, list_filesystems, list_packages
#![allow(clippy::too_many_arguments)]

use crate::cli::cache::InspectionCache;
use crate::cli::formatters::*;
use crate::cli::profiles::get_profile;
use crate::core::ProgressReporter;
use crate::Guestfs;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::path::{Path, PathBuf};

use super::{
    collect_inspection_data, init_guestfs_ro, print_inspection_report, print_profile_report,
};

/// Inspect a disk image and display OS information
pub fn inspect_image(
    image: &Path,
    verbose: bool,
    debug: bool,
    output_format: Option<OutputFormat>,
    profile: Option<String>,
    export_format: Option<String>,
    export_path: Option<PathBuf>,
    use_cache: bool,
    force_refresh: bool,
) -> Result<()> {
    // Try to get cached result if caching is enabled
    if use_cache && !force_refresh {
        if let Ok(cache) = InspectionCache::new() {
            if let Ok(Some(cached_report)) = cache.get(image) {
                log::debug!("Using cached inspection result");

                // Handle export if requested
                if let (Some(export_fmt), Some(export_out)) = (export_format, export_path) {
                    use crate::cli::exporters::{export_report, ExportFormat};

                    let fmt = ExportFormat::from_str(&export_fmt)?;
                    export_report(&cached_report, fmt, &export_out)?;

                    println!("Report exported to: {}", export_out.display());
                    return Ok(());
                }

                // Handle profile output
                if profile.is_some() {
                    println!("⚠ Cannot use profiles with cached results. Use --cache-refresh to re-inspect.");
                    return Ok(());
                }

                // Print cached result
                print_inspection_report(&cached_report, output_format, verbose)?;
                return Ok(());
            }
        }
    }

    let mut g = Guestfs::new()?;
    g.set_verbose(verbose);
    g.set_debug(debug);

    let progress = ProgressReporter::spinner(&format!("Inspecting: {}", image.display()));

    if verbose {
        eprintln!("[VERBOSE] Adding drive: {}", image.display());
    }
    g.add_drive_ro(image.to_str().ok_or_else(|| {
        anyhow::anyhow!(
            "Disk image path contains invalid UTF-8: {}",
            image.display()
        )
    })?)?;

    progress.set_message("Launching appliance...");
    if verbose {
        eprintln!("[VERBOSE] Launching QEMU appliance...");
    }
    g.launch().context("Failed to launch")?;

    progress.set_message("Scanning disk...");

    // If structured output format is requested, skip text display and collect data directly
    if output_format.is_some() || export_format.is_some() {
        let roots = g.inspect_os()?;
        progress.finish_and_clear();

        if let Some(profile_name) = profile {
            if roots.is_empty() {
                eprintln!("No operating systems found in image");
                g.shutdown()?;
                return Ok(());
            }
            let root = &roots[0];
            if let Some(profile_impl) = get_profile(&profile_name) {
                let report = profile_impl.inspect(&mut g, root)?;
                if let Some(format) = output_format {
                    let _formatter = get_formatter(format, true)?;
                    let output = serde_json::to_string_pretty(&report)?;
                    println!("{}", output);
                } else {
                    print_profile_report(&report);
                }
                g.shutdown()?;
                return Ok(());
            } else {
                eprintln!(
                    "Unknown profile: {}. Available: security, migration, performance",
                    profile_name
                );
                g.shutdown()?;
                return Err(anyhow::anyhow!("Invalid profile"));
            }
        }

        if roots.is_empty() {
            eprintln!("No operating systems found in image");
            g.shutdown()?;
            return Ok(());
        }

        let mut report = collect_inspection_data(&mut g, &roots[0], verbose)?;
        report.image_path = Some(image.to_string_lossy().to_string());
        g.shutdown()?;

        if use_cache {
            if let Ok(cache) = InspectionCache::new() {
                if let Err(e) = cache.store(image, &report) {
                    log::debug!("Failed to cache inspection result: {}", e);
                } else {
                    log::debug!("Cached inspection result");
                }
            }
        }

        if let (Some(export_fmt), Some(export_out)) = (export_format, export_path) {
            use crate::cli::exporters::{export_report, ExportFormat};
            let fmt = ExportFormat::from_str(&export_fmt)?;
            export_report(&report, fmt, &export_out)?;
            eprintln!("Report exported to: {}", export_out.display());
            return Ok(());
        }

        if let Some(format) = output_format {
            let formatter = get_formatter(format, true)?;
            let output = formatter.format(&report)?;
            println!("{}", output);
        }

        return Ok(());
    }

    // Text display path — only reached when no structured output is requested

    // List devices
    if verbose {
        eprintln!("[VERBOSE] Enumerating block devices...");
    }
    println!("\n{}", "💾 Block Devices".truecolor(222, 115, 86).bold());
    println!("{}", "─".repeat(60).bright_black());
    let devices = g.list_devices()?;
    for device in &devices {
        let size = g.blockdev_getsize64(device)?;
        if verbose {
            eprintln!("[VERBOSE] Found device: {} ({} bytes)", device, size);
        }
        println!(
            "  {} {} {} ({:.2} GB)",
            "▪".truecolor(222, 115, 86),
            device.bright_white().bold(),
            format!("{} bytes", size).bright_black(),
            size as f64 / 1e9
        );

        // Additional device information
        if let Ok(ro) = g.blockdev_getro(device) {
            if ro {
                println!("    {} Read-only: {}", "•".bright_black(), "yes".red());
            } else {
                println!("    {} Read-only: {}", "•".bright_black(), "no".green());
            }
        }
        if let Ok(ss) = g.blockdev_getss(device) {
            println!(
                "    {} Sector size: {}",
                "•".bright_black(),
                format!("{} bytes", ss).bright_white()
            );
        }
    }

    // List partitions
    if verbose {
        eprintln!("[VERBOSE] Analyzing partition table...");
    }
    println!("\n{}", "🗂  Partitions".truecolor(222, 115, 86).bold());
    println!("{}", "─".repeat(60).bright_black());
    let partitions = g.list_partitions()?;
    for partition in &partitions {
        if verbose {
            eprintln!("[VERBOSE] Examining partition: {}", partition);
        }
        println!(
            "  {} {}",
            "📦".truecolor(222, 115, 86),
            partition.bright_white().bold()
        );

        let device = partition
            .split(|c: char| c.is_ascii_digit())
            .next()
            .unwrap_or(partition);
        if let Ok(part_list) = g.part_list(device) {
            let part_num = g.part_to_partnum(partition)?;
            if let Some(p) = part_list.iter().find(|p| p.part_num == part_num) {
                println!(
                    "    {} Number: {}",
                    "•".bright_black(),
                    format!("{}", p.part_num).yellow()
                );
                println!(
                    "    {} Start:  {}",
                    "•".bright_black(),
                    format!("{} bytes", p.part_start).bright_black()
                );
                println!(
                    "    {} Size:   {} ({})",
                    "•".bright_black(),
                    format!("{} bytes", p.part_size).bright_black(),
                    format!("{:.2} GB", p.part_size as f64 / 1e9).bright_white()
                );
                println!(
                    "    {} End:    {}",
                    "•".bright_black(),
                    format!("{} bytes", p.part_end).bright_black()
                );
            }
        }
    }

    // Partition scheme
    if verbose {
        eprintln!("[VERBOSE] Detecting partition scheme...");
    }
    if let Ok(scheme) = g.part_get_parttype("/dev/sda") {
        println!(
            "\n{}",
            "⚙️  Partition Scheme".truecolor(222, 115, 86).bold()
        );
        println!("{}", "─".repeat(60).bright_black());
        let scheme_icon = match scheme.as_str() {
            "gpt" => "🔷",
            "msdos" | "mbr" => "🔶",
            _ => "⬡",
        };
        println!("  {} Type: {}", scheme_icon, scheme.bright_white().bold());
        if verbose {
            eprintln!("[VERBOSE] Partition scheme: {}", scheme);
        }
    }

    // List filesystems
    if verbose {
        eprintln!("[VERBOSE] Detecting filesystems...");
    }
    println!("\n{}", "📁 Filesystems".truecolor(222, 115, 86).bold());
    println!("{}", "─".repeat(60).bright_black());
    let filesystems = g.list_filesystems()?;
    for (device, fstype) in &filesystems {
        if verbose {
            eprintln!("[VERBOSE] Filesystem on {}: {}", device, fstype);
        }

        let fs_icon = match fstype.as_str() {
            "ext2" | "ext3" | "ext4" => "🐧",
            "xfs" => "🔴",
            "btrfs" => "🌳",
            "ntfs" => "🪟",
            "vfat" | "fat" => "📂",
            "swap" => "💾",
            _ => "❓",
        };

        if fstype == "unknown" {
            println!(
                "  {} {} {}",
                fs_icon,
                device.yellow(),
                fstype.bright_black()
            );
        } else {
            println!(
                "  {} {} {}",
                fs_icon,
                device.yellow(),
                fstype.bright_white().bold()
            );
        }

        if fstype != "unknown" && fstype != "swap" {
            if let Ok(label) = g.vfs_label(device) {
                if !label.is_empty() {
                    println!("    {} Label: {}", "•".bright_black(), label.bright_white());
                }
            }
            if let Ok(uuid) = g.vfs_uuid(device) {
                if !uuid.is_empty() {
                    println!("    {} UUID:  {}", "•".bright_black(), uuid.bright_black());
                }
            }
        }
    }

    // OS inspection
    progress.set_message("Detecting operating systems...");
    if verbose {
        eprintln!("[VERBOSE] Running OS detection algorithms...");
    }
    let roots = g.inspect_os()?;

    progress.finish_and_clear();

    // Text display path continues — profile handling for text mode
    if let Some(profile_name) = profile {
        if roots.is_empty() {
            eprintln!("No operating systems found in image");
            g.shutdown()?;
            return Ok(());
        }

        let root = &roots[0];

        if let Some(profile_impl) = get_profile(&profile_name) {
            println!("\n=== {} ===\n", profile_impl.description());
            let report = profile_impl.inspect(&mut g, root)?;
            print_profile_report(&report);
            g.shutdown()?;
            return Ok(());
        } else {
            eprintln!(
                "Unknown profile: {}. Available: security, migration, performance",
                profile_name
            );
            g.shutdown()?;
            return Err(anyhow::anyhow!("Invalid profile"));
        }
    }

    // Traditional text output with killer UX

    // Print Quick Summary first
    if !roots.is_empty() {
        println!("\n╭─────────────────────────────────────────────────────────╮");
        println!(
            "│ {} {}",
            "✨ Quick Summary".truecolor(222, 115, 86).bold(),
            " ".repeat(38)
        );
        println!("╰─────────────────────────────────────────────────────────╯");

        for root in &roots {
            if let Ok(ostype) = g.inspect_get_type(root) {
                let os_icon = match ostype.as_str() {
                    "linux" => "🐧",
                    "windows" => "🪟",
                    "freebsd" => "👿",
                    _ => "💻",
                };

                let product = g
                    .inspect_get_product_name(root)
                    .unwrap_or_else(|_| "Unknown".to_string());
                let distro = g
                    .inspect_get_distro(root)
                    .unwrap_or_else(|_| "unknown".to_string());
                let major = g.inspect_get_major_version(root).unwrap_or(0);
                let minor = g.inspect_get_minor_version(root).unwrap_or(0);

                print!("  {} {} ", os_icon, product.bright_green().bold());
                if major > 0 || minor > 0 {
                    print!("{} ", format!("v{}.{}", major, minor).bright_white());
                }
                println!("({})", distro.truecolor(222, 115, 86));
            }
        }
        println!();
    }

    println!("{}", "🖥️  Operating Systems".truecolor(222, 115, 86).bold());
    println!("{}", "─".repeat(60).bright_black());

    if roots.is_empty() {
        println!(
            "  {} {}",
            "⚠️".yellow(),
            "No operating systems found".bright_black()
        );
        if verbose {
            eprintln!("[VERBOSE] No bootable operating systems detected");
        }
    } else {
        for root in &roots {
            if verbose {
                eprintln!("[VERBOSE] Inspecting OS at root: {}", root);
            }
            println!(
                "  {} Root: {}",
                "🔹".truecolor(222, 115, 86),
                root.bright_white().bold()
            );
            println!();

            if let Ok(ostype) = g.inspect_get_type(root) {
                if verbose {
                    eprintln!("[VERBOSE] OS type detected: {}", ostype);
                }
                let os_icon = match ostype.as_str() {
                    "linux" => "🐧",
                    "windows" => "🪟",
                    "freebsd" => "👿",
                    _ => "💻",
                };
                println!(
                    "    {} Type:         {}",
                    os_icon,
                    ostype.bright_white().bold()
                );
            }
            if let Ok(distro) = g.inspect_get_distro(root) {
                if verbose {
                    eprintln!("[VERBOSE] Distribution: {}", distro);
                }
                if distro == "unknown" {
                    println!(
                        "    {} Distribution: {}",
                        "📦".bright_black(),
                        distro.bright_black()
                    );
                } else {
                    println!(
                        "    {} Distribution: {}",
                        "📦".green(),
                        distro.bright_green().bold()
                    );
                }
            }
            if let Ok(product) = g.inspect_get_product_name(root) {
                if verbose {
                    eprintln!("[VERBOSE] Product name: {}", product);
                }
                println!(
                    "    {} Product:      {}",
                    "🏷️".green(),
                    product.bright_green().bold()
                );
            }
            if let Ok(arch) = g.inspect_get_arch(root) {
                if verbose {
                    eprintln!("[VERBOSE] Architecture: {}", arch);
                }
                println!(
                    "    {} Architecture: {}",
                    "⚙️".truecolor(222, 115, 86),
                    arch.truecolor(222, 115, 86).bold()
                );
            }
            if let Ok(major) = g.inspect_get_major_version(root) {
                if let Ok(minor) = g.inspect_get_minor_version(root) {
                    if verbose {
                        eprintln!("[VERBOSE] Version: {}.{}", major, minor);
                    }
                    let version = format!("{}.{}", major, minor);
                    if version == "0.0" {
                        println!(
                            "    {} Version:      {}",
                            "🔢".bright_black(),
                            version.bright_black()
                        );
                    } else {
                        println!(
                            "    {} Version:      {}",
                            "🔢".green(),
                            version.bright_green().bold()
                        );
                    }
                }
            }
            if let Ok(hostname) = g.inspect_get_hostname(root) {
                if verbose {
                    eprintln!("[VERBOSE] Hostname: {}", hostname);
                }
                if hostname == "localhost" {
                    println!(
                        "    {} Hostname:     {}",
                        "🏠".bright_black(),
                        hostname.bright_black()
                    );
                } else {
                    println!(
                        "    {} Hostname:     {}",
                        "🏠".blue(),
                        hostname.bright_blue().bold()
                    );
                }
            }
            if let Ok(pkg_fmt) = g.inspect_get_package_format(root) {
                if verbose {
                    eprintln!("[VERBOSE] Package format: {}", pkg_fmt);
                }
                let pkg_icon = match pkg_fmt.as_str() {
                    "rpm" => "🔴",
                    "deb" => "🟣",
                    "pacman" => "📦",
                    _ => "📦",
                };
                if pkg_fmt == "unknown" {
                    println!("    {} Packages:     {}", pkg_icon, pkg_fmt.bright_black());
                } else {
                    println!(
                        "    {} Packages:     {}",
                        pkg_icon,
                        pkg_fmt.bright_magenta().bold()
                    );
                }
            }

            // Additional detailed information
            if verbose {
                eprintln!("[VERBOSE] Retrieving init system information...");
            }
            if let Ok(init) = g.inspect_get_init_system(root) {
                if init == "unknown" {
                    println!(
                        "    {} Init system:  {}",
                        "⚡".bright_black(),
                        init.bright_black()
                    );
                } else {
                    println!(
                        "    {} Init system:  {}",
                        "⚡".yellow(),
                        init.truecolor(222, 115, 86).bold()
                    );
                }
            }

            if verbose {
                eprintln!("[VERBOSE] Detecting package management tool...");
            }
            if let Ok(pkg_mgr) = g.inspect_get_package_management(root) {
                if pkg_mgr == "unknown" {
                    println!(
                        "    {} Pkg Manager:  {}",
                        "🔧".yellow(),
                        pkg_mgr.bright_black()
                    );
                } else {
                    println!(
                        "    {} Pkg Manager:  {}",
                        "🔧".yellow(),
                        pkg_mgr.bright_white().bold()
                    );
                }
            }

            if verbose {
                eprintln!("[VERBOSE] Checking OS format...");
            }
            if let Ok(format) = g.inspect_get_format(root) {
                println!(
                    "    {} Format:       {}",
                    "💿".yellow(),
                    format.bright_white()
                );
            }

            if verbose {
                eprintln!("[VERBOSE] Checking for product variant...");
            }
            if let Ok(variant) = g.inspect_get_product_variant(root) {
                if !variant.is_empty() {
                    println!("    Variant:      {}", variant);
                }
            }

            // Mount points
            if verbose {
                eprintln!("[VERBOSE] Analyzing mount points...");
            }
            if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
                if mountpoints.len() > 1 {
                    println!("    Mount points:");
                    for (mp, dev) in mountpoints {
                        println!("      {} -> {}", mp, dev);
                        if verbose {
                            eprintln!("[VERBOSE] Mountpoint: {} -> {}", mp, dev);
                        }
                    }
                }
            }

            // Flags and characteristics
            if verbose {
                eprintln!("[VERBOSE] Checking OS characteristics...");
            }
            if let Ok(multipart) = g.inspect_is_multipart(root) {
                if multipart {
                    println!("    Multipart:    yes");
                }
            }
            if let Ok(live) = g.inspect_is_live(root) {
                if live {
                    println!("    Live CD:      yes");
                }
            }
            if let Ok(netinst) = g.inspect_is_netinst(root) {
                if netinst {
                    println!("    NetInstall:   yes");
                }
            }

            // Try to mount and get additional info
            if verbose {
                eprintln!(
                    "[VERBOSE] Attempting to mount root filesystem for detailed inspection..."
                );
            }
            if g.mount_ro(root, "/").is_ok() {
                // Filesystem usage
                if verbose {
                    eprintln!("[VERBOSE] Getting filesystem usage statistics...");
                }
                if let Ok(usage) = g.statvfs("/") {
                    let blocks = *usage.get("blocks").unwrap_or(&0);
                    let bsize = *usage.get("bsize").unwrap_or(&4096);
                    let bfree = *usage.get("bfree").unwrap_or(&0);

                    let total_bytes = blocks.saturating_mul(bsize);
                    let free_bytes = bfree.saturating_mul(bsize);
                    let used_bytes = total_bytes.saturating_sub(free_bytes);
                    let used_percent = if total_bytes > 0 {
                        (used_bytes as f64 / total_bytes as f64) * 100.0
                    } else {
                        0.0
                    };

                    println!("    Disk usage:");
                    println!("      Total: {:.2} GB", total_bytes as f64 / 1e9);
                    println!(
                        "      Used:  {:.2} GB ({:.1}%)",
                        used_bytes as f64 / 1e9,
                        used_percent
                    );
                    println!("      Free:  {:.2} GB", free_bytes as f64 / 1e9);
                }

                // Count installed packages
                if verbose {
                    eprintln!("[VERBOSE] Counting installed packages...");
                }
                match g.inspect_get_package_format(root) {
                    Ok(pkg_fmt) if pkg_fmt == "rpm" => {
                        if let Ok(packages) = g.rpm_list() {
                            println!("    Installed RPM packages: {}", packages.len());
                            if verbose {
                                eprintln!("[VERBOSE] Found {} RPM packages", packages.len());
                            }
                        }
                    }
                    Ok(pkg_fmt) if pkg_fmt == "deb" => {
                        if let Ok(packages) = g.dpkg_list() {
                            println!("    Installed DEB packages: {}", packages.len());
                            if verbose {
                                eprintln!("[VERBOSE] Found {} DEB packages", packages.len());
                            }
                        }
                    }
                    _ => {}
                }

                // Kernel information
                if verbose {
                    eprintln!("[VERBOSE] Searching for kernel versions...");
                }
                if let Ok(files) = g.ls("/boot") {
                    let kernels: Vec<_> = files
                        .iter()
                        .filter(|f| f.starts_with("vmlinuz-") || f.starts_with("vmlinux-"))
                        .collect();
                    if !kernels.is_empty() {
                        println!("    Installed kernels:");
                        for kernel in kernels {
                            println!("      {}", kernel);
                            if verbose {
                                eprintln!("[VERBOSE] Found kernel: {}", kernel);
                            }
                        }
                    }
                }

                g.umount("/").ok();
            } else if verbose {
                eprintln!("[VERBOSE] Could not mount root filesystem for detailed inspection");
            }

            // System Configuration
            if verbose {
                eprintln!("[VERBOSE] Gathering system configuration...");
            }
            println!();
            println!(
                "    {}",
                "⚙️  System Configuration".truecolor(222, 115, 86).bold()
            );
            println!("    {}", "─".repeat(56).bright_black());

            if let Ok(timezone) = g.inspect_timezone(root) {
                if timezone == "unknown" {
                    println!(
                        "      {} Timezone:    {}",
                        "🌍".yellow(),
                        timezone.bright_black()
                    );
                } else {
                    println!(
                        "      {} Timezone:    {}",
                        "🌍".yellow(),
                        timezone.bright_white().bold()
                    );
                }
            }

            if let Ok(locale) = g.inspect_locale(root) {
                if locale == "unknown" {
                    println!(
                        "      {} Locale:      {}",
                        "🗣️".yellow(),
                        locale.bright_black()
                    );
                } else {
                    println!(
                        "      {} Locale:      {}",
                        "🗣️".yellow(),
                        locale.bright_white()
                    );
                }
            }

            // SELinux
            if let Ok(selinux) = g.inspect_selinux(root) {
                match selinux.as_str() {
                    "enforcing" => println!("      🔒 SELinux:     {}", selinux.green().bold()),
                    "permissive" => println!("      ⚠️ SELinux:     {}", selinux.yellow()),
                    "disabled" => println!("      🔓 SELinux:     {}", selinux.bright_black()),
                    _ => println!("      ❓ SELinux:     {}", selinux.bright_black()),
                }
            }

            // Cloud-init
            if let Ok(has_cloud_init) = g.inspect_cloud_init(root) {
                if has_cloud_init {
                    println!(
                        "      {} Cloud-init:  {}",
                        "☁️".yellow(),
                        "yes".green().bold()
                    );
                }
            }

            // VM Tools
            if verbose {
                eprintln!("[VERBOSE] Detecting virtualization guest tools...");
            }
            if let Ok(vm_tools) = g.inspect_vm_tools(root) {
                if !vm_tools.is_empty() {
                    println!(
                        "      {} VM Tools:    {}",
                        "🔧".yellow(),
                        vm_tools.join(", ").bright_white().bold()
                    );
                }
            }

            // Network Configuration
            if verbose {
                eprintln!("[VERBOSE] Analyzing network configuration...");
            }
            if let Ok(interfaces) = g.inspect_network(root) {
                if !interfaces.is_empty() {
                    println!();
                    println!(
                        "    {}",
                        "🌐 Network Configuration".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());
                    for iface in &interfaces {
                        println!(
                            "      {} Interface: {}",
                            "📡".yellow(),
                            iface.name.bright_white().bold()
                        );
                        if !iface.ip_address.is_empty() {
                            println!(
                                "        {} IP:   {}",
                                "•".bright_black(),
                                iface.ip_address.join(", ").bright_white()
                            );
                        }
                        if !iface.mac_address.is_empty() {
                            println!(
                                "        {} MAC:  {}",
                                "•".bright_black(),
                                iface.mac_address.bright_black()
                            );
                        }
                        if iface.dhcp {
                            println!(
                                "        {} DHCP: {}",
                                "•".bright_black(),
                                "yes".green().bold()
                            );
                        } else {
                            println!(
                                "        {} DHCP: {}",
                                "•".bright_black(),
                                "no".bright_black()
                            );
                        }
                    }
                }
            }

            if let Ok(dns_servers) = g.inspect_dns(root) {
                if !dns_servers.is_empty() {
                    println!(
                        "      {} DNS:  {}",
                        "🌐".yellow(),
                        dns_servers.join(", ").bright_white().bold()
                    );
                }
            }

            // User Accounts
            if verbose {
                eprintln!("[VERBOSE] Listing user accounts...");
            }
            if let Ok(users) = g.inspect_users(root) {
                let regular_users: Vec<_> = users
                    .iter()
                    .filter(|u| {
                        let uid: u32 = u.uid.parse().unwrap_or(0);
                        (1000..65534).contains(&uid)
                    })
                    .collect();

                let system_users: Vec<_> = users
                    .iter()
                    .filter(|u| {
                        let uid: u32 = u.uid.parse().unwrap_or(0);
                        uid > 0 && uid < 1000
                    })
                    .collect();

                if !regular_users.is_empty() || !system_users.is_empty() {
                    println!();
                    println!("    {}", "👥 User Accounts".truecolor(222, 115, 86).bold());
                    println!("    {}", "─".repeat(56).bright_black());

                    if !regular_users.is_empty() {
                        println!(
                            "      {} Regular users: {}",
                            "👤".yellow(),
                            regular_users.len().to_string().bright_white().bold()
                        );
                        for user in regular_users.iter().take(10) {
                            println!(
                                "        {} {} {} {} {}",
                                "•".bright_black(),
                                user.username.bright_white().bold(),
                                format!("(uid: {})", user.uid).bright_black(),
                                "→".bright_black(),
                                user.home.bright_black()
                            );
                        }
                        if regular_users.len() > 10 {
                            println!(
                                "        {} and {} more...",
                                "•".bright_black(),
                                (regular_users.len() - 10).to_string().bright_black()
                            );
                        }
                    }

                    println!(
                        "      {} System users: {}",
                        "⚙️".bright_black(),
                        system_users.len().to_string().bright_black()
                    );
                }
            }

            // SSH Configuration
            if verbose {
                eprintln!("[VERBOSE] Checking SSH configuration...");
            }
            if let Ok(ssh_config) = g.inspect_ssh_config(root) {
                if !ssh_config.is_empty() {
                    println!();
                    println!(
                        "    {}",
                        "🔐 SSH Configuration".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());
                    if let Some(port) = ssh_config.get("Port") {
                        println!(
                            "      {} Port: {}",
                            "•".bright_black(),
                            port.bright_white().bold()
                        );
                    }
                    if let Some(permit_root) = ssh_config.get("PermitRootLogin") {
                        if permit_root == "yes" {
                            println!(
                                "      {} PermitRootLogin: {}",
                                "•".bright_black(),
                                permit_root.red()
                            );
                        } else {
                            println!(
                                "      {} PermitRootLogin: {}",
                                "•".bright_black(),
                                permit_root.green()
                            );
                        }
                    }
                    if let Some(password_auth) = ssh_config.get("PasswordAuthentication") {
                        if password_auth == "no" {
                            println!(
                                "      {} PasswordAuth: {}",
                                "•".bright_black(),
                                password_auth.green()
                            );
                        } else {
                            println!(
                                "      {} PasswordAuth: {}",
                                "•".bright_black(),
                                password_auth.yellow()
                            );
                        }
                    }
                }
            }

            // Systemd Services
            if verbose {
                eprintln!("[VERBOSE] Listing systemd services...");
            }
            if let Ok(services) = g.inspect_systemd_services(root) {
                if !services.is_empty() {
                    println!();
                    println!(
                        "    {}",
                        "⚙️  Systemd Services".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());
                    println!(
                        "      {} Enabled: {}",
                        "✓".green(),
                        services.len().to_string().bright_white().bold()
                    );
                    for service in services.iter().take(15) {
                        println!(
                            "        {} {}",
                            "•".bright_black(),
                            service.name.bright_white()
                        );
                    }
                    if services.len() > 15 {
                        println!(
                            "        {} and {} more...",
                            "•".bright_black(),
                            (services.len() - 15).to_string().bright_black()
                        );
                    }
                }
            }

            // Language Runtimes
            if verbose {
                eprintln!("[VERBOSE] Detecting language runtimes...");
            }
            if let Ok(runtimes) = g.inspect_runtimes(root) {
                if !runtimes.is_empty() {
                    println!();
                    println!(
                        "    {}",
                        "💻 Language Runtimes".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());

                    // Define icons for each runtime
                    for runtime in runtimes.keys() {
                        let (icon, name) = match runtime.as_str() {
                            "python3" | "python" | "python2" => ("🐍", runtime.as_str()),
                            "node" | "nodejs" => ("🟢", "Node.js"),
                            "java" => ("☕", "Java"),
                            "ruby" => ("💎", "Ruby"),
                            "go" => ("🔷", "Go"),
                            "perl" => ("🐪", "Perl"),
                            _ => ("📦", runtime.as_str()),
                        };
                        println!("      {} {}", icon, name.bright_white().bold());
                    }
                }
            }

            // Container Runtimes
            if verbose {
                eprintln!("[VERBOSE] Detecting container runtimes...");
            }
            if let Ok(container_runtimes) = g.inspect_container_runtimes(root) {
                if !container_runtimes.is_empty() {
                    println!();
                    println!(
                        "    {}",
                        "🐳 Container Runtimes".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());
                    for runtime in &container_runtimes {
                        let (icon, name) = match runtime.as_str() {
                            "docker" => ("🐳", "Docker"),
                            "podman" => ("🦭", "Podman"),
                            "containerd" => ("📦", "containerd"),
                            "cri-o" => ("🔷", "CRI-O"),
                            _ => ("📦", runtime.as_str()),
                        };
                        println!("      {} {}", icon, name.bright_white().bold());
                    }
                }
            }

            // Storage Configuration
            if verbose {
                eprintln!("[VERBOSE] Analyzing storage configuration...");
            }
            if let Ok(lvm_info) = g.inspect_lvm(root) {
                if !lvm_info.physical_volumes.is_empty()
                    || !lvm_info.volume_groups.is_empty()
                    || !lvm_info.logical_volumes.is_empty()
                {
                    println!();
                    println!(
                        "    {}",
                        "💾 LVM Configuration".truecolor(222, 115, 86).bold()
                    );
                    println!("    {}", "─".repeat(56).bright_black());
                    if !lvm_info.physical_volumes.is_empty() {
                        println!(
                            "      {} Physical Volumes: {}",
                            "🔷".bright_blue(),
                            lvm_info.physical_volumes.join(", ").bright_white()
                        );
                    }
                    if !lvm_info.volume_groups.is_empty() {
                        let vg_names = lvm_info
                            .volume_groups
                            .iter()
                            .map(|vg| vg.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        println!(
                            "      {} Volume Groups: {}",
                            "📦".yellow(),
                            vg_names.bright_white().bold()
                        );
                    }
                    if !lvm_info.logical_volumes.is_empty() {
                        let lv_names = lvm_info
                            .logical_volumes
                            .iter()
                            .map(|lv| lv.name.as_str())
                            .collect::<Vec<_>>()
                            .join(", ");
                        println!(
                            "      {} Logical Volumes: {}",
                            "💿".truecolor(222, 115, 86),
                            lv_names.bright_white()
                        );
                    }
                }
            }

            // Swap
            if let Ok(swap_devices) = g.inspect_swap(root) {
                if !swap_devices.is_empty() {
                    println!("\n    === Swap Configuration ===");
                    for swap in &swap_devices {
                        println!("      {}", swap);
                    }
                }
            }

            // fstab mounts
            if let Ok(fstab_mounts) = g.inspect_fstab(root) {
                if fstab_mounts.len() > 1 {
                    println!("\n    === Filesystem Mounts (fstab) ===");
                    for (device, mountpoint, fstype) in fstab_mounts.iter().take(10) {
                        println!("      {} on {} type {}", device, mountpoint, fstype);
                    }
                }
            }

            // Boot Configuration
            if verbose {
                eprintln!("[VERBOSE] Analyzing boot configuration...");
            }
            if let Ok(boot_config) = g.inspect_boot_config(root) {
                if boot_config.bootloader != "unknown" {
                    println!("\n    === Boot Configuration ===");
                    println!("      Bootloader: {}", boot_config.bootloader);
                    if boot_config.timeout != "unknown" {
                        println!("      Timeout: {}", boot_config.timeout);
                    }
                    if boot_config.default_entry != "unknown" {
                        println!("      Default: {}", boot_config.default_entry);
                    }
                }
            }

            // Scheduled Tasks
            if verbose {
                eprintln!("[VERBOSE] Checking scheduled tasks...");
            }
            if let Ok(cron_jobs) = g.inspect_cron(root) {
                if !cron_jobs.is_empty() {
                    println!("\n    === Cron Jobs ===");
                    println!("      Total: {}", cron_jobs.len());
                    for job in cron_jobs.iter().take(5) {
                        println!("        {}", job);
                    }
                    if cron_jobs.len() > 5 {
                        println!("        ... and {} more", cron_jobs.len() - 5);
                    }
                }
            }

            if let Ok(timers) = g.inspect_systemd_timers(root) {
                if !timers.is_empty() {
                    println!("\n    === Systemd Timers ===");
                    for timer in &timers {
                        println!("      {}", timer);
                    }
                }
            }

            // SSL Certificates
            if verbose {
                eprintln!("[VERBOSE] Scanning SSL certificates...");
            }
            if let Ok(certs) = g.inspect_certificates(root) {
                if !certs.is_empty() {
                    println!("\n    === SSL Certificates ===");
                    println!("      Found: {} certificates", certs.len());
                    for cert in certs.iter().take(5) {
                        println!("        {} ({})", cert.path, cert.subject);
                    }
                    if certs.len() > 5 {
                        println!("        ... and {} more", certs.len() - 5);
                    }
                }
            }

            // Kernel Parameters
            if verbose {
                eprintln!("[VERBOSE] Reading kernel parameters...");
            }
            if let Ok(kernel_params) = g.inspect_kernel_params(root) {
                if !kernel_params.is_empty() {
                    println!("\n    === Kernel Parameters (sysctl) ===");
                    println!("      Total: {}", kernel_params.len());
                    let mut params_vec: Vec<_> = kernel_params.iter().collect();
                    params_vec.sort_by_key(|&(k, _)| k);
                    for (key, value) in params_vec.iter().take(10) {
                        println!("        {} = {}", key, value);
                    }
                    if kernel_params.len() > 10 {
                        println!("        ... and {} more", kernel_params.len() - 10);
                    }
                }
            }
        }
    }

    if verbose {
        eprintln!("[VERBOSE] Shutting down appliance...");
    }
    g.shutdown()?;

    if verbose {
        eprintln!("[VERBOSE] Inspection complete");
    }
    Ok(())
}

/// List filesystems and partitions
pub fn list_filesystems(image: &Path, detailed: bool, verbose: bool) -> Result<()> {
    let progress = ProgressReporter::spinner("Loading disk image...");
    let mut g = init_guestfs_ro(image, verbose)?;
    progress.set_message("Scanning filesystems...");

    let devices = g.list_devices().context("Failed to list devices")?;

    progress.finish_and_clear();

    println!("\n{}", "═".repeat(70).bright_blue());
    println!(
        "{} {}",
        "💾 Disk Image:".truecolor(222, 115, 86).bold(),
        image.display().to_string().bright_white()
    );
    println!("{}\n", "═".repeat(70).bright_blue());

    // Devices
    println!("{}", "Block Devices".bright_white().bold());
    println!("{}", "─".repeat(50).bright_black());
    for device in devices {
        println!(
            "  {} {}",
            "▪".truecolor(222, 115, 86),
            device.bright_white().bold()
        );

        if detailed {
            if let Ok(size) = g.blockdev_getsize64(&device) {
                let gb = size as f64 / 1_073_741_824.0; // 1024^3
                println!(
                    "    {} {} ({:.2} GiB)",
                    "Size:".dimmed(),
                    size.to_string().truecolor(222, 115, 86),
                    gb
                );
            }

            if let Ok(parttype) = g.part_get_parttype(&device) {
                println!(
                    "    {} {}",
                    "Partition table:".dimmed(),
                    parttype.bright_green()
                );
            }
        }
    }

    // Partitions
    let partitions = g.list_partitions().context("Failed to list partitions")?;
    if !partitions.is_empty() {
        println!("\n{}", "Partitions".bright_white().bold());
        println!("{}", "─".repeat(50).bright_black());

        for partition in partitions {
            let fstype = g
                .vfs_type(&partition)
                .unwrap_or_else(|_| "unknown".to_string());
            let size = g.blockdev_getsize64(&partition).unwrap_or(0);
            let gb = size as f64 / 1_073_741_824.0;

            let fs_icon = match fstype.as_str() {
                "ext2" | "ext3" | "ext4" => "📁",
                "ntfs" => "🪟",
                "vfat" | "fat" | "fat32" => "💾",
                "xfs" => "🗄",
                "btrfs" => "🌳",
                "swap" => "💫",
                _ => "❓",
            };

            println!(
                "  {} {} {} {}",
                fs_icon,
                partition.bright_white().bold(),
                format!("({})", fstype).truecolor(222, 115, 86),
                format!("{:.1} GiB", gb).truecolor(222, 115, 86)
            );

            if let Ok(label) = g.vfs_label(&partition) {
                if !label.is_empty() {
                    println!("    {} {}", "Label:".dimmed(), label.bright_green());
                }
            }

            if detailed {
                if let Ok(uuid) = g.vfs_uuid(&partition) {
                    if !uuid.is_empty() {
                        println!("    {} {}", "UUID:".dimmed(), uuid.dimmed());
                    }
                }

                if let Ok(partnum) = g.part_to_partnum(&partition) {
                    println!(
                        "    {} {}",
                        "Number:".dimmed(),
                        partnum.to_string().bright_magenta()
                    );
                }
            }
        }
    }

    // LVM information
    if let Ok(vgs) = g.vgs() {
        if !vgs.is_empty() {
            println!("\n{}", "LVM Volume Groups".bright_white().bold());
            println!("{}", "─".repeat(50).bright_black());
            for vg in vgs {
                println!("  {} {}", "▸".bright_magenta(), vg.bright_white().bold());
            }
        }
    }

    if let Ok(lvs) = g.lvs() {
        if !lvs.is_empty() {
            println!("\n{}", "LVM Logical Volumes".bright_white().bold());
            println!("{}", "─".repeat(50).bright_black());
            for lv in lvs {
                let size = g.blockdev_getsize64(&lv).unwrap_or(0);
                let gb = size as f64 / 1_073_741_824.0;

                println!(
                    "  {} {} {}",
                    "▸".bright_magenta(),
                    lv.bright_white().bold(),
                    format!("{:.1} GiB", gb).truecolor(222, 115, 86)
                );
            }
        }
    }

    println!("\n{}", "═".repeat(70).bright_blue());

    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}

/// List installed packages
pub fn list_packages(
    image: &Path,
    filter: Option<String>,
    limit: Option<usize>,
    json_output: bool,
    verbose: bool,
) -> Result<()> {
    use serde_json::json;

    // Show progress for long operations
    let progress = if !json_output {
        let p = ProgressReporter::spinner("Loading disk image...");
        Some(p)
    } else {
        None
    };

    let mut g = init_guestfs_ro(image, verbose)?;

    if let Some(ref p) = progress {
        p.set_message("Detecting operating system...");
    }

    let roots = g.inspect_os().context("Failed to inspect OS")?;
    if roots.is_empty() {
        if let Some(p) = progress {
            p.abandon_with_message("No operating system detected");
        }
        anyhow::bail!("No operating system detected in disk image");
    }

    let root = &roots[0];

    // Mount filesystems
    if let Some(ref p) = progress {
        p.set_message("Mounting filesystems...");
    }

    if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
        let mut mounts: Vec<_> = mountpoints.iter().collect();
        mounts.sort_by_key(|(mount, _)| mount.len());
        for (mount, device) in mounts {
            g.mount_ro(device, mount).ok();
        }
    }

    // List packages
    if let Some(ref p) = progress {
        p.set_message("Listing installed packages...");
    }

    let apps = g
        .inspect_list_applications(root)
        .context("Failed to list applications")?;

    if let Some(p) = progress {
        p.finish_and_clear();
    }

    // Apply filter
    let filtered: Vec<_> = apps
        .into_iter()
        .filter(|app| {
            if let Some(ref f) = filter {
                app.name.contains(f)
            } else {
                true
            }
        })
        .collect();

    // Apply limit
    let limited: Vec<_> = if let Some(lim) = limit {
        filtered.into_iter().take(lim).collect()
    } else {
        filtered
    };

    if json_output {
        let packages: Vec<_> = limited
            .iter()
            .map(|app| {
                json!({
                    "name": app.name,
                    "version": app.version,
                    "release": app.release,
                    "epoch": app.epoch,
                })
            })
            .collect();

        println!(
            "{}",
            serde_json::to_string_pretty(&json!({
                "total": packages.len(),
                "packages": packages,
            }))?
        );
    } else {
        println!("Found {} package(s)\n", limited.len());

        if !limited.is_empty() {
            println!("{:<40} {:<20} {:<20}", "Package", "Version", "Release");
            println!("{}", "-".repeat(82));

            for app in limited {
                let name = if app.name.chars().count() > 38 {
                    format!("{}...", app.name.chars().take(35).collect::<String>())
                } else {
                    app.name.clone()
                };

                let version = if app.version.chars().count() > 18 {
                    format!("{}...", app.version.chars().take(15).collect::<String>())
                } else {
                    app.version.clone()
                };

                let release = if app.release.chars().count() > 18 {
                    format!("{}...", app.release.chars().take(15).collect::<String>())
                } else {
                    app.release.clone()
                };

                println!("{:<40} {:<20} {:<20}", name, version, release);
            }
        }
    }

    if let Err(e) = g.umount_all() {
        log::warn!("Cleanup: umount_all failed: {}", e);
    }
    if let Err(e) = g.shutdown() {
        log::warn!("Cleanup: shutdown failed: {}", e);
    }
    Ok(())
}
