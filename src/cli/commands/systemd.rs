// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd analysis commands
#![allow(clippy::too_many_arguments)]

use crate::core::systemd::boot::BootAnalyzer;
use crate::core::systemd::journal::{JournalFilter, JournalReader};
use crate::core::systemd::services::ServiceAnalyzer;
use crate::core::{ProgressReporter, SystemdAnalyzer};
use crate::Guestfs;
use anyhow::Result;
use owo_colors::OwoColorize;
use std::path::Path;

use super::{init_guestfs_ro, mount_all_ro};

/// Helper function to mount disk for systemd operations
fn mount_disk_for_systemd(image: &Path, verbose: bool) -> Result<(Guestfs, String)> {
    let progress = ProgressReporter::spinner("Loading disk image...");

    let mut g = init_guestfs_ro(image, verbose)?;

    progress.set_message("Mounting filesystems...");

    let root = mount_all_ro(&mut g).ok_or_else(|| {
        progress.abandon_with_message("No operating systems found");
        anyhow::anyhow!("No operating systems found in image")
    })?;

    progress.finish_and_clear();

    Ok((g, root))
}

/// Analyze systemd journal logs
pub fn systemd_journal_command(
    image: &Path,
    priority: Option<u8>,
    unit: Option<&str>,
    errors: bool,
    warnings: bool,
    stats: bool,
    limit: Option<usize>,
    verbose: bool,
) -> Result<()> {
    let (mut g, _root) = mount_disk_for_systemd(image, verbose)?;

    // Create temporary directory for analysis
    let temp_dir = tempfile::tempdir()?;
    let mount_path = temp_dir.path();

    // Copy journal directory if it exists
    let journal_path = "/var/log/journal";
    if g.is_dir(journal_path).unwrap_or(false) {
        let local_journal = mount_path.join("var/log/journal");
        std::fs::create_dir_all(&local_journal)?;

        if let Ok(entries) = g.ls(journal_path) {
            for entry in entries {
                let src = format!("{}/{}", journal_path, entry);
                let dst = local_journal.join(&entry);

                if g.is_dir(&src).unwrap_or(false) {
                    std::fs::create_dir_all(&dst)?;
                } else if g.is_file(&src).unwrap_or(false) {
                    if let Ok(content) = g.read_file(&src) {
                        std::fs::write(&dst, content)?;
                    }
                }
            }
        }
    }

    // Create analyzer and reader
    let analyzer = SystemdAnalyzer::new(mount_path);
    let reader = JournalReader::new(analyzer);

    if stats {
        // Show statistics
        let filter = JournalFilter {
            priority,
            unit: unit.map(String::from),
            limit,
            ..Default::default()
        };

        let statistics = reader.get_statistics(&filter)?;

        println!("{}", "Journal Statistics".bold().underline());
        println!();
        println!("Total entries: {}", statistics.total_entries);
        println!("Errors (0-3):  {}", statistics.error_count.red());
        println!("Warnings (4):  {}", statistics.warning_count.yellow());
        println!();

        println!("{}", "By Priority:".bold());
        let mut priorities: Vec<_> = statistics.by_priority.iter().collect();
        priorities.sort_by_key(|(p, _)| *p);
        for (priority, count) in priorities {
            let priority_name = match priority {
                0 => "EMERG",
                1 => "ALERT",
                2 => "CRIT",
                3 => "ERR",
                4 => "WARNING",
                5 => "NOTICE",
                6 => "INFO",
                7 => "DEBUG",
                _ => "UNKNOWN",
            };
            println!("  {} ({}): {}", priority_name, priority, count);
        }

        if !statistics.by_unit.is_empty() {
            println!();
            println!("{}", "Top Units:".bold());
            let mut units: Vec<_> = statistics.by_unit.iter().collect();
            units.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
            for (unit, count) in units.iter().take(10) {
                println!("  {}: {}", unit, count);
            }
        }
    } else {
        // Show journal entries
        let entries = if errors {
            reader.get_errors()?
        } else if warnings {
            reader.get_warnings()?
        } else {
            let filter = JournalFilter {
                priority,
                unit: unit.map(String::from),
                limit,
                ..Default::default()
            };
            reader.read_entries(&filter)?
        };

        if entries.is_empty() {
            println!("No journal entries found");
        } else {
            println!("{}", "Journal Entries".bold().underline());
            println!();

            for entry in entries {
                print!("{} ", entry.timestamp_str().dimmed());

                // Print priority with color
                print!("[");
                match entry.priority {
                    0..=2 => print!("{}", entry.priority_str().red()),
                    3 => print!("{}", entry.priority_str().bright_red()),
                    4 => print!("{}", entry.priority_str().yellow()),
                    5 => print!("{}", entry.priority_str().truecolor(222, 115, 86)),
                    _ => print!("{}", entry.priority_str().white()),
                }
                print!("] ");

                if let Some(ref unit) = entry.unit {
                    print!("{}: ", unit.bright_blue());
                }
                println!("{}", entry.message);
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

/// Analyze systemd services and dependencies
pub fn systemd_services_command(
    image: &Path,
    service: Option<&str>,
    failed: bool,
    diagram: bool,
    output: Option<&str>,
    verbose: bool,
) -> Result<()> {
    let (mut g, _root) = mount_disk_for_systemd(image, verbose)?;

    // Create temporary directory for analysis
    let temp_dir = tempfile::tempdir()?;
    let mount_path = temp_dir.path();

    // Copy systemd directories
    let systemd_dirs = vec![
        "/etc/systemd/system",
        "/lib/systemd/system",
        "/run/systemd/system",
    ];

    for dir in &systemd_dirs {
        if g.is_dir(dir).unwrap_or(false) {
            let local_dir = mount_path.join(dir.trim_start_matches('/'));
            std::fs::create_dir_all(&local_dir)?;

            if let Ok(entries) = g.ls(dir) {
                for entry in entries {
                    let src = format!("{}/{}", dir, entry);
                    if g.is_file(&src).unwrap_or(false) {
                        if let Ok(content) = g.read_file(&src) {
                            let dst = local_dir.join(&entry);
                            std::fs::write(&dst, content)?;
                        }
                    }
                }
            }
        }
    }

    // Create analyzer and service analyzer
    let analyzer = SystemdAnalyzer::new(mount_path);
    let service_analyzer = ServiceAnalyzer::new(analyzer);

    if let Some(service_name) = service {
        // Show specific service details
        if diagram {
            let mermaid = service_analyzer.generate_dependency_diagram(service_name)?;
            println!("{}", mermaid);
        } else {
            let dep_tree = service_analyzer.get_dependency_tree(service_name)?;
            println!(
                "{}",
                format!("Dependency Tree for {}", service_name)
                    .bold()
                    .underline()
            );
            println!();
            println!("Service: {}", dep_tree.service_name.bright_blue());
            println!("Dependencies: {}", dep_tree.count_dependencies());

            fn print_tree(tree: &crate::core::systemd::services::DependencyTree, indent: usize) {
                for dep in &tree.dependencies {
                    println!("{}{}", "  ".repeat(indent), dep.service_name);
                    print_tree(dep, indent + 1);
                }
            }

            if !dep_tree.dependencies.is_empty() {
                println!();
                print_tree(&dep_tree, 1);
            }
        }
    } else if failed {
        // Show failed services
        let failed_services = service_analyzer.get_failed_services()?;

        if failed_services.is_empty() {
            println!("{}", "No failed services found".green());
        } else {
            println!("{}", "Failed Services".bold().underline().red());
            println!();

            for service in failed_services {
                println!("{} {}", "✗".red(), service.name.bright_red());
                if let Some(desc) = service.description {
                    println!("  Description: {}", desc.dimmed());
                }
            }
        }
    } else {
        // List all services
        let services = service_analyzer.list_services()?;

        if output == Some("json") {
            println!("{}", serde_json::to_string_pretty(&services)?);
        } else {
            println!("{}", "Systemd Services".bold().underline());
            println!();
            println!(
                "{:<50} {:<15} {}",
                "Service".bold(),
                "State".bold(),
                "Description".bold()
            );
            println!("{}", "-".repeat(100));

            for service in services {
                let desc = service.description.unwrap_or_else(|| "-".to_string());

                // Print service name
                print!("{:<50} ", service.name.bright_blue());

                // Print colored state based on service state
                match service.state {
                    crate::core::ServiceState::Active => print!("{:<15} ", "active".green()),
                    crate::core::ServiceState::Failed => print!("{:<15} ", "failed".red()),
                    crate::core::ServiceState::Inactive => print!("{:<15} ", "inactive".dimmed()),
                    _ => print!("{:<15} ", service.state.to_string().white()),
                }

                // Print description
                println!("{}", desc.dimmed());
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

/// Analyze systemd boot performance
pub fn systemd_boot_command(
    image: &Path,
    timeline: bool,
    recommendations: bool,
    summary: bool,
    top: usize,
    verbose: bool,
) -> Result<()> {
    let (mut g, _root) = mount_disk_for_systemd(image, verbose)?;

    // Create temporary directory for analysis
    let temp_dir = tempfile::tempdir()?;
    let mount_path = temp_dir.path();

    // Try to copy systemd-analyze output if available
    let analyze_path = "/var/lib/systemd/analyze-blame.txt";
    if g.is_file(analyze_path).unwrap_or(false) {
        let local_analyze = mount_path.join("var/lib/systemd");
        std::fs::create_dir_all(&local_analyze)?;

        if let Ok(content) = g.read_file(analyze_path) {
            std::fs::write(local_analyze.join("analyze-blame.txt"), content)?;
        }
    }

    // Create analyzer and boot analyzer
    let analyzer = SystemdAnalyzer::new(mount_path);
    let boot_analyzer = BootAnalyzer::new(analyzer);

    let timing = boot_analyzer.analyze_boot()?;

    if timeline {
        // Show boot timeline diagram
        let mermaid = boot_analyzer.generate_boot_timeline(&timing);
        println!("{}", mermaid);
    } else if recommendations {
        // Show optimization recommendations
        let recs = boot_analyzer.get_recommendations(&timing);

        println!("{}", "Boot Optimization Recommendations".bold().underline());
        println!();

        for rec in recs {
            if rec.contains("looks good") {
                println!("{} {}", "✓".green(), rec.green());
            } else {
                println!("{} {}", "⚠".yellow(), rec);
            }
        }
    } else if summary {
        // Show summary statistics
        let sum = boot_analyzer.generate_summary(&timing);
        println!("{}", sum);
    } else {
        // Show slowest services
        let slowest = timing.slowest_services(top);

        println!("{}", "Boot Performance Analysis".bold().underline());
        println!();
        println!("Total Boot Time: {:.2}s", timing.total_time as f64 / 1000.0);
        println!("  - Kernel:     {:.2}s", timing.kernel_time as f64 / 1000.0);
        println!("  - Initrd:     {:.2}s", timing.initrd_time as f64 / 1000.0);
        println!(
            "  - Userspace:  {:.2}s",
            timing.userspace_time as f64 / 1000.0
        );
        println!();

        if slowest.is_empty() {
            println!("No service timing data available");
        } else {
            println!("{}", format!("Top {} Slowest Services:", top).bold());
            println!();
            println!("{:<50} {}", "Service".bold(), "Time".bold());
            println!("{}", "-".repeat(60));

            for service in slowest {
                let time_str = format!("{:.2}s", service.activation_time as f64 / 1000.0);

                // Print service name and colored time
                print!("{:<50} ", service.name.bright_blue());
                if service.activation_time > 3000 {
                    println!("{}", time_str.red());
                } else if service.activation_time > 1000 {
                    println!("{}", time_str.yellow());
                } else {
                    println!("{}", time_str.green());
                }
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
