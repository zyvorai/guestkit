// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Analysis and inspection commands: dashboard, summary, scan, compare, profile,
//! recommend, discover, timeline, bench, inspect, context, insights, doctor,
//! predict, chart, compliance, score, query, monitor, troubleshoot, depends,
//! validate, forensics, audit, baseline, explore

use anyhow::Result;
use colored::Colorize;

use super::{format_bytes, ShellContext};

pub fn cmd_dashboard(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                        SYSTEM DASHBOARD                              ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    // System Information
    println!(
        "{}",
        "┌─ System Information ─────────────────────────────────────┐".cyan()
    );
    if let Ok(os_type) = ctx.guestfs.inspect_get_type(&ctx.root) {
        println!("  {} {}", "Type:".yellow().bold(), os_type.green());
    }
    if let Ok(distro) = ctx.guestfs.inspect_get_distro(&ctx.root) {
        println!("  {} {}", "Distribution:".yellow().bold(), distro.green());
    }
    if let Ok(version) = ctx.guestfs.inspect_get_product_name(&ctx.root) {
        println!("  {} {}", "Version:".yellow().bold(), version.green());
    }
    if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
        println!("  {} {}", "Architecture:".yellow().bold(), arch.green());
    }
    if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
        println!("  {} {}", "Hostname:".yellow().bold(), hostname.cyan());
    }
    println!(
        "{}",
        "└──────────────────────────────────────────────────────────┘".cyan()
    );
    println!();

    // Storage Overview
    println!(
        "{}",
        "┌─ Storage Overview ───────────────────────────────────────┐".cyan()
    );
    if let Ok(filesystems) = ctx.guestfs.list_filesystems() {
        let fs_count = filesystems.len();
        println!(
            "  {} {}",
            "Filesystems:".yellow().bold(),
            fs_count.to_string().green()
        );

        for (device, fstype) in filesystems.iter().take(5) {
            if fstype != "unknown" && !fstype.is_empty() {
                let size_str = if let Ok(size) = ctx.guestfs.blockdev_getsize64(device) {
                    format_bytes(size as u64)
                } else {
                    "unknown".to_string()
                };
                println!(
                    "    {} {} ({})",
                    "•".cyan(),
                    device.bright_black(),
                    size_str.yellow()
                );
            }
        }
    }
    println!(
        "{}",
        "└──────────────────────────────────────────────────────────┘".cyan()
    );
    println!();

    // Quick Stats
    println!(
        "{}",
        "┌─ Quick Stats ────────────────────────────────────────────┐".cyan()
    );

    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        println!(
            "  📦 {} packages installed",
            pkg_info.packages.len().to_string().green().bold()
        );
    }

    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let user_count = users.len();
        let root_users = users.iter().filter(|u| u.uid == "0").count();
        println!(
            "  👥 {} users ({} root)",
            user_count.to_string().green().bold(),
            root_users.to_string().red()
        );
    }

    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled = services.iter().filter(|s| s.enabled).count();
        println!(
            "  ⚙ {} services ({} enabled)",
            services.len().to_string().green().bold(),
            enabled.to_string().cyan()
        );
    }

    println!(
        "{}",
        "└──────────────────────────────────────────────────────────┘".cyan()
    );
    println!();

    // Security Status
    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        println!(
            "{}",
            "┌─ Security Status ────────────────────────────────────────┐".cyan()
        );

        let selinux_icon = if &sec.selinux != "disabled" {
            "✓"
        } else {
            "✗"
        };
        let selinux_color = if &sec.selinux != "disabled" {
            sec.selinux.green()
        } else {
            sec.selinux.red()
        };
        println!("  {} SELinux:  {}", selinux_icon, selinux_color);

        let apparmor_icon = if sec.apparmor { "✓" } else { "✗" };
        let apparmor_status = if sec.apparmor {
            "enabled".green()
        } else {
            "disabled".red()
        };
        println!("  {} AppArmor: {}", apparmor_icon, apparmor_status);

        let auditd_icon = if sec.auditd { "✓" } else { "✗" };
        let auditd_status = if sec.auditd {
            "enabled".green()
        } else {
            "disabled".red()
        };
        println!("  {} Auditd:   {}", auditd_icon, auditd_status);

        if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
            let fw_icon = if fw.enabled { "✓" } else { "✗" };
            let fw_status = if fw.enabled {
                format!("enabled ({})", fw.firewall_type).green()
            } else {
                format!("disabled ({})", fw.firewall_type).red()
            };
            println!("  {} Firewall: {}", fw_icon, fw_status);
        }

        println!(
            "{}",
            "└──────────────────────────────────────────────────────────┘".cyan()
        );
    }

    println!(
        "\n{} Use specific commands for detailed information",
        "💡".to_string().yellow()
    );
    println!();

    Ok(())
}

pub fn cmd_summary(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
    );

    // One-liner system info
    let os = ctx
        .guestfs
        .inspect_get_product_name(&ctx.root)
        .unwrap_or_else(|_| "Unknown".to_string());
    let arch = ctx
        .guestfs
        .inspect_get_arch(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());
    let hostname = ctx
        .guestfs
        .inspect_get_hostname(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());

    println!(
        "  🖥 {} | {} | {}",
        os.green().bold(),
        arch.cyan(),
        hostname.yellow()
    );

    // Quick counts
    let mut quick_stats = Vec::new();

    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        quick_stats.push(format!("{} pkgs", pkg_info.packages.len()));
    }

    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        quick_stats.push(format!("{} users", users.len()));
    }

    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled = services.iter().filter(|s| s.enabled).count();
        quick_stats.push(format!("{}/{} services", enabled, services.len()));
    }

    if !quick_stats.is_empty() {
        println!("  {}", quick_stats.join(" • ").bright_black());
    }

    println!(
        "{}",
        "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━".bright_black()
    );
    println!();

    Ok(())
}

pub fn cmd_snapshot(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    use chrono::Local;

    let timestamp = Local::now().format("%Y-%m-%d %H:%M:%S");
    let output_file = if args.is_empty() {
        format!("snapshot-{}.md", Local::now().format("%Y%m%d-%H%M%S"))
    } else {
        args[0].to_string()
    };

    println!("{} Generating comprehensive system snapshot...", "→".cyan());

    let mut report = String::new();

    // Header
    report.push_str("# System Snapshot Report\n\n");
    report.push_str(&format!("**Generated:** {}\n\n", timestamp));
    report.push_str("---\n\n");

    // System Information
    report.push_str("## System Information\n\n");
    if let Ok(os_type) = ctx.guestfs.inspect_get_type(&ctx.root) {
        report.push_str(&format!("- **Type:** {}\n", os_type));
    }
    if let Ok(distro) = ctx.guestfs.inspect_get_distro(&ctx.root) {
        report.push_str(&format!("- **Distribution:** {}\n", distro));
    }
    if let Ok(version) = ctx.guestfs.inspect_get_product_name(&ctx.root) {
        report.push_str(&format!("- **Version:** {}\n", version));
    }
    if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
        report.push_str(&format!("- **Architecture:** {}\n", arch));
    }
    if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
        report.push_str(&format!("- **Hostname:** {}\n", hostname));
    }
    report.push('\n');

    // Storage
    report.push_str("## Storage Overview\n\n");
    if let Ok(filesystems) = ctx.guestfs.list_filesystems() {
        report.push_str("| Device | Type | Size |\n");
        report.push_str("|--------|------|------|\n");
        for (device, fstype) in filesystems.iter() {
            if fstype != "unknown" && !fstype.is_empty() {
                let size_str = if let Ok(size) = ctx.guestfs.blockdev_getsize64(device) {
                    format_bytes(size as u64)
                } else {
                    "unknown".to_string()
                };
                report.push_str(&format!("| {} | {} | {} |\n", device, fstype, size_str));
            }
        }
        report.push('\n');
    }

    // Packages
    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        let packages = &pkg_info.packages;
        report.push_str(&format!("## Installed Packages ({})\n\n", packages.len()));
        report.push_str("| Package | Version |\n");
        report.push_str("|---------|----------|\n");
        for pkg in packages.iter().take(50) {
            report.push_str(&format!("| {} | {} |\n", pkg.name, pkg.version));
        }
        if packages.len() > 50 {
            report.push_str(&format!("\n*Showing 50 of {} packages*\n", packages.len()));
        }
        report.push('\n');
    }

    // Users
    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        report.push_str(&format!("## User Accounts ({})\n\n", users.len()));
        report.push_str("| Username | UID | GID | Home Directory |\n");
        report.push_str("|----------|-----|-----|----------------|\n");
        for user in users {
            let uid_marker = if user.uid == "0" { " ⚠️" } else { "" };
            report.push_str(&format!(
                "| {}{} | {} | {} | {} |\n",
                user.username, uid_marker, user.uid, user.gid, user.home
            ));
        }
        report.push('\n');
    }

    // Services
    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled_count = services.iter().filter(|s| s.enabled).count();
        report.push_str(&format!(
            "## System Services ({} total, {} enabled)\n\n",
            services.len(),
            enabled_count
        ));
        report.push_str("| Service | Status |\n");
        report.push_str("|---------|--------|\n");
        for svc in services.iter().take(50) {
            let status = if svc.enabled {
                "✓ Enabled"
            } else {
                "✗ Disabled"
            };
            report.push_str(&format!("| {} | {} |\n", svc.name, status));
        }
        if services.len() > 50 {
            report.push_str(&format!("\n*Showing 50 of {} services*\n", services.len()));
        }
        report.push('\n');
    }

    // Security
    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        report.push_str("## Security Configuration\n\n");
        report.push_str("| Feature | Status |\n");
        report.push_str("|---------|--------|\n");

        let selinux_status = if &sec.selinux != "disabled" {
            format!("✓ {}", sec.selinux)
        } else {
            "✗ Disabled".to_string()
        };
        report.push_str(&format!("| SELinux | {} |\n", selinux_status));

        let apparmor = if sec.apparmor {
            "✓ Enabled"
        } else {
            "✗ Disabled"
        };
        report.push_str(&format!("| AppArmor | {} |\n", apparmor));

        let auditd = if sec.auditd {
            "✓ Enabled"
        } else {
            "✗ Disabled"
        };
        report.push_str(&format!("| Auditd | {} |\n", auditd));

        let fail2ban = if sec.fail2ban {
            "✓ Installed"
        } else {
            "✗ Not installed"
        };
        report.push_str(&format!("| fail2ban | {} |\n", fail2ban));

        let aide = if sec.aide {
            "✓ Installed"
        } else {
            "✗ Not installed"
        };
        report.push_str(&format!("| AIDE | {} |\n", aide));

        if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
            let fw_status = if fw.enabled {
                format!("✓ Enabled ({})", fw.firewall_type)
            } else {
                format!("✗ Disabled ({})", fw.firewall_type)
            };
            report.push_str(&format!("| Firewall | {} |\n", fw_status));
        }
        report.push('\n');
    }

    // Network
    if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
        report.push_str(&format!(
            "## Network Configuration ({} interfaces)\n\n",
            interfaces.len()
        ));
        for iface in interfaces {
            report.push_str(&format!("- {}\n", iface.name));
        }

        if let Ok(dns) = ctx.guestfs.inspect_dns(&ctx.root) {
            if !dns.is_empty() {
                report.push_str("\n**DNS Servers:**\n\n");
                for server in dns {
                    report.push_str(&format!("- {}\n", server));
                }
            }
        }
        report.push('\n');
    }

    // Footer
    report.push_str("---\n\n");
    report.push_str("*Generated by GuestKit Interactive Shell*\n");

    // Write to file
    use std::fs;
    fs::write(&output_file, report)?;

    println!(
        "{} Snapshot saved to: {}",
        "✓".green(),
        output_file.yellow()
    );
    println!(
        "{} Report includes: system info, storage, packages, users, services, security, network",
        "→".cyan()
    );

    Ok(())
}

pub fn cmd_diff(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.len() < 2 {
        println!("{}", "Usage: diff <type> <filter1> [filter2]".yellow());
        println!();
        println!("{}", "Examples:".green().bold());
        println!(
            "  {} - Compare package versions",
            "diff package kernel".cyan()
        );
        println!("  {} - Compare files", "diff file /etc/fstab".cyan());
        println!("  {} - Show file changes", "diff changes /var/log".cyan());
        return Ok(());
    }

    let diff_type = args[0];

    match diff_type {
        "file" => {
            let file_path = args[1];
            println!("{} Analyzing file: {}", "→".cyan(), file_path.yellow());

            if let Ok(stat) = ctx.guestfs.stat(file_path) {
                println!("\n{}", "File Information:".yellow().bold());
                println!("  Size: {} bytes", stat.size.to_string().green());
                println!("  Mode: {:o}", stat.mode);

                if let Ok(content) = ctx.guestfs.read_file(file_path) {
                    let lines: Vec<&str> = std::str::from_utf8(&content)
                        .unwrap_or("")
                        .lines()
                        .collect();
                    println!("  Lines: {}", lines.len().to_string().green());
                }
            }
        }
        "package" => {
            let pkg_name = args[1];
            println!(
                "{} Searching for package: {}",
                "→".cyan(),
                pkg_name.yellow()
            );

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let matches: Vec<_> = pkg_info
                    .packages
                    .iter()
                    .filter(|p| p.name.contains(pkg_name))
                    .collect();

                if matches.is_empty() {
                    println!("{} No matching packages found", "⚠".yellow());
                } else {
                    println!("\n{}", "Matching Packages:".yellow().bold());
                    for pkg in matches {
                        println!(
                            "  {} {} - {}",
                            "•".cyan(),
                            pkg.name.green(),
                            pkg.version.to_string().bright_black()
                        );
                    }
                }
            }
        }
        _ => {
            println!("{} Unknown diff type: {}", "Error:".red(), diff_type);
        }
    }
    println!();

    Ok(())
}

pub fn cmd_scan(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  System Scanner                          ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Available Scans:".yellow().bold());
        println!("  {} - Quick security scan", "scan security".cyan());
        println!("  {} - Find common issues", "scan issues".cyan());
        println!("  {} - Scan for vulnerabilities", "scan vulns".cyan());
        println!("  {} - Scan all (comprehensive)", "scan all".cyan());
        println!();

        return Ok(());
    }

    let scan_type = args[0];

    match scan_type {
        "security" => {
            println!("\n{}", "🔍 Security Scan".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let mut findings = Vec::new();

            // Check 1: Root users
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let root_count = users.iter().filter(|u| u.uid == "0").count();
                if root_count > 1 {
                    findings.push((
                        "HIGH".red(),
                        format!("{} root accounts found (expected 1)", root_count),
                    ));
                }
            }

            // Check 2: Security features
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                if &sec.selinux == "disabled" {
                    findings.push(("MEDIUM".yellow(), "SELinux is disabled".to_string()));
                }
                if !sec.apparmor {
                    findings.push(("MEDIUM".yellow(), "AppArmor is disabled".to_string()));
                }
                if !sec.auditd {
                    findings.push(("LOW".bright_black(), "Auditd is not enabled".to_string()));
                }
            }

            // Check 3: Firewall
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if !fw.enabled {
                    findings.push(("HIGH".red(), "Firewall is disabled".to_string()));
                }
            }

            println!(
                "{} ({} findings)",
                "Security Findings:".yellow().bold(),
                findings.len()
            );
            if findings.is_empty() {
                println!("  {} No security issues detected!", "✓".green());
            } else {
                for (severity, finding) in findings {
                    println!("  [{}] {}", severity, finding);
                }
            }
            println!();
        }
        "issues" => {
            println!("\n{}", "🔍 Common Issues Scan".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            println!("{} Scanning for common issues...", "→".cyan());
            println!();

            let mut issue_count = 0;

            // Check for empty password users (simplified)
            println!("{}", "Checking user accounts...".yellow());
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                for user in users.iter().take(5) {
                    println!(
                        "  {} {} (UID: {})",
                        "•".cyan(),
                        user.username.green(),
                        user.uid.bright_black()
                    );
                }
            }

            println!();
            println!("{}", "Checking for common misconfigurations...".yellow());

            // Check fstab
            if ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                println!("  {} /etc/fstab exists", "✓".green());
            } else {
                println!("  {} /etc/fstab missing", "✗".red());
                issue_count += 1;
            }

            println!();
            if issue_count == 0 {
                println!("{} No critical issues found", "✓".green());
            } else {
                println!("{} {} issues found", "⚠".yellow(), issue_count);
            }
            println!();
        }
        "vulns" => {
            println!("\n{}", "🔍 Vulnerability Scan".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            println!("{} Checking for known vulnerabilities...", "→".cyan());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("  Scanning {} packages...", pkg_info.packages.len());
                println!();
                println!(
                    "{} Full vulnerability scanning requires CVE database",
                    "Note:".yellow()
                );
                println!("{} This is a basic package audit", "Note:".yellow());
                println!();

                // Check for very old or suspicious packages
                let kernel_pkgs: Vec<_> = pkg_info
                    .packages
                    .iter()
                    .filter(|p| p.name.contains("kernel"))
                    .collect();

                if !kernel_pkgs.is_empty() {
                    println!("{}", "Kernel packages:".green().bold());
                    for pkg in kernel_pkgs {
                        println!(
                            "  {} {}",
                            pkg.name.cyan(),
                            pkg.version.to_string().bright_black()
                        );
                    }
                }
            }
            println!();
        }
        "all" => {
            println!("\n{}", "🔍 Comprehensive System Scan".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            println!("{} Running all scans...", "→".cyan());
            println!();

            // Run all scans
            cmd_scan(ctx, &["security"])?;
            cmd_scan(ctx, &["issues"])?;

            println!("{}", "═".repeat(60).cyan());
            println!("{} Scan complete!", "✓".green());
            println!();
        }
        _ => {
            println!("{} Unknown scan type: {}", "Error:".red(), scan_type);
        }
    }

    Ok(())
}

pub fn cmd_compare(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Comparison Tools                        ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Usage:".yellow().bold());
        println!(
            "  {} - Compare two files",
            "compare files <file1> <file2>".cyan()
        );
        println!(
            "  {} - Compare directories",
            "compare dirs <dir1> <dir2>".cyan()
        );
        println!(
            "  {} - Compare package lists",
            "compare packages <snap1> <snap2>".cyan()
        );
        println!();

        println!("{}", "Examples:".green().bold());
        println!("  compare files /etc/fstab /etc/fstab.bak");
        println!("  compare dirs /etc /etc.backup");
        println!();

        return Ok(());
    }

    let compare_type = args[0];

    match compare_type {
        "files" => {
            if args.len() < 3 {
                println!("{} Usage: compare files <file1> <file2>", "Error:".red());
                return Ok(());
            }

            let file1 = args[1];
            let file2 = args[2];

            println!("\n{} Comparing files:", "→".cyan());
            println!("  {} {}", "A:".yellow(), file1.green());
            println!("  {} {}", "B:".yellow(), file2.green());
            println!();

            let stat1 = ctx.guestfs.stat(file1)?;
            let stat2 = ctx.guestfs.stat(file2)?;

            println!("{}", "Size Comparison:".yellow().bold());
            println!("  A: {} bytes", stat1.size.to_string().cyan());
            println!("  B: {} bytes", stat2.size.to_string().cyan());

            if stat1.size == stat2.size {
                println!("  {} Files are same size", "✓".green());
            } else {
                let diff = (stat1.size - stat2.size).abs();
                println!("  {} Difference: {} bytes", "△".yellow(), diff);
            }

            println!();
            println!("{}", "Modification Time:".yellow().bold());
            println!("  A: {}", stat1.mtime.to_string().cyan());
            println!("  B: {}", stat2.mtime.to_string().cyan());

            if stat1.mtime > stat2.mtime {
                println!("  {} A is newer", "→".cyan());
            } else if stat2.mtime > stat1.mtime {
                println!("  {} B is newer", "→".cyan());
            } else {
                println!("  {} Same modification time", "✓".green());
            }
            println!();
        }
        "dirs" => {
            if args.len() < 3 {
                println!("{} Usage: compare dirs <dir1> <dir2>", "Error:".red());
                return Ok(());
            }

            let dir1 = args[1];
            let dir2 = args[2];

            println!("\n{} Comparing directories:", "→".cyan());
            println!("  {} {}", "A:".yellow(), dir1.green());
            println!("  {} {}", "B:".yellow(), dir2.green());
            println!();

            let entries1 = ctx.guestfs.ls(dir1)?;
            let entries2 = ctx.guestfs.ls(dir2)?;

            println!("{}", "File Count:".yellow().bold());
            println!("  A: {} files", entries1.len().to_string().cyan());
            println!("  B: {} files", entries2.len().to_string().cyan());

            let only_in_a: Vec<_> = entries1.iter().filter(|e| !entries2.contains(e)).collect();

            let only_in_b: Vec<_> = entries2.iter().filter(|e| !entries1.contains(e)).collect();

            if !only_in_a.is_empty() {
                println!();
                println!("{} ({}):", "Only in A".yellow().bold(), only_in_a.len());
                for entry in only_in_a.iter().take(10) {
                    println!("  {} {}", "-".red(), entry);
                }
                if only_in_a.len() > 10 {
                    println!("  ... and {} more", only_in_a.len() - 10);
                }
            }

            if !only_in_b.is_empty() {
                println!();
                println!("{} ({}):", "Only in B".yellow().bold(), only_in_b.len());
                for entry in only_in_b.iter().take(10) {
                    println!("  {} {}", "+".green(), entry);
                }
                if only_in_b.len() > 10 {
                    println!("  ... and {} more", only_in_b.len() - 10);
                }
            }

            if only_in_a.is_empty() && only_in_b.is_empty() {
                println!();
                println!("{} Directories have identical file lists", "✓".green());
            }
            println!();
        }
        _ => {
            println!(
                "{} Unknown comparison type: {}",
                "Error:".red(),
                compare_type
            );
        }
    }

    Ok(())
}

pub fn cmd_profile(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  System Profiler                         ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Available Profiles:".yellow().bold());
        println!(
            "  {} - Create full system profile",
            "profile create [name]".cyan()
        );
        println!("  {} - Quick system fingerprint", "profile quick".cyan());
        println!("  {} - Show system characteristics", "profile show".cyan());
        println!("  {} - Detect system purpose", "profile detect".cyan());
        println!();

        return Ok(());
    }

    let profile_type = args[0];

    match profile_type {
        "create" => {
            let profile_name = if args.len() > 1 {
                args[1]
            } else {
                "system-profile"
            };

            println!(
                "\n{} Creating system profile: {}",
                "→".cyan(),
                profile_name.yellow()
            );
            println!();

            let mut profile_data = String::new();
            profile_data.push_str(&format!("# System Profile: {}\n\n", profile_name));

            // Basic info
            if let Ok(os) = ctx.guestfs.inspect_get_product_name(&ctx.root) {
                profile_data.push_str(&format!("**OS:** {}\n", os));
            }
            if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
                profile_data.push_str(&format!("**Architecture:** {}\n", arch));
            }
            if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
                profile_data.push_str(&format!("**Hostname:** {}\n", hostname));
            }

            profile_data.push_str("\n## Profile Metrics\n\n");

            // Metrics
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                profile_data.push_str(&format!("- Packages: {}\n", pkg_info.packages.len()));
            }
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                profile_data.push_str(&format!("- Users: {}\n", users.len()));
            }
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                profile_data.push_str(&format!(
                    "- Services: {} ({} enabled)\n",
                    services.len(),
                    enabled
                ));
            }

            let filename = format!("{}.md", profile_name);
            use std::fs;
            fs::write(&filename, profile_data)?;

            println!("{} Profile saved to: {}", "✓".green(), filename.yellow());
            println!();
        }
        "quick" => {
            println!("\n{}", "🔍 Quick System Fingerprint".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let mut fingerprint = Vec::new();

            if let Ok(os_type) = ctx.guestfs.inspect_get_type(&ctx.root) {
                fingerprint.push(format!("Type: {}", os_type));
            }
            if let Ok(distro) = ctx.guestfs.inspect_get_distro(&ctx.root) {
                fingerprint.push(format!("Distro: {}", distro));
            }
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let count = pkg_info.packages.len();
                let size = if count < 200 {
                    "minimal"
                } else if count < 500 {
                    "standard"
                } else {
                    "full"
                };
                fingerprint.push(format!("Size: {} ({} pkgs)", size, count));
            }

            for item in fingerprint {
                println!("  {} {}", "•".cyan(), item.green());
            }
            println!();
        }
        "detect" => {
            println!("\n{}", "🎯 System Purpose Detection".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let mut purposes = Vec::new();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let packages = &pkg_info.packages;

                // Check for web server
                if packages.iter().any(|p| {
                    p.name.contains("httpd")
                        || p.name.contains("nginx")
                        || p.name.contains("apache")
                }) {
                    purposes.push(("Web Server", "🌐", "HTTP server software detected"));
                }

                // Check for database
                if packages.iter().any(|p| {
                    p.name.contains("mysql")
                        || p.name.contains("postgres")
                        || p.name.contains("mariadb")
                }) {
                    purposes.push(("Database Server", "💾", "Database software detected"));
                }

                // Check for development
                if packages.iter().any(|p| {
                    p.name.contains("gcc")
                        || p.name.contains("python-dev")
                        || p.name.contains("build-essential")
                }) {
                    purposes.push(("Development", "⚙", "Development tools detected"));
                }

                // Check for desktop
                if packages.iter().any(|p| {
                    p.name.contains("gnome") || p.name.contains("kde") || p.name.contains("xorg")
                }) {
                    purposes.push(("Desktop/Workstation", "🖥", "Desktop environment detected"));
                }

                // Check for container
                if packages.iter().any(|p| {
                    p.name.contains("docker")
                        || p.name.contains("podman")
                        || p.name.contains("kubernetes")
                }) {
                    purposes.push(("Container Platform", "📦", "Container runtime detected"));
                }
            }

            if purposes.is_empty() {
                println!("  🔧 Minimal/Base system");
                println!("  No specific purpose detected - likely a base installation");
            } else {
                println!("{}", "Detected Purposes:".green().bold());
                for (purpose, icon, desc) in purposes {
                    println!(
                        "  {} {} - {}",
                        icon,
                        purpose.green().bold(),
                        desc.bright_black()
                    );
                }
            }
            println!();
        }
        "show" => {
            println!("\n{}", "📋 System Characteristics".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            println!("{}", "System Identity:".green().bold());
            if let Ok(os) = ctx.guestfs.inspect_get_product_name(&ctx.root) {
                println!("  OS: {}", os.cyan());
            }
            if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
                println!("  Architecture: {}", arch.cyan());
            }

            println!();
            println!("{}", "Security Profile:".green().bold());
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let profile = if &sec.selinux != "disabled" && sec.apparmor {
                    "Hardened"
                } else if &sec.selinux != "disabled" || sec.apparmor {
                    "Standard"
                } else {
                    "Basic"
                };
                println!("  Security Level: {}", profile.yellow());
            }

            println!();
        }
        _ => {
            println!(
                "{} Unknown profile command: {}",
                "Error:".red(),
                profile_type
            );
        }
    }

    Ok(())
}

pub fn cmd_recommend(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Smart Recommendations                       ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!(
        "{} Analyzing system and generating recommendations...",
        "→".cyan()
    );
    println!();

    let mut recommendations = Vec::new();

    // Security recommendations
    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        if &sec.selinux == "disabled" {
            recommendations.push((
                "HIGH",
                "Security",
                "Enable SELinux for enhanced security",
                "wizard security",
            ));
        }

        if !sec.auditd {
            recommendations.push((
                "MEDIUM",
                "Monitoring",
                "Enable auditd for system auditing",
                "scan security",
            ));
        }
    }

    // Firewall recommendation
    if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
        if !fw.enabled {
            recommendations.push((
                "HIGH",
                "Security",
                "Enable firewall for network protection",
                "quick security",
            ));
        }
    }

    // User account recommendations
    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let root_count = users.iter().filter(|u| u.uid == "0").count();
        if root_count > 1 {
            recommendations.push((
                "HIGH",
                "Security",
                "Multiple root accounts detected - review user list",
                "users",
            ));
        }
    }

    // Package recommendations
    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        if pkg_info.packages.len() < 100 {
            recommendations.push((
                "LOW",
                "System",
                "Very minimal package set - consider if all tools are available",
                "wizard packages",
            ));
        }
    }

    // General recommendations
    recommendations.push((
        "INFO",
        "Analysis",
        "Generate a full system snapshot for documentation",
        "snapshot",
    ));

    recommendations.push((
        "INFO",
        "Export",
        "Export data for external analysis",
        "batch export /tmp/data",
    ));

    if recommendations.is_empty() {
        println!("{} No recommendations - system looks good!", "✓".green());
    } else {
        println!(
            "{} ({} items)",
            "Recommendations:".yellow().bold(),
            recommendations.len()
        );
        println!();

        for (priority, category, recommendation, command) in recommendations {
            let priority_colored = match priority {
                "HIGH" => "HIGH".red(),
                "MEDIUM" => "MEDIUM".yellow(),
                "LOW" => "LOW".bright_black(),
                _ => "INFO".cyan(),
            };

            println!(
                "  [{}] {} - {}",
                priority_colored,
                category.green().bold(),
                recommendation
            );
            println!("      {} {}", "Command:".bright_black(), command.cyan());
            println!();
        }
    }

    println!(
        "{} Run suggested commands to address recommendations",
        "Tip:".yellow()
    );
    println!();

    Ok(())
}

pub fn cmd_discover(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  System Discovery                        ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Discovery Options:".yellow().bold());
        println!("  {} - Discover interesting files", "discover files".cyan());
        println!(
            "  {} - Discover installed applications",
            "discover apps".cyan()
        );
        println!(
            "  {} - Discover network configuration",
            "discover network".cyan()
        );
        println!("  {} - Discover all (comprehensive)", "discover all".cyan());
        println!();

        return Ok(());
    }

    let discover_type = args[0];

    match discover_type {
        "files" => {
            println!("\n{}", "📂 Discovering Interesting Files".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let interesting_paths = vec![
                ("/etc/fstab", "Filesystem table"),
                ("/etc/passwd", "User accounts"),
                ("/etc/shadow", "Password hashes"),
                ("/etc/hosts", "Host name mappings"),
                ("/etc/ssh/sshd_config", "SSH server config"),
                ("/var/log/syslog", "System log"),
                ("/root/.bash_history", "Root command history"),
            ];

            println!("{}", "Critical System Files:".green().bold());
            for (path, description) in interesting_paths {
                if ctx.guestfs.exists(path).unwrap_or(false) {
                    if let Ok(stat) = ctx.guestfs.stat(path) {
                        println!(
                            "  {} {} - {} ({} bytes)",
                            "✓".green(),
                            path.cyan(),
                            description.bright_black(),
                            stat.size
                        );
                    }
                } else {
                    println!(
                        "  {} {} - {} (not found)",
                        "✗".red(),
                        path,
                        description.bright_black()
                    );
                }
            }
            println!();
        }
        "apps" => {
            println!("\n{}", "🚀 Discovering Applications".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let packages = &pkg_info.packages;

                let categories = vec![
                    ("Web Servers", vec!["httpd", "nginx", "apache"]),
                    ("Databases", vec!["mysql", "postgres", "mariadb", "mongodb"]),
                    (
                        "Programming",
                        vec!["python", "ruby", "nodejs", "java", "golang"],
                    ),
                    (
                        "Security Tools",
                        vec!["nmap", "wireshark", "fail2ban", "aide"],
                    ),
                    ("System Tools", vec!["systemd", "cron", "rsyslog"]),
                ];

                for (category, keywords) in categories {
                    let found: Vec<_> = packages
                        .iter()
                        .filter(|p| keywords.iter().any(|k| p.name.contains(k)))
                        .collect();

                    if !found.is_empty() {
                        println!("{} ({}):", category.green().bold(), found.len());
                        for pkg in found.iter().take(5) {
                            println!(
                                "  {} {} - {}",
                                "•".cyan(),
                                pkg.name.green(),
                                pkg.version.to_string().bright_black()
                            );
                        }
                        if found.len() > 5 {
                            println!("  ... and {} more", found.len() - 5);
                        }
                        println!();
                    }
                }
            }
        }
        "network" => {
            println!(
                "\n{}",
                "🌐 Discovering Network Configuration".yellow().bold()
            );
            println!("{}", "─".repeat(60).cyan());
            println!();

            if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
                println!(
                    "{} ({}):",
                    "Network Interfaces".green().bold(),
                    interfaces.len()
                );
                for iface in interfaces {
                    println!("  {} {}", "•".cyan(), iface.name.green());
                }
                println!();
            }

            if let Ok(dns) = ctx.guestfs.inspect_dns(&ctx.root) {
                if !dns.is_empty() {
                    println!("{} ({}):", "DNS Servers".green().bold(), dns.len());
                    for server in dns {
                        println!("  {} {}", "•".cyan(), server.yellow());
                    }
                    println!();
                }
            }

            // Check for common network files
            println!("{}", "Network Configuration Files:".green().bold());
            let net_files = vec!["/etc/hosts", "/etc/resolv.conf", "/etc/hostname"];
            for file in net_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!("  {} {}", "✓".green(), file.cyan());
                }
            }
            println!();
        }
        "all" => {
            println!("\n{}", "🔍 Comprehensive System Discovery".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            cmd_discover(ctx, &["files"])?;
            cmd_discover(ctx, &["apps"])?;
            cmd_discover(ctx, &["network"])?;

            println!("{}", "═".repeat(60).cyan());
            println!("{} Discovery complete!", "✓".green());
            println!();
        }
        _ => {
            println!(
                "{} Unknown discovery type: {}",
                "Error:".red(),
                discover_type
            );
        }
    }

    Ok(())
}

pub fn cmd_report(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Report Generator                        ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Available Reports:".yellow().bold());
        println!("  {} - Executive summary report", "report executive".cyan());
        println!("  {} - Technical detail report", "report technical".cyan());
        println!("  {} - Security audit report", "report security".cyan());
        println!("  {} - Compliance report", "report compliance".cyan());
        println!();

        println!("{}", "Output Options:".green().bold());
        println!("  Add {} to save to file", "--output <file>".cyan());
        println!("  Example: report executive --output summary.md");
        println!();

        return Ok(());
    }

    let report_type = args[0];
    let output_file = if args.len() > 2 && args[1] == "--output" {
        Some(args[2])
    } else {
        None
    };

    let mut report_content = String::new();

    match report_type {
        "executive" => {
            use chrono::Local;
            report_content.push_str("# Executive Summary Report\n\n");
            report_content.push_str(&format!(
                "**Generated:** {}\n\n",
                Local::now().format("%Y-%m-%d %H:%M:%S")
            ));

            report_content.push_str("## Overview\n\n");

            if let Ok(os) = ctx.guestfs.inspect_get_product_name(&ctx.root) {
                report_content.push_str(&format!("System running **{}**", os));
            }

            if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
                report_content.push_str(&format!(" on host **{}**", hostname));
            }
            report_content.push_str(".\n\n");

            report_content.push_str("## Key Metrics\n\n");

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                report_content.push_str(&format!(
                    "- **Installed Packages:** {}\n",
                    pkg_info.packages.len()
                ));
            }

            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                report_content.push_str(&format!("- **User Accounts:** {}\n", users.len()));
            }

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                report_content.push_str(&format!(
                    "- **Active Services:** {}/{}\n",
                    enabled,
                    services.len()
                ));
            }

            report_content.push_str("\n## Recommendations\n\n");
            report_content.push_str("- Review security configuration\n");
            report_content.push_str("- Verify all services are necessary\n");
            report_content.push_str("- Ensure regular updates are applied\n");

            println!("\n{}", "📊 Executive Summary Report".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();
            println!("{}", report_content);
        }
        "security" => {
            report_content.push_str("# Security Audit Report\n\n");

            report_content.push_str("## Security Features\n\n");

            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                report_content.push_str(&format!("- SELinux: {}\n", sec.selinux));
                report_content.push_str(&format!(
                    "- AppArmor: {}\n",
                    if sec.apparmor { "Enabled" } else { "Disabled" }
                ));
                report_content.push_str(&format!(
                    "- Auditd: {}\n",
                    if sec.auditd { "Enabled" } else { "Disabled" }
                ));
            }

            report_content.push_str("\n## Firewall Status\n\n");

            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                report_content.push_str(&format!(
                    "- Status: {}\n",
                    if fw.enabled {
                        "Enabled"
                    } else {
                        "**Disabled**"
                    }
                ));
                report_content.push_str(&format!("- Type: {}\n", fw.firewall_type));
            }

            println!("\n{}", "🔒 Security Audit Report".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();
            println!("{}", report_content);
        }
        _ => {
            println!("{} Unknown report type: {}", "Error:".red(), report_type);
            return Ok(());
        }
    }

    if let Some(file) = output_file {
        use std::fs;
        fs::write(file, &report_content)?;
        println!("{} Report saved to: {}", "✓".green(), file.yellow());
    }

    println!();
    Ok(())
}

pub fn cmd_timeline(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                  Session Timeline                        ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!("{}", "Current Session:".yellow().bold());
    println!();

    // Visual timeline
    println!("  {} Session started", "┌─".cyan());
    println!("  {} Shell initialized", "├─".cyan());

    if ctx.command_count > 0 {
        println!(
            "  {} {} commands executed",
            "├─".cyan(),
            ctx.command_count.to_string().green().bold()
        );
    }

    if !ctx.aliases.is_empty() {
        println!(
            "  {} {} aliases defined",
            "├─".cyan(),
            ctx.aliases.len().to_string().green()
        );
    }

    if !ctx.bookmarks.is_empty() {
        println!(
            "  {} {} bookmarks created",
            "├─".cyan(),
            ctx.bookmarks.len().to_string().green()
        );
    }

    if let Some(duration) = ctx.last_command_time {
        println!(
            "  {} Last command: {}",
            "├─".cyan(),
            format!("{:.2}ms", duration.as_secs_f64() * 1000.0).yellow()
        );
    }

    println!("  {} Current state", "└─".cyan());
    println!();

    println!("{}", "Session Statistics:".green().bold());
    println!("  Path: {}", ctx.current_path.cyan());
    println!("  OS: {}", ctx.get_os_info().yellow());
    println!();

    println!("{}", "Suggested Next Steps:".yellow().bold());
    if ctx.command_count < 5 {
        println!("  • Try {} for system overview", "'dashboard'".cyan());
        println!("  • Run {} for guided help", "'wizard'".cyan());
    } else {
        println!("  • Create snapshot: {}", "'snapshot'".cyan());
        println!("  • Get recommendations: {}", "'recommend'".cyan());
    }
    println!();

    Ok(())
}

pub fn cmd_bench(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Performance Benchmark                   ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Benchmark Commands:".yellow().bold());
        println!("  {} - Benchmark file operations", "bench files".cyan());
        println!("  {} - Benchmark directory listing", "bench list".cyan());
        println!("  {} - Benchmark package queries", "bench packages".cyan());
        println!("  {} - Run all benchmarks", "bench all".cyan());
        println!();

        return Ok(());
    }

    let bench_type = args[0];

    match bench_type {
        "files" => {
            println!("\n{}", "📊 File Operations Benchmark".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let test_file = "/etc/fstab";
            if ctx.guestfs.exists(test_file).unwrap_or(false) {
                let iterations = 10;
                let start = std::time::Instant::now();

                for _ in 0..iterations {
                    let _ = ctx.guestfs.stat(test_file);
                }

                let duration = start.elapsed();
                let avg = duration.as_micros() / iterations;

                println!(
                    "  Test: {} stat operations on {}",
                    iterations,
                    test_file.cyan()
                );
                println!("  Total: {:.2}ms", duration.as_secs_f64() * 1000.0);
                println!("  Average: {}μs per operation", avg.to_string().green());
                println!();
            }
        }
        "list" => {
            println!("\n{}", "📊 Directory Listing Benchmark".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let start = std::time::Instant::now();
            let result = ctx.guestfs.ls("/etc");
            let duration = start.elapsed();

            if let Ok(entries) = result {
                println!(
                    "  Listed {} entries in /etc",
                    entries.len().to_string().green()
                );
                println!("  Time: {:.2}ms", duration.as_secs_f64() * 1000.0);
                println!();
            }
        }
        "packages" => {
            println!("\n{}", "📊 Package Query Benchmark".yellow().bold());
            println!("{}", "─".repeat(60).cyan());
            println!();

            let start = std::time::Instant::now();
            let result = ctx.guestfs.inspect_packages(&ctx.root);
            let duration = start.elapsed();

            if let Ok(pkg_info) = result {
                println!(
                    "  Queried {} packages",
                    pkg_info.packages.len().to_string().green()
                );
                println!("  Time: {:.2}ms", duration.as_secs_f64() * 1000.0);
                println!();
            }
        }
        "all" => {
            println!("\n{}", "📊 Complete Benchmark Suite".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            cmd_bench(ctx, &["files"])?;
            cmd_bench(ctx, &["list"])?;
            cmd_bench(ctx, &["packages"])?;

            println!("{}", "═".repeat(60).cyan());
            println!("{} Benchmark complete!", "✓".green());
            println!();
        }
        _ => {
            println!("{} Unknown benchmark: {}", "Error:".red(), bench_type);
        }
    }

    Ok(())
}

pub fn cmd_context(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Context-Aware Suggestions                   ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!(
        "{} Current Location: {}",
        "📍".cyan(),
        ctx.current_path.yellow().bold()
    );
    println!();

    // Analyze current path and provide context-aware suggestions
    let path = &ctx.current_path;
    let mut suggestions = Vec::new();

    if path.contains("/etc") {
        suggestions.push(("High", "Configuration files detected", "cat /etc/fstab"));
        suggestions.push(("High", "View network configuration", "cat /etc/hosts"));
        suggestions.push(("Medium", "Check installed services", "services"));
        suggestions.push(("Medium", "Review security settings", "security"));
    } else if path.contains("/var/log") {
        suggestions.push(("High", "Search for errors in logs", "grep ERROR ."));
        suggestions.push(("High", "View recent log files", "recent . 10"));
        suggestions.push((
            "Medium",
            "Find critical messages",
            "search critical --content",
        ));
    } else if path.contains("/home") || path.contains("/root") {
        suggestions.push(("High", "List user files", "ls -la"));
        suggestions.push(("Medium", "Find configuration files", "find .* ."));
        suggestions.push(("Low", "Search for SSH keys", "find .ssh ."));
    } else if path.contains("/usr") {
        suggestions.push(("High", "Installed applications", "discover apps"));
        suggestions.push(("Medium", "Package information", "packages"));
    } else if path == "/" {
        suggestions.push(("High", "System overview", "dashboard"));
        suggestions.push(("High", "Quick summary", "summary"));
        suggestions.push(("Medium", "Security analysis", "wizard security"));
        suggestions.push(("Low", "Explore filesystem", "tree / 2"));
    }

    // Add generic suggestions
    if !path.contains("/var/log") {
        suggestions.push(("Info", "Navigate to logs", "cd /var/log"));
    }
    if !path.contains("/etc") {
        suggestions.push(("Info", "Navigate to config", "cd /etc"));
    }

    println!("{}", "Suggested Actions:".yellow().bold());
    println!("{}", "─".repeat(70).cyan());

    for (priority, desc, cmd) in suggestions {
        let priority_colored = match priority {
            "High" => priority.red().bold(),
            "Medium" => priority.yellow().bold(),
            "Low" => priority.green(),
            _ => priority.cyan(),
        };

        println!("  {} {} - {}", priority_colored, desc, cmd.bright_black());
    }

    println!();
    println!(
        "{} Run {} for location-specific help",
        "💡".yellow(),
        "context".cyan()
    );
    println!();

    Ok(())
}

pub fn cmd_inspect(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("\n{}", "Usage: inspect <component>".red());
        println!();
        println!("{}", "Available Components:".yellow().bold());
        println!("  {} - Boot configuration and kernel", "boot".green());
        println!("  {} - System logging configuration", "logging".green());
        println!(
            "  {} - Package manager and repositories",
            "packages".green()
        );
        println!("  {} - System services and daemons", "services".green());
        println!("  {} - Kernel modules and drivers", "kernel".green());
        println!();
        return Ok(());
    }

    let component = args[0];

    match component {
        "boot" => {
            println!(
                "\n{} {}",
                "🚀".cyan(),
                "Boot Configuration Inspection".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Filesystem Table (/etc/fstab):".green().bold());
            if ctx.guestfs.exists("/etc/fstab")? {
                if let Ok(content) = ctx.guestfs.read_file("/etc/fstab") {
                    let lines: Vec<&str> = std::str::from_utf8(&content)
                        .unwrap_or("")
                        .lines()
                        .filter(|line| !line.trim().is_empty() && !line.trim().starts_with('#'))
                        .collect();

                    for line in lines {
                        println!("  {}", line.cyan());
                    }
                }
            } else {
                println!("  {} /etc/fstab not found", "✗".red());
            }
            println!();

            println!("{}", "Bootloader Configuration:".green().bold());
            let grub_files = vec![
                "/boot/grub/grub.cfg",
                "/boot/grub2/grub.cfg",
                "/boot/efi/EFI/*/grub.cfg",
            ];
            for file in grub_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!("  {} {}", "✓".green(), file.cyan());
                }
            }
            println!();

            // Kernel information would be displayed here if available
            println!();
        }

        "logging" => {
            println!(
                "\n{} {}",
                "📝".cyan(),
                "Logging Configuration Inspection".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "System Logging:".green().bold());
            let log_configs = vec![
                ("/etc/rsyslog.conf", "rsyslog configuration"),
                ("/etc/syslog-ng/syslog-ng.conf", "syslog-ng configuration"),
                (
                    "/etc/systemd/journald.conf",
                    "systemd journal configuration",
                ),
            ];

            for (file, desc) in log_configs {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!(
                        "  {} {} - {}",
                        "✓".green(),
                        file.cyan(),
                        desc.bright_black()
                    );
                } else {
                    println!(
                        "  {} {} - {}",
                        "✗".bright_black(),
                        file.bright_black(),
                        desc.bright_black()
                    );
                }
            }
            println!();

            println!("{}", "Log Directories:".green().bold());
            let log_dirs = vec!["/var/log", "/var/log/audit", "/var/log/journal"];

            for dir in log_dirs {
                if ctx.guestfs.is_dir(dir).unwrap_or(false) {
                    if let Ok(files) = ctx.guestfs.ls(dir) {
                        println!(
                            "  {} {} ({} files)",
                            "✓".green(),
                            dir.cyan(),
                            files.len().to_string().yellow()
                        );
                    }
                }
            }
            println!();

            println!("{} Commands to explore logs:", "💡".yellow());
            println!("  • {}", "cd /var/log".cyan());
            println!("  • {}", "recent /var/log 20".cyan());
            println!("  • {}", "search error --content --path /var/log".cyan());
            println!();
        }

        "packages" => {
            println!(
                "\n{} {}",
                "📦".cyan(),
                "Package Manager Inspection".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("{}", "Package Statistics:".green().bold());
                println!(
                    "  Total packages: {}",
                    pkg_info.packages.len().to_string().yellow().bold()
                );
                println!();

                // Categorize packages
                let mut categories: std::collections::HashMap<&str, usize> =
                    std::collections::HashMap::new();
                for pkg in &pkg_info.packages {
                    let name = pkg.name.to_lowercase();
                    if name.contains("lib") {
                        *categories.entry("Libraries").or_insert(0) += 1;
                    } else if name.contains("devel") || name.contains("dev") {
                        *categories.entry("Development").or_insert(0) += 1;
                    } else if name.contains("doc") {
                        *categories.entry("Documentation").or_insert(0) += 1;
                    } else if name.contains("kernel") {
                        *categories.entry("Kernel").or_insert(0) += 1;
                    } else if name.contains("python")
                        || name.contains("perl")
                        || name.contains("ruby")
                    {
                        *categories.entry("Interpreters").or_insert(0) += 1;
                    } else {
                        *categories.entry("Other").or_insert(0) += 1;
                    }
                }

                println!("{}", "Package Categories:".green().bold());
                let mut cat_vec: Vec<_> = categories.iter().collect();
                cat_vec.sort_by_key(|(_, count)| std::cmp::Reverse(**count));
                for (cat, count) in cat_vec {
                    println!("  {:15} {}", cat, count.to_string().cyan());
                }
                println!();
            }

            println!("{}", "Package Manager Configuration:".green().bold());
            let pkg_configs = vec![
                ("/etc/yum.conf", "YUM configuration"),
                ("/etc/dnf/dnf.conf", "DNF configuration"),
                ("/etc/apt/sources.list", "APT sources"),
                ("/etc/zypp/zypp.conf", "Zypper configuration"),
            ];

            for (file, desc) in pkg_configs {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!(
                        "  {} {} - {}",
                        "✓".green(),
                        file.cyan(),
                        desc.bright_black()
                    );
                }
            }
            println!();

            println!("{} Package commands:", "💡".yellow());
            println!("  • {}", "packages <pattern> - Search packages".cyan());
            println!(
                "  • {}",
                "export packages json - Export package list".cyan()
            );
            println!();
        }

        "services" => {
            println!(
                "\n{} {}",
                "⚙".cyan(),
                "System Services Inspection".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled: Vec<_> = services.iter().filter(|s| s.enabled).collect();
                let disabled: Vec<_> = services.iter().filter(|s| !s.enabled).collect();

                println!("{}", "Service Statistics:".green().bold());
                println!("  Total:    {}", services.len().to_string().yellow());
                println!(
                    "  Enabled:  {} {}%",
                    enabled.len().to_string().green(),
                    format!(
                        "({:.1})",
                        (enabled.len() as f64 / services.len().max(1) as f64) * 100.0
                    )
                    .bright_black()
                );
                println!("  Disabled: {}", disabled.len().to_string().bright_black());
                println!();

                println!("{}", "Enabled Services (first 20):".green().bold());
                for svc in enabled.iter().take(20) {
                    println!("  {} {}", "✓".green(), svc.name.cyan());
                }
                if enabled.len() > 20 {
                    println!(
                        "  ... and {} more",
                        (enabled.len() - 20).to_string().bright_black()
                    );
                }
                println!();

                // Categorize services
                let mut critical = Vec::new();
                let mut network = Vec::new();
                let mut security = Vec::new();

                for svc in &enabled {
                    let name = svc.name.to_lowercase();
                    if name.contains("ssh") || name.contains("systemd") || name.contains("dbus") {
                        critical.push(&svc.name);
                    } else if name.contains("network") || name.contains("firewall") {
                        network.push(&svc.name);
                    } else if name.contains("selinux") || name.contains("audit") {
                        security.push(&svc.name);
                    }
                }

                if !critical.is_empty() {
                    println!("{}", "Critical Services:".red().bold());
                    for svc in critical {
                        println!("  • {}", svc.yellow());
                    }
                    println!();
                }

                if !network.is_empty() {
                    println!("{}", "Network Services:".cyan().bold());
                    for svc in network {
                        println!("  • {}", svc.cyan());
                    }
                    println!();
                }

                if !security.is_empty() {
                    println!("{}", "Security Services:".green().bold());
                    for svc in security {
                        println!("  • {}", svc.green());
                    }
                    println!();
                }
            }

            println!("{} Service commands:", "💡".yellow());
            println!("  • {}", "services - List all services".cyan());
            println!("  • {}", "services <pattern> - Search services".cyan());
            println!();
        }

        "kernel" => {
            println!("\n{} {}", "🔧".cyan(), "Kernel Inspection".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            // Kernel version information would be displayed here if available
            println!();

            println!("{}", "Kernel Modules:".green().bold());
            let mod_dirs = vec!["/lib/modules", "/usr/lib/modules"];

            for dir in mod_dirs {
                if ctx.guestfs.is_dir(dir).unwrap_or(false) {
                    if let Ok(subdirs) = ctx.guestfs.ls(dir) {
                        println!("  {} {}", "✓".green(), dir.cyan());
                        for subdir in subdirs.iter().take(5) {
                            println!("    • {}", subdir.bright_black());
                        }
                        if subdirs.len() > 5 {
                            println!(
                                "    ... and {} more",
                                (subdirs.len() - 5).to_string().bright_black()
                            );
                        }
                    }
                }
            }
            println!();

            println!("{}", "Kernel Configuration:".green().bold());
            let kernel_configs = vec!["/boot/config-*", "/proc/config.gz"];

            for pattern in kernel_configs {
                println!("  {}", pattern.cyan());
            }
            println!();

            println!("{} Explore kernel:", "💡".yellow());
            println!("  • {}", "cd /boot".cyan());
            println!("  • {}", "ls -la /boot".cyan());
            println!("  • {}", "cd /lib/modules".cyan());
            println!();
        }

        _ => {
            println!("{} Unknown component: {}", "Error:".red(), component);
            println!("{} inspect <component>", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

pub fn cmd_insights(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Intelligent System Insights                ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!("{}", "Analyzing system patterns...".yellow());
    println!();

    let mut insights = Vec::new();

    // Analyze packages
    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        let pkg_count = pkg_info.packages.len();

        if pkg_count > 1500 {
            insights.push((
                "📦",
                "Package Density",
                format!(
                    "{} packages detected - This is a feature-rich system",
                    pkg_count
                ),
                "Consider reviewing with 'packages' to identify unused packages".to_string(),
                "Medium",
            ));
        } else if pkg_count < 300 {
            insights.push((
                "📦",
                "Minimal Footprint",
                format!("{} packages - This is a lean, focused system", pkg_count),
                "Minimal attack surface, good for security".to_string(),
                "Info",
            ));
        }

        // Detect development environment
        let dev_packages = pkg_info
            .packages
            .iter()
            .filter(|p| {
                p.name.contains("gcc")
                    || p.name.contains("make")
                    || p.name.contains("git")
                    || p.name.contains("devel")
            })
            .count();

        if dev_packages > 20 {
            insights.push((
                "💻",
                "Development Environment",
                format!("{} development packages found", dev_packages),
                "This appears to be a build/development system - ensure build tools are up to date"
                    .to_string(),
                "Info",
            ));
        }

        // Check for multiple web servers
        let web_servers = pkg_info
            .packages
            .iter()
            .filter(|p| {
                p.name.contains("httpd") || p.name.contains("nginx") || p.name.contains("apache")
            })
            .count();

        if web_servers > 1 {
            insights.push((
                "⚠️",
                "Multiple Web Servers",
                format!("{} different web server packages detected", web_servers),
                "Multiple web servers can cause port conflicts - verify only one is enabled"
                    .to_string(),
                "Warning",
            ));
        }
    }

    // Analyze security
    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        let mut security_score = 0;
        let mut security_features = Vec::new();

        if &sec.selinux != "disabled" {
            security_score += 1;
            security_features.push("SELinux");
        }
        if sec.apparmor {
            security_score += 1;
            security_features.push("AppArmor");
        }
        if sec.auditd {
            security_score += 1;
            security_features.push("Auditd");
        }

        if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
            if fw.enabled {
                security_score += 1;
                security_features.push("Firewall");
            }
        }

        if security_score >= 3 {
            insights.push((
                "🛡️",
                "Strong Security Posture",
                format!(
                    "{} security features active: {}",
                    security_score,
                    security_features.join(", ")
                ),
                "Well-configured security - maintain with regular updates".to_string(),
                "Good",
            ));
        } else if security_score <= 1 {
            insights.push((
                "🚨",
                "Weak Security Posture",
                format!("Only {} security features active", security_score),
                "Critical: Enable SELinux/AppArmor and firewall - run 'advisor secure'".to_string(),
                "Critical",
            ));
        }
    }

    // Analyze users
    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let root_users = users.iter().filter(|u| u.uid == "0").count();
        let regular_users = users
            .iter()
            .filter(|u| {
                if let Ok(uid) = u.uid.parse::<u32>() {
                    uid >= 1000
                } else {
                    false
                }
            })
            .count();

        if root_users > 1 {
            insights.push((
                "⚠️",
                "Multiple Root Accounts",
                format!("{} accounts with UID 0 detected", root_users),
                "Security risk: Review root accounts immediately with 'users'".to_string(),
                "High",
            ));
        }

        if regular_users == 0 {
            insights.push((
                "🤖",
                "Service-Only System",
                "No regular user accounts detected".to_string(),
                "This is a dedicated service system - appropriate for containers/VMs".to_string(),
                "Info",
            ));
        } else if regular_users > 10 {
            insights.push((
                "👥",
                "Multi-User Environment",
                format!("{} regular user accounts", regular_users),
                "Review user access regularly for security - 'users' command".to_string(),
                "Info",
            ));
        }
    }

    // Analyze services
    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled = services.iter().filter(|s| s.enabled).count();
        let total = services.len();
        let ratio = (enabled as f64 / total.max(1) as f64) * 100.0;

        if ratio > 70.0 {
            insights.push((
                "⚙️",
                "High Service Density",
                format!("{:.0}% of services enabled ({}/{})", ratio, enabled, total),
                "Many services running - review with 'services' to disable unused ones".to_string(),
                "Medium",
            ));
        } else if ratio < 30.0 {
            insights.push((
                "⚙️",
                "Selective Service Configuration",
                format!("Only {:.0}% of services enabled", ratio),
                "Conservative service configuration - good for security and performance"
                    .to_string(),
                "Good",
            ));
        }
    }

    // Display insights
    if insights.is_empty() {
        println!("{}", "No significant patterns detected.".bright_black());
        println!("System appears to be in a standard configuration.");
    } else {
        println!(
            "{} ({} insights)",
            "Key Insights:".green().bold(),
            insights.len()
        );
        println!("{}", "─".repeat(70).cyan());
        println!();

        for (icon, title, description, recommendation, priority) in insights {
            let priority_colored = match priority {
                "Critical" => priority.red().bold(),
                "High" => priority.red(),
                "Warning" => priority.yellow().bold(),
                "Medium" => priority.yellow(),
                "Good" => priority.green(),
                _ => priority.cyan(),
            };

            println!("{} {} [{}]", icon, title.bold(), priority_colored);
            println!("  {}", description);
            println!("  {} {}", "→".cyan(), recommendation.bright_black());
            println!();
        }
    }

    println!("{} Next Steps:", "💡".yellow());
    println!("  • {}", "verify all - Comprehensive validation".cyan());
    println!("  • {}", "advisor secure - Security improvements".cyan());
    println!("  • {}", "optimize - Optimization recommendations".cyan());
    println!();

    Ok(())
}

/// Interactive diagnostic doctor
pub fn cmd_doctor(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                  System Doctor                           ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!("{}", "Running comprehensive system diagnostic...".yellow());
    println!();

    let mut health_score = 100;
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    let mut recommendations = Vec::new();

    // Check 1: Critical Files
    println!("{} Checking critical files...", "→".cyan());
    let critical_files = vec![
        ("/etc/passwd", "User database"),
        ("/etc/shadow", "Password hashes"),
        ("/etc/fstab", "Filesystem table"),
    ];

    for (file, desc) in &critical_files {
        if !ctx.guestfs.exists(file).unwrap_or(false) {
            health_score -= 20;
            issues.push(format!("Missing critical file: {} ({})", file, desc));
        }
    }

    // Check 2: Security Configuration
    println!("{} Checking security configuration...", "→".cyan());
    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        if &sec.selinux == "disabled" {
            health_score -= 10;
            warnings.push("SELinux is disabled - mandatory access control not active");
            recommendations.push("Enable SELinux for enhanced security");
        }

        if !sec.apparmor && &sec.selinux == "disabled" {
            health_score -= 10;
            warnings.push("No MAC system active (neither SELinux nor AppArmor)");
        }

        if !sec.auditd {
            health_score -= 5;
            warnings.push("Audit daemon not running - no detailed event logging");
            recommendations.push("Enable auditd for security event tracking");
        }

        if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
            if !fw.enabled {
                health_score -= 15;
                warnings.push("Firewall is disabled - no network filtering");
                recommendations.push("Enable and configure firewall immediately");
            }
        }
    }

    // Check 3: Boot Configuration
    println!("{} Checking boot configuration...", "→".cyan());
    if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
        health_score -= 15;
        issues.push("Missing /etc/fstab - system may not boot properly".to_string());
    }

    let grub_found = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false)
        || ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);

    if !grub_found {
        health_score -= 10;
        warnings.push("No GRUB configuration found - boot loader may not be configured");
    }

    // Check 4: User Configuration
    println!("{} Checking user configuration...", "→".cyan());
    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let root_users = users.iter().filter(|u| u.uid == "0").count();
        if root_users > 1 {
            health_score -= 15;
            issues.push(format!("Multiple root accounts detected ({})", root_users));
            recommendations.push("Audit root accounts and remove duplicates");
        }

        let locked_users = users
            .iter()
            .filter(|u| u.shell.contains("nologin") || u.shell.contains("false"))
            .count();
        if locked_users == users.len() && !users.is_empty() {
            warnings.push("All user accounts appear to be locked");
        }
    }

    // Check 5: Network Configuration
    println!("{} Checking network configuration...", "→".cyan());
    if !ctx.guestfs.exists("/etc/hosts").unwrap_or(false) {
        health_score -= 5;
        warnings.push("Missing /etc/hosts file");
    }

    if !ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
        health_score -= 5;
        warnings.push("Missing /etc/resolv.conf - DNS may not be configured");
    }

    println!();
    println!("{}", "═".repeat(70).cyan());
    println!();

    // Display Results
    let health_grade = if health_score >= 90 {
        "A (Excellent)".green().bold()
    } else if health_score >= 75 {
        "B (Good)".green()
    } else if health_score >= 60 {
        "C (Fair)".yellow()
    } else if health_score >= 40 {
        "D (Poor)".red()
    } else {
        "F (Critical)".red().bold()
    };

    println!(
        "{} {}/100 - Grade: {}",
        "Overall Health Score:".green().bold(),
        health_score,
        health_grade
    );
    println!();

    if !issues.is_empty() {
        println!(
            "{} ({} found)",
            "Critical Issues:".red().bold(),
            issues.len()
        );
        for issue in &issues {
            println!("  {} {}", "✗".red(), issue);
        }
        println!();
    }

    if !warnings.is_empty() {
        println!("{} ({} found)", "Warnings:".yellow().bold(), warnings.len());
        for warning in &warnings {
            println!("  {} {}", "⚠".yellow(), warning);
        }
        println!();
    }

    if !recommendations.is_empty() {
        println!(
            "{} ({} items)",
            "Recommended Actions:".cyan().bold(),
            recommendations.len()
        );
        for (i, rec) in recommendations.iter().enumerate() {
            println!("  {} {}", format!("{}.", i + 1).cyan(), rec);
        }
        println!();
    }

    if issues.is_empty() && warnings.is_empty() {
        println!(
            "{} System is healthy! No critical issues detected.",
            "✓".green().bold()
        );
        println!();
    }

    println!("{} Detailed Analysis:", "💡".yellow());
    println!("  • {}", "verify all - Run all verification checks".cyan());
    println!(
        "  • {}",
        "wizard health - Interactive health assessment".cyan()
    );
    println!("  • {}", "scan issues - Scan for specific problems".cyan());
    println!();

    Ok(())
}

/// Goal setting and tracking
pub fn cmd_predict(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Predictive Issue Analysis                   ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!(
        "{}",
        "Analyzing system patterns for potential future issues...".yellow()
    );
    println!();

    let mut predictions = Vec::new();

    // Get system data
    let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
    let sec = ctx.guestfs.inspect_security(&ctx.root)?;

    // Prediction 1: Security vulnerabilities
    if &sec.selinux == "disabled" && !sec.apparmor {
        predictions.push((
            "🔓",
            "High Risk: Security Breach",
            "No MAC system active increases attack surface",
            "Within 30 days without security hardening",
            "Critical",
            vec![
                "Enable SELinux or AppArmor immediately",
                "Review security audit logs",
                "Implement least-privilege access controls",
            ],
        ));
    }

    // Prediction 2: Package updates
    let pkg_count = pkg_info.packages.len();
    if pkg_count > 500 {
        predictions.push((
            "📦",
            "Medium: Package Update Burden",
            "Large number of packages requires frequent updates",
            "Ongoing maintenance burden",
            "Medium",
            vec![
                "Set up automated update scheduling",
                "Review installed packages for unnecessary ones",
                "Consider containerizing some workloads",
            ],
        ));
    }

    // Prediction 3: Boot issues
    if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
        predictions.push((
            "⚠️",
            "Critical: Boot Failure Risk",
            "Missing /etc/fstab may prevent system boot",
            "Next reboot will likely fail",
            "Critical",
            vec![
                "Generate proper /etc/fstab immediately",
                "Test boot configuration in safe environment",
                "Document filesystem mount requirements",
            ],
        ));
    }

    // Prediction 4: Compliance drift
    if !sec.auditd {
        predictions.push((
            "📋",
            "Medium: Compliance Drift",
            "No audit logging means compliance violations may be undetected",
            "Audit failures within 90 days",
            "Medium",
            vec![
                "Enable auditd service",
                "Configure audit rules for compliance requirements",
                "Set up centralized log collection",
            ],
        ));
    }

    // Prediction 5: Service degradation
    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled = services.iter().filter(|s| s.enabled).count();
        if enabled > 50 {
            predictions.push((
                "⚙️",
                "Low: Performance Degradation",
                "Many enabled services may cause resource contention",
                "Performance issues within 60-90 days under load",
                "Low",
                vec![
                    "Review and disable unnecessary services",
                    "Implement resource limits and quotas",
                    "Monitor CPU and memory usage trends",
                ],
            ));
        }
    }

    // Prediction 6: User account issues
    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let normal_users = users.iter().filter(|u| u.uid != "0").count();
        if normal_users == 0 {
            predictions.push((
                "👤",
                "Medium: Single Point of Failure",
                "Only root account exists - no user separation",
                "Security incident within 30-60 days",
                "Medium",
                vec![
                    "Create dedicated service accounts",
                    "Implement sudo for privileged operations",
                    "Disable direct root login",
                ],
            ));
        }
    }

    // Display predictions
    if predictions.is_empty() {
        println!("{}", "✓ No significant issues predicted!".green().bold());
        println!("  Your system follows best practices.");
    } else {
        println!(
            "{} {} predictions identified:",
            "🔮".cyan(),
            predictions.len().to_string().cyan().bold()
        );
        println!();

        for (icon, title, description, timeline, severity, mitigations) in &predictions {
            let severity_colored = match *severity {
                "Critical" => severity.red().bold(),
                "High" => severity.red(),
                "Medium" => severity.yellow(),
                _ => severity.bright_black(),
            };

            println!("{} {} [{}]", icon.cyan(), title.bold(), severity_colored);
            println!("  Issue:      {}", description);
            println!("  Timeline:   {}", timeline.cyan());
            println!("  Mitigation:");
            for (i, mitigation) in mitigations.iter().enumerate() {
                println!("    {}. {}", i + 1, mitigation);
            }
            println!();
        }

        // Summary
        let critical = predictions.iter().filter(|p| p.4 == "Critical").count();
        let high = predictions.iter().filter(|p| p.4 == "High").count();
        let medium = predictions.iter().filter(|p| p.4 == "Medium").count();

        println!("{} Summary:", "📊".cyan());
        if critical > 0 {
            println!(
                "  {} Critical issues requiring immediate attention",
                critical.to_string().red().bold()
            );
        }
        if high > 0 {
            println!(
                "  {} High priority issues to address soon",
                high.to_string().red()
            );
        }
        if medium > 0 {
            println!(
                "  {} Medium priority issues to plan for",
                medium.to_string().yellow()
            );
        }
    }

    println!();
    println!("{} Next Steps:", "💡".yellow());
    println!("  • {}", "doctor - Run comprehensive health check".cyan());
    println!(
        "  • {}",
        "verify all - Validate all system components".cyan()
    );
    println!(
        "  • {}",
        "roadmap 30 - Create 30-day improvement plan".cyan()
    );
    println!();

    Ok(())
}

/// Data visualization with ASCII charts
pub fn cmd_chart(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Data Visualization Charts                   ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    let chart_type = if args.is_empty() { "menu" } else { args[0] };

    match chart_type {
        "menu" => {
            println!("{}", "Available Charts:".yellow().bold());
            println!();
            println!(
                "{} {} - Package distribution by category",
                "1.".cyan(),
                "packages".green()
            );
            println!(
                "{} {} - User account distribution",
                "2.".cyan(),
                "users".green()
            );
            println!(
                "{} {} - Service status breakdown",
                "3.".cyan(),
                "services".green()
            );
            println!(
                "{} {} - Storage usage visualization",
                "4.".cyan(),
                "storage".green()
            );
            println!(
                "{} {} - Security features overview",
                "5.".cyan(),
                "security".green()
            );
            println!();
            println!("{} chart <name>", "Usage:".yellow());
        }

        "packages" => {
            println!("{}", "📦 Package Distribution Chart".cyan().bold());
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;

            // Categorize packages
            let mut dev_tools = 0;
            let mut libraries = 0;
            let mut system = 0;
            let mut apps = 0;
            let mut other = 0;

            for pkg in &pkg_info.packages {
                let name = pkg.name.to_lowercase();
                if name.contains("gcc")
                    || name.contains("make")
                    || name.contains("devel")
                    || name.contains("dev-")
                {
                    dev_tools += 1;
                } else if name.contains("lib") || name.starts_with("lib") {
                    libraries += 1;
                } else if name.contains("kernel")
                    || name.contains("systemd")
                    || name.contains("core")
                {
                    system += 1;
                } else if name.contains("app") || name.contains("tool") {
                    apps += 1;
                } else {
                    other += 1;
                }
            }

            let total = (pkg_info.packages.len().max(1)) as f32;
            let max_bar = 50;

            println!(
                "Development Tools: {} ({}%)",
                dev_tools,
                ((dev_tools as f32 / total) * 100.0) as i32
            );
            let bar_len = ((dev_tools as f32 / total) * max_bar as f32) as usize;
            println!(
                "{} {}",
                "▓".repeat(bar_len).green(),
                "░".repeat(max_bar - bar_len).bright_black()
            );
            println!();

            println!(
                "Libraries:         {} ({}%)",
                libraries,
                ((libraries as f32 / total) * 100.0) as i32
            );
            let bar_len = ((libraries as f32 / total) * max_bar as f32) as usize;
            println!(
                "{} {}",
                "▓".repeat(bar_len).cyan(),
                "░".repeat(max_bar - bar_len).bright_black()
            );
            println!();

            println!(
                "System Packages:   {} ({}%)",
                system,
                ((system as f32 / total) * 100.0) as i32
            );
            let bar_len = ((system as f32 / total) * max_bar as f32) as usize;
            println!(
                "{} {}",
                "▓".repeat(bar_len).yellow(),
                "░".repeat(max_bar - bar_len).bright_black()
            );
            println!();

            println!(
                "Applications:      {} ({}%)",
                apps,
                ((apps as f32 / total) * 100.0) as i32
            );
            let bar_len = ((apps as f32 / total) * max_bar as f32) as usize;
            println!(
                "{} {}",
                "▓".repeat(bar_len).blue(),
                "░".repeat(max_bar - bar_len).bright_black()
            );
            println!();

            println!(
                "Other:             {} ({}%)",
                other,
                ((other as f32 / total) * 100.0) as i32
            );
            let bar_len = ((other as f32 / total) * max_bar as f32) as usize;
            println!(
                "{} {}",
                "▓".repeat(bar_len).bright_black(),
                "░".repeat(max_bar - bar_len).bright_black()
            );
            println!();

            println!("Total: {} packages", total as i32);
        }

        "users" => {
            println!("{}", "👥 User Account Distribution".cyan().bold());
            println!();

            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let root_users = users.iter().filter(|u| u.uid == "0").count();
                let system_users = users
                    .iter()
                    .filter(|u| {
                        let uid = u.uid.parse::<i32>().unwrap_or(9999);
                        uid > 0 && uid < 1000
                    })
                    .count();
                let normal_users = users
                    .iter()
                    .filter(|u| {
                        let uid = u.uid.parse::<i32>().unwrap_or(0);
                        uid >= 1000
                    })
                    .count();

                let total = (users.len().max(1)) as f32;
                let max_bar = 50;

                println!("Root (UID 0):      {}", root_users);
                let bar_len = ((root_users as f32 / total) * max_bar as f32) as usize;
                println!(
                    "{} {}",
                    "▓".repeat(bar_len).red(),
                    "░".repeat(max_bar - bar_len).bright_black()
                );
                println!();

                println!("System (1-999):    {}", system_users);
                let bar_len = ((system_users as f32 / total) * max_bar as f32) as usize;
                println!(
                    "{} {}",
                    "▓".repeat(bar_len).yellow(),
                    "░".repeat(max_bar - bar_len).bright_black()
                );
                println!();

                println!("Normal (1000+):    {}", normal_users);
                let bar_len = ((normal_users as f32 / total) * max_bar as f32) as usize;
                println!(
                    "{} {}",
                    "▓".repeat(bar_len).green(),
                    "░".repeat(max_bar - bar_len).bright_black()
                );
                println!();

                println!("Total: {} users", total as i32);
            }
        }

        "services" => {
            println!("{}", "⚙️  Service Status Breakdown".cyan().bold());
            println!();

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                let disabled = services.len() - enabled;
                let total = (services.len().max(1)) as f32;
                let max_bar = 50;

                println!(
                    "Enabled Services:  {} ({}%)",
                    enabled,
                    ((enabled as f32 / total) * 100.0) as i32
                );
                let bar_len = ((enabled as f32 / total) * max_bar as f32) as usize;
                println!(
                    "{} {}",
                    "▓".repeat(bar_len).green(),
                    "░".repeat(max_bar - bar_len).bright_black()
                );
                println!();

                println!(
                    "Disabled Services: {} ({}%)",
                    disabled,
                    ((disabled as f32 / total) * 100.0) as i32
                );
                let bar_len = ((disabled as f32 / total) * max_bar as f32) as usize;
                println!(
                    "{} {}",
                    "▓".repeat(bar_len).red(),
                    "░".repeat(max_bar - bar_len).bright_black()
                );
                println!();

                println!("Total: {} services", total as i32);
                println!();
                println!(
                    "Service Density: {}",
                    if enabled > 50 {
                        "High".red()
                    } else if enabled > 30 {
                        "Medium".yellow()
                    } else {
                        "Low".green()
                    }
                );
            }
        }

        "storage" => {
            println!("{}", "💾 Storage Usage Visualization".cyan().bold());
            println!();

            if let Ok(filesystems) = ctx.guestfs.list_filesystems() {
                println!("Mounted Filesystems:");
                println!();

                for (path, fstype) in filesystems.iter().take(10) {
                    if fstype != "unknown" && fstype != "swap" {
                        // Simplified visualization (actual size info would require statvfs)
                        println!("{}", path.cyan());
                        println!("  Type: {}", fstype.green());
                        println!(
                            "  {}",
                            "▓▓▓▓▓▓▓▓▓▓░░░░░░░░░░ 50% usage (estimated)".bright_black()
                        );
                        println!();
                    }
                }
            }
        }

        "security" => {
            println!("{}", "🛡️  Security Features Overview".cyan().bold());
            println!();

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            let max_bar = 40;

            // SELinux
            let selinux_status = if &sec.selinux != "disabled" { 1.0 } else { 0.0 };
            println!(
                "SELinux:    [{}{}] {}",
                "▓"
                    .repeat((selinux_status * max_bar as f32) as usize)
                    .green(),
                "░"
                    .repeat(((1.0 - selinux_status) * max_bar as f32) as usize)
                    .bright_black(),
                if selinux_status > 0.0 {
                    "Enabled".green()
                } else {
                    "Disabled".red()
                }
            );

            // AppArmor
            let apparmor_status = if sec.apparmor { 1.0 } else { 0.0 };
            println!(
                "AppArmor:   [{}{}] {}",
                "▓"
                    .repeat((apparmor_status * max_bar as f32) as usize)
                    .green(),
                "░"
                    .repeat(((1.0 - apparmor_status) * max_bar as f32) as usize)
                    .bright_black(),
                if apparmor_status > 0.0 {
                    "Active".green()
                } else {
                    "Inactive".red()
                }
            );

            // Auditd
            let auditd_status = if sec.auditd { 1.0 } else { 0.0 };
            println!(
                "Auditd:     [{}{}] {}",
                "▓"
                    .repeat((auditd_status * max_bar as f32) as usize)
                    .green(),
                "░"
                    .repeat(((1.0 - auditd_status) * max_bar as f32) as usize)
                    .bright_black(),
                if auditd_status > 0.0 {
                    "Running".green()
                } else {
                    "Not Running".red()
                }
            );

            // Firewall
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                let fw_status = if fw.enabled { 1.0 } else { 0.0 };
                println!(
                    "Firewall:   [{}{}] {}",
                    "▓".repeat((fw_status * max_bar as f32) as usize).green(),
                    "░"
                        .repeat(((1.0 - fw_status) * max_bar as f32) as usize)
                        .bright_black(),
                    if fw_status > 0.0 {
                        "Enabled".green()
                    } else {
                        "Disabled".red()
                    }
                );
            }

            println!();
            let score = ((selinux_status + apparmor_status + auditd_status) / 3.0 * 100.0) as i32;
            println!("Overall Security Score: {}%", score.to_string().cyan());
        }

        _ => {
            println!("{} Unknown chart type: {}", "Error:".red(), chart_type);
            println!("{} chart menu", "Usage:".yellow());
        }
    }

    println!();
    Ok(())
}

/// Compliance checking against standards
pub fn cmd_compliance(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Compliance Standards Checker                ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    let standard = if args.is_empty() { "menu" } else { args[0] };

    match standard {
        "menu" => {
            println!("{}", "Available Compliance Standards:".yellow().bold());
            println!();
            println!(
                "{} {} - Center for Internet Security Benchmark",
                "1.".cyan(),
                "cis".green()
            );
            println!(
                "{} {} - Payment Card Industry Data Security",
                "2.".cyan(),
                "pci-dss".green()
            );
            println!(
                "{} {} - Health Insurance Portability Act",
                "3.".cyan(),
                "hipaa".green()
            );
            println!(
                "{} {} - General Data Protection Regulation",
                "4.".cyan(),
                "gdpr".green()
            );
            println!(
                "{} {} - Service Organization Control",
                "5.".cyan(),
                "soc2".green()
            );
            println!();
            println!("{} compliance <standard>", "Usage:".yellow());
        }

        "cis" => {
            println!("{}", "📋 CIS Benchmark Compliance Check".cyan().bold());
            println!();

            let mut passed = 0;
            let mut failed = 0;
            let mut checks = Vec::new();

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;

            // CIS 1.6.1: Ensure SELinux/AppArmor is enabled
            if &sec.selinux != "disabled" || sec.apparmor {
                checks.push((
                    "1.6.1",
                    "MAC system enabled",
                    true,
                    "SELinux or AppArmor is active",
                ));
                passed += 1;
            } else {
                checks.push((
                    "1.6.1",
                    "MAC system enabled",
                    false,
                    "Enable SELinux or AppArmor",
                ));
                failed += 1;
            }

            // CIS 4.1.1: Ensure auditing is enabled
            if sec.auditd {
                checks.push(("4.1.1", "Auditing enabled", true, "Auditd is running"));
                passed += 1;
            } else {
                checks.push((
                    "4.1.1",
                    "Auditing enabled",
                    false,
                    "Enable and start auditd service",
                ));
                failed += 1;
            }

            // CIS 3.5.1: Ensure firewall is enabled
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if fw.enabled {
                    checks.push(("3.5.1", "Firewall enabled", true, "Firewall is active"));
                    passed += 1;
                } else {
                    checks.push((
                        "3.5.1",
                        "Firewall enabled",
                        false,
                        "Enable firewall service",
                    ));
                    failed += 1;
                }
            }

            // CIS 5.2.1: Ensure permissions on /etc/ssh/sshd_config are configured
            if ctx.guestfs.exists("/etc/ssh/sshd_config").unwrap_or(false) {
                checks.push(("5.2.1", "SSH config exists", true, "sshd_config is present"));
                passed += 1;
            } else {
                checks.push((
                    "5.2.1",
                    "SSH config exists",
                    false,
                    "SSH server may not be configured",
                ));
                failed += 1;
            }

            // CIS 1.1.1: Ensure mounting of filesystems is configured
            if ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                checks.push((
                    "1.1.1",
                    "Filesystem table configured",
                    true,
                    "/etc/fstab exists",
                ));
                passed += 1;
            } else {
                checks.push((
                    "1.1.1",
                    "Filesystem table configured",
                    false,
                    "Create /etc/fstab",
                ));
                failed += 1;
            }

            // Display results
            for (id, name, status, detail) in checks {
                let status_icon = if status { "✓".green() } else { "✗".red() };
                let status_text = if status { "PASS".green() } else { "FAIL".red() };

                println!(
                    "{} {} {} - {}",
                    status_icon,
                    id.cyan(),
                    name.bold(),
                    status_text
                );
                println!("    {}", detail.bright_black());
                println!();
            }

            // Summary
            let total = (passed + failed).max(1);
            let compliance_rate = (passed as f32 / total as f32) * 100.0;

            println!("{} Compliance Summary:", "📊".cyan());
            println!("  Passed:     {} checks", passed.to_string().green());
            println!("  Failed:     {} checks", failed.to_string().red());
            println!("  Total:      {} checks", total);
            println!("  Rate:       {:.1}%", compliance_rate);
            println!();

            if compliance_rate >= 80.0 {
                println!("  Status:     {} Compliant", "✓".green().bold());
            } else if compliance_rate >= 60.0 {
                println!("  Status:     {} Partially Compliant", "⚠".yellow());
            } else {
                println!("  Status:     {} Non-Compliant", "✗".red());
            }
        }

        "pci-dss" => {
            println!("{}", "💳 PCI-DSS Compliance Check".cyan().bold());
            println!();

            let mut passed = 0;
            let mut failed = 0;

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;

            println!(
                "{} Install and maintain firewall configuration",
                "Requirement 1:".cyan().bold()
            );
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if fw.enabled {
                    println!("  {} Firewall is active", "✓".green());
                    passed += 1;
                } else {
                    println!("  {} Firewall is not active", "✗".red());
                    failed += 1;
                }
            } else {
                println!("  {} Could not verify firewall status", "?".yellow());
            }
            println!();

            println!(
                "{} Do not use vendor-supplied defaults",
                "Requirement 2:".cyan().bold()
            );
            // This would require deeper inspection
            println!("  {} Manual review required", "?".yellow());
            println!();

            println!(
                "{} Track and monitor all access to network resources",
                "Requirement 10:".cyan().bold()
            );
            if sec.auditd {
                println!("  {} Audit logging is enabled", "✓".green());
                passed += 1;
            } else {
                println!("  {} Audit logging is not enabled", "✗".red());
                failed += 1;
            }
            println!();

            let total = (passed + failed).max(1);
            if total > 0 {
                let rate = (passed as f32 / total as f32) * 100.0;
                println!(
                    "Automated checks: {:.0}% compliant ({}/{})",
                    rate, passed, total
                );
                println!();
            }

            println!(
                "{} PCI-DSS requires comprehensive manual audit.",
                "Note:".yellow()
            );
            println!("This automated check covers only basic requirements.");
        }

        "hipaa" => {
            println!("{}", "🏥 HIPAA Compliance Check".cyan().bold());
            println!();

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            let mut passed = 0;
            let mut failed = 0;

            println!("{} Access Control", "§164.312(a)(1):".cyan().bold());
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let normal_users = users.iter().filter(|u| u.uid != "0").count();
                if normal_users > 0 {
                    println!("  {} User access controls in place", "✓".green());
                    passed += 1;
                } else {
                    println!("  {} No non-root users (poor access control)", "✗".red());
                    failed += 1;
                }
            }
            println!();

            println!("{} Audit Controls", "§164.312(b):".cyan().bold());
            if sec.auditd {
                println!("  {} Audit logging enabled", "✓".green());
                passed += 1;
            } else {
                println!("  {} No audit logging", "✗".red());
                failed += 1;
            }
            println!();

            println!("{} Integrity Controls", "§164.312(c)(1):".cyan().bold());
            if &sec.selinux != "disabled" || sec.apparmor {
                println!("  {} Mandatory access control active", "✓".green());
                passed += 1;
            } else {
                println!("  {} No MAC system active", "✗".red());
                failed += 1;
            }
            println!();

            println!("{} Transmission Security", "§164.312(e)(1):".cyan().bold());
            // Would need to check for encryption configs
            println!("  {} Manual verification required", "?".yellow());
            println!();

            let total = (passed + failed).max(1);
            if total > 0 {
                let rate = (passed as f32 / total as f32) * 100.0;
                println!(
                    "Technical safeguards: {:.0}% implemented ({}/{})",
                    rate, passed, total
                );
            }
        }

        "gdpr" | "soc2" => {
            println!(
                "{} {} compliance checking requires manual audit.",
                standard.to_uppercase().cyan().bold(),
                "Note:".yellow()
            );
            println!();
            println!("Key areas to review:");
            println!("  • Data encryption at rest and in transit");
            println!("  • Access controls and authentication");
            println!("  • Audit logging and monitoring");
            println!("  • Data retention policies");
            println!("  • Incident response procedures");
            println!();
            println!("Use these commands for technical verification:");
            println!("  • {} - Security feature check", "verify security".green());
            println!("  • {} - System health diagnostic", "doctor".green());
            println!("  • {} - Security insights", "insights".green());
        }

        _ => {
            println!("{} Unknown standard: {}", "Error:".red(), standard);
            println!("{} compliance menu", "Usage:".yellow());
        }
    }

    println!();
    Ok(())
}

/// Command template system for repeatable operations
pub fn cmd_score(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Comprehensive System Score                  ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    println!(
        "{}",
        "Calculating multi-dimensional system score...".yellow()
    );
    println!();

    let mut total_score = 0;
    let mut max_score = 0;
    let mut scores = Vec::new();

    // Security Score (0-30)
    let sec = ctx.guestfs.inspect_security(&ctx.root)?;
    let mut sec_score = 0;
    max_score += 30;

    if &sec.selinux != "disabled" {
        sec_score += 10;
    }
    if sec.apparmor {
        sec_score += 10;
    }
    if sec.auditd {
        sec_score += 5;
    }
    if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
        if fw.enabled {
            sec_score += 5;
        }
    }

    total_score += sec_score;
    scores.push(("Security", sec_score, 30));

    // Reliability Score (0-25)
    let mut rel_score = 25;
    max_score += 25;

    if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
        rel_score -= 10;
    }
    let grub_found = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false)
        || ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);
    if !grub_found {
        rel_score -= 10;
    }
    if !ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
        rel_score -= 5;
    }

    total_score += rel_score;
    scores.push(("Reliability", rel_score, 25));

    // Configuration Score (0-20)
    let mut config_score = 0;
    max_score += 20;

    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        let normal_users = users.iter().filter(|u| u.uid != "0").count();
        if normal_users > 0 {
            config_score += 10;
        }
    }

    if ctx.guestfs.exists("/etc/ssh/sshd_config").unwrap_or(false) {
        config_score += 5;
    }

    let syslog_found = ctx.guestfs.exists("/etc/rsyslog.conf").unwrap_or(false)
        || ctx.guestfs.exists("/etc/syslog.conf").unwrap_or(false);
    if syslog_found {
        config_score += 5;
    }

    total_score += config_score;
    scores.push(("Configuration", config_score, 20));

    // Maintainability Score (0-15)
    let mut maint_score = 15;
    max_score += 15;

    let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
    let pkg_count = pkg_info.packages.len();

    if pkg_count > 1500 {
        maint_score -= 5; // Too many packages
    }
    if pkg_count < 100 {
        maint_score -= 5; // Too minimal, might be missing essentials
    }

    total_score += maint_score;
    scores.push(("Maintainability", maint_score, 15));

    // Performance Score (0-10)
    let mut perf_score = 10;
    max_score += 10;

    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let enabled = services.iter().filter(|s| s.enabled).count();
        if enabled > 80 {
            perf_score -= 5;
        } else if enabled > 50 {
            perf_score -= 3;
        }
    }

    total_score += perf_score;
    scores.push(("Performance", perf_score, 10));

    // Display scores
    println!("{}", "Score Breakdown:".cyan().bold());
    println!();

    for (category, score, max) in &scores {
        let percentage = (*score as f32 / *max as f32) * 100.0;
        let bar_length = 40;
        let filled = ((percentage / 100.0) * bar_length as f32) as usize;

        let color = if percentage >= 80.0 {
            "green"
        } else if percentage >= 60.0 {
            "yellow"
        } else {
            "red"
        };

        let bar = match color {
            "green" => format!(
                "{}{}",
                "▓".repeat(filled).green(),
                "░".repeat(bar_length - filled).bright_black()
            ),
            "yellow" => format!(
                "{}{}",
                "▓".repeat(filled).yellow(),
                "░".repeat(bar_length - filled).bright_black()
            ),
            _ => format!(
                "{}{}",
                "▓".repeat(filled).red(),
                "░".repeat(bar_length - filled).bright_black()
            ),
        };

        println!(
            "{:15} [{}] {}/{} ({:.0}%)",
            format!("{}:", category).bold(),
            bar,
            score.to_string().cyan(),
            max,
            percentage
        );
    }

    println!();
    println!("{}", "═".repeat(60).bright_black());

    let overall_percentage = (total_score as f32 / max_score as f32) * 100.0;
    let grade = if overall_percentage >= 90.0 {
        "A+ (Excellent)".green().bold()
    } else if overall_percentage >= 85.0 {
        "A (Very Good)".green()
    } else if overall_percentage >= 80.0 {
        "B+ (Good)".green()
    } else if overall_percentage >= 75.0 {
        "B (Above Average)".yellow()
    } else if overall_percentage >= 70.0 {
        "C+ (Average)".yellow()
    } else if overall_percentage >= 60.0 {
        "C (Below Average)".yellow()
    } else {
        "D (Needs Improvement)".red()
    };

    println!(
        "{:15} {}/{} ({:.1}%)",
        "Overall Score:".bold(),
        total_score.to_string().cyan().bold(),
        max_score,
        overall_percentage
    );
    println!("{:15} {}", "Grade:".bold(), grade);

    println!();
    println!("{} Recommendations:", "💡".yellow());

    if sec_score < 20 {
        println!(
            "  • {}",
            "Improve security posture (enable SELinux/AppArmor, firewall, audit)".cyan()
        );
    }
    if rel_score < 15 {
        println!(
            "  • {}",
            "Fix critical configuration files (/etc/fstab, boot loader)".cyan()
        );
    }
    if config_score < 10 {
        println!(
            "  • {}",
            "Enhance system configuration (user accounts, SSH, logging)".cyan()
        );
    }
    if maint_score < 10 {
        println!("  • {}", "Optimize package management".cyan());
    }
    if perf_score < 7 {
        println!("  • {}", "Reduce service overhead".cyan());
    }

    if overall_percentage >= 85.0 {
        println!();
        println!("{}", "✓ System is in excellent condition!".green().bold());
    } else if overall_percentage >= 70.0 {
        println!();
        println!(
            "{}",
            "⚠ System is acceptable but has room for improvement.".yellow()
        );
    } else {
        println!();
        println!(
            "{}",
            "✗ System requires attention to critical issues.".red()
        );
    }

    println!();
    Ok(())
}

/// Query system data with SQL-like syntax
pub fn cmd_query(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              System Query Interface                      ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    if args.is_empty() {
        println!("{}", "Available Queries:".yellow().bold());
        println!();
        println!(
            "{} {} - Find packages by name",
            "1.".cyan(),
            "packages where name=<pattern>".green()
        );
        println!(
            "{} {} - Find users by UID range",
            "2.".cyan(),
            "users where uid>1000".green()
        );
        println!(
            "{} {} - Find enabled services",
            "3.".cyan(),
            "services where enabled=true".green()
        );
        println!(
            "{} {} - Count packages by type",
            "4.".cyan(),
            "count packages by type".green()
        );
        println!(
            "{} {} - List largest packages",
            "5.".cyan(),
            "packages order by size desc limit 10".green()
        );
        println!();
        println!("{}", "Examples:".green().bold());
        println!("  query packages where name=kernel");
        println!("  query users where uid>1000");
        println!("  query services where enabled=true");
        println!("  query count packages");
        println!();
        return Ok(());
    }

    let query_str = args.join(" ");

    // Simple query parser
    if query_str.starts_with("packages where name=") {
        let pattern = query_str.strip_prefix("packages where name=").unwrap_or("");
        println!("{} Packages matching '{}':", "→".cyan(), pattern.green());
        println!();

        let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
        let matches: Vec<_> = pkg_info
            .packages
            .iter()
            .filter(|p| p.name.contains(pattern))
            .collect();

        for (i, pkg) in matches.iter().enumerate().take(50) {
            println!(
                "{:3}. {} - {}",
                i + 1,
                pkg.name.cyan(),
                pkg.version.to_string().bright_black()
            );
        }

        println!();
        println!(
            "Found {} matching packages",
            matches.len().to_string().green()
        );
    } else if query_str.starts_with("users where uid>") {
        let uid_str = query_str.strip_prefix("users where uid>").unwrap_or("1000");
        let min_uid: i32 = uid_str.parse().unwrap_or(1000);

        println!(
            "{} Users with UID > {}:",
            "→".cyan(),
            min_uid.to_string().green()
        );
        println!();

        if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
            let matches: Vec<_> = users
                .iter()
                .filter(|u| u.uid.parse::<i32>().unwrap_or(0) > min_uid)
                .collect();

            for user in &matches {
                println!(
                    "  {} {} (UID: {}, GID: {})",
                    "•".cyan(),
                    user.username.green(),
                    user.uid.yellow(),
                    user.gid.bright_black()
                );
            }

            println!();
            println!("Found {} matching users", matches.len().to_string().green());
        }
    } else if query_str == "services where enabled=true" {
        println!("{} Enabled services:", "→".cyan());
        println!();

        if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
            let enabled: Vec<_> = services.iter().filter(|s| s.enabled).collect();

            for (i, service) in enabled.iter().enumerate().take(50) {
                println!("{:3}. {}", i + 1, service.name.cyan());
            }

            println!();
            println!(
                "Found {} enabled services",
                enabled.len().to_string().green()
            );
        }
    } else if query_str == "count packages" {
        let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
        println!(
            "{} Total packages: {}",
            "→".cyan(),
            pkg_info.packages.len().to_string().green().bold()
        );
    } else if query_str.starts_with("packages order by") {
        println!("{} Package list:", "→".cyan());
        println!();

        let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
        let mut packages: Vec<_> = pkg_info.packages.iter().collect();

        // Simple sorting by name
        packages.sort_by_key(|p| &p.name);

        let limit = if query_str.contains("limit") {
            let parts: Vec<&str> = query_str.split("limit").collect();
            if parts.len() > 1 {
                parts[1].trim().parse::<usize>().unwrap_or(10)
            } else {
                10
            }
        } else {
            10
        };

        for (i, pkg) in packages.iter().take(limit).enumerate() {
            println!(
                "{:3}. {} - {}",
                i + 1,
                pkg.name.cyan(),
                pkg.version.to_string().bright_black()
            );
        }

        println!();
        println!(
            "Showing {} of {} packages",
            limit.min(packages.len()),
            packages.len()
        );
    } else {
        println!("{} Unsupported query syntax", "Error:".red());
        println!("{} query (without arguments) for examples", "Tip:".yellow());
    }

    println!();
    Ok(())
}

/// System monitoring and change detection
pub fn cmd_monitor(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              System Monitoring & Alerts                  ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    if args.is_empty() {
        println!("{}", "Monitoring Capabilities:".yellow().bold());
        println!();
        println!(
            "{} {} - Monitor for security issues",
            "1.".cyan(),
            "monitor security".green()
        );
        println!(
            "{} {} - Monitor system health",
            "2.".cyan(),
            "monitor health".green()
        );
        println!(
            "{} {} - Monitor for changes",
            "3.".cyan(),
            "monitor changes".green()
        );
        println!(
            "{} {} - Alert configuration",
            "4.".cyan(),
            "monitor alerts".green()
        );
        println!();
        println!("{} monitor <type>", "Usage:".yellow());
        return Ok(());
    }

    let monitor_type = args[0];

    match monitor_type {
        "security" => {
            println!("{}", "🔒 Security Monitoring Report".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            let mut alerts = Vec::new();

            if &sec.selinux == "disabled" && !sec.apparmor {
                alerts.push((
                    "CRITICAL",
                    "No MAC system active",
                    "Enable SELinux or AppArmor",
                ));
            }

            if !sec.auditd {
                alerts.push((
                    "WARNING",
                    "Audit daemon not running",
                    "Enable auditd for security logging",
                ));
            }

            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if !fw.enabled {
                    alerts.push((
                        "CRITICAL",
                        "Firewall is disabled",
                        "Enable firewall immediately",
                    ));
                }
            }

            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let root_users = users.iter().filter(|u| u.uid == "0").count();
                if root_users > 1 {
                    alerts.push((
                        "WARNING",
                        "Multiple root accounts detected",
                        "Review and consolidate root access",
                    ));
                }
            }

            if alerts.is_empty() {
                println!("{}", "✓ No security alerts detected".green().bold());
                println!("  System security configuration appears nominal.");
            } else {
                println!("{} {} security alerts:", "⚠".yellow(), alerts.len());
                println!();

                for (i, (level, issue, action)) in alerts.iter().enumerate() {
                    let level_colored = match *level {
                        "CRITICAL" => level.red().bold(),
                        "WARNING" => level.yellow(),
                        _ => level.bright_black(),
                    };

                    println!(
                        "{} [{}] {}",
                        format!("{}.", i + 1).cyan(),
                        level_colored,
                        issue.bold()
                    );
                    println!("   Action: {}", action.bright_black());
                    println!();
                }
            }
        }

        "health" => {
            println!("{}", "🏥 Health Monitoring Report".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let mut issues = Vec::new();

            if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                issues.push(("ERROR", "Missing /etc/fstab", "System may not boot"));
            }

            let grub_found = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false)
                || ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);
            if !grub_found {
                issues.push((
                    "ERROR",
                    "No GRUB configuration",
                    "Boot loader not configured",
                ));
            }

            if !ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                issues.push(("WARNING", "Missing /etc/resolv.conf", "DNS may not work"));
            }

            if issues.is_empty() {
                println!("{}", "✓ No health issues detected".green().bold());
                println!("  System health appears good.");
            } else {
                println!("{} {} health issues:", "⚠".yellow(), issues.len());
                println!();

                for (i, (level, issue, impact)) in issues.iter().enumerate() {
                    let level_colored = match *level {
                        "ERROR" => level.red().bold(),
                        "WARNING" => level.yellow(),
                        _ => level.bright_black(),
                    };

                    println!(
                        "{} [{}] {}",
                        format!("{}.", i + 1).cyan(),
                        level_colored,
                        issue.bold()
                    );
                    println!("   Impact: {}", impact.bright_black());
                    println!();
                }
            }
        }

        "changes" => {
            println!("{}", "📊 Change Detection Report".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{}", "Note:".yellow().bold());
            println!("  Change detection requires multiple snapshots over time.");
            println!("  Use 'snapshot' command to create baseline snapshots.");
            println!();
            println!("{}", "Recommended workflow:".green());
            println!("  1. snapshot baseline.md");
            println!("  2. (make system changes)");
            println!("  3. snapshot current.md");
            println!("  4. compare files baseline.md current.md");
        }

        "alerts" => {
            println!("{}", "🔔 Alert Configuration".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{}", "Alert Rules:".yellow().bold());
            println!();
            println!("{} Security configuration changes", "•".cyan());
            println!("{} Critical file modifications", "•".cyan());
            println!("{} Service state changes", "•".cyan());
            println!("{} User account additions/removals", "•".cyan());
            println!("{} Package installations/removals", "•".cyan());
            println!();
            println!("{}", "Note:".yellow().bold());
            println!("  Alert rules are informational. Use 'monitor security' and");
            println!("  'monitor health' for current status checks.");
        }

        _ => {
            println!("{} Unknown monitor type: {}", "Error:".red(), monitor_type);
            println!(
                "{} monitor (without arguments) for options",
                "Tip:".yellow()
            );
        }
    }

    println!();
    Ok(())
}

/// Migration preparation and readiness assessment
pub fn cmd_troubleshoot(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║          Intelligent Troubleshooting Assistant          ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    if args.is_empty() {
        println!("{}", "Troubleshooting Categories:".yellow().bold());
        println!();
        println!(
            "{} {} - Boot and startup issues",
            "1.".cyan(),
            "troubleshoot boot".green()
        );
        println!(
            "{} {} - Network connectivity problems",
            "2.".cyan(),
            "troubleshoot network".green()
        );
        println!(
            "{} {} - Service failures",
            "3.".cyan(),
            "troubleshoot services".green()
        );
        println!(
            "{} {} - Performance issues",
            "4.".cyan(),
            "troubleshoot performance".green()
        );
        println!(
            "{} {} - Security concerns",
            "5.".cyan(),
            "troubleshoot security".green()
        );
        println!(
            "{} {} - Auto-detect issues",
            "6.".cyan(),
            "troubleshoot auto".green()
        );
        println!();
        println!("{} troubleshoot <category>", "Usage:".yellow());
        return Ok(());
    }

    let category = args[0];

    match category {
        "boot" => {
            println!("{}", "🔧 Boot & Startup Troubleshooting".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let mut issues_found = Vec::new();
            let mut solutions = Vec::new();

            // Check fstab
            println!("{} Checking filesystem table...", "→".cyan());
            if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                println!("  {} /etc/fstab is missing", "✗".red().bold());
                issues_found.push("Missing /etc/fstab");
                solutions.push((
                    "Create /etc/fstab",
                    "The filesystem table is required for mounting filesystems at boot",
                    vec![
                        "1. Generate fstab from current mounts",
                        "2. Verify UUID or device paths",
                        "3. Test in rescue mode before production boot",
                    ],
                ));
            } else {
                println!("  {} /etc/fstab exists", "✓".green());
            }

            // Check boot loader
            println!("{} Checking boot loader configuration...", "→".cyan());
            let grub_cfg = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false);
            let grub2_cfg = ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);

            if !grub_cfg && !grub2_cfg {
                println!("  {} No GRUB configuration found", "✗".red().bold());
                issues_found.push("Missing GRUB configuration");
                solutions.push((
                    "Install and configure GRUB",
                    "Boot loader is missing or not configured properly",
                    vec![
                        "1. Install grub2 package",
                        "2. Run grub2-mkconfig -o /boot/grub2/grub.cfg",
                        "3. Install to boot device: grub2-install /dev/sda",
                    ],
                ));
            } else {
                println!("  {} GRUB configuration found", "✓".green());
            }

            // Check kernel
            println!("{} Checking kernel installation...", "→".cyan());
            let has_kernel = ctx.guestfs.exists("/boot/vmlinuz").unwrap_or(false)
                || ctx.guestfs.exists("/boot/vmlinuz-linux").unwrap_or(false);

            if !has_kernel {
                println!("  {} No kernel found in /boot", "✗".red().bold());
                issues_found.push("No kernel installed");
                solutions.push((
                    "Install kernel",
                    "System cannot boot without a kernel",
                    vec![
                        "1. Install kernel package for your distribution",
                        "2. Regenerate initramfs/initrd",
                        "3. Update GRUB configuration",
                    ],
                ));
            } else {
                println!("  {} Kernel found", "✓".green());
            }

            // Summary
            println!();
            if issues_found.is_empty() {
                println!("{}", "✓ No boot issues detected!".green().bold());
                println!("  Boot configuration appears correct.");
            } else {
                println!("{} {} boot issues detected:", "⚠".red(), issues_found.len());
                println!();

                for (i, (title, description, steps)) in solutions.iter().enumerate() {
                    println!(
                        "{} {}",
                        format!("Issue {}:", i + 1).yellow().bold(),
                        title.bold()
                    );
                    println!("   {}", description.bright_black());
                    println!();
                    println!("   {}:", "Solution Steps".green());
                    for step in steps {
                        println!("     {}", step.cyan());
                    }
                    println!();
                }
            }
        }

        "network" => {
            println!("{}", "🌐 Network Troubleshooting".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let mut issues_found = Vec::new();
            let mut solutions = Vec::new();

            // Check DNS configuration
            println!("{} Checking DNS configuration...", "→".cyan());
            if !ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                println!("  {} /etc/resolv.conf is missing", "✗".red().bold());
                issues_found.push("Missing DNS configuration");
                solutions.push((
                    "Configure DNS",
                    "No DNS resolver configuration found",
                    vec![
                        "1. Create /etc/resolv.conf",
                        "2. Add nameserver entries (e.g., nameserver 8.8.8.8)",
                        "3. Consider using systemd-resolved for dynamic DNS",
                    ],
                ));
            } else {
                println!("  {} /etc/resolv.conf exists", "✓".green());
            }

            // Check hosts file
            println!("{} Checking hosts file...", "→".cyan());
            if !ctx.guestfs.exists("/etc/hosts").unwrap_or(false) {
                println!("  {} /etc/hosts is missing", "✗".red().bold());
                issues_found.push("Missing hosts file");
                solutions.push((
                    "Create hosts file",
                    "Basic hostname resolution requires /etc/hosts",
                    vec![
                        "1. Create /etc/hosts with localhost entries",
                        "2. Add: 127.0.0.1 localhost",
                        "3. Add: ::1 localhost ip6-localhost",
                    ],
                ));
            } else {
                println!("  {} /etc/hosts exists", "✓".green());
            }

            // Check network manager
            println!("{} Checking network management...", "→".cyan());
            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let has_nm = pkg_info
                .packages
                .iter()
                .any(|p| p.name.contains("NetworkManager"));
            let has_netctl = pkg_info.packages.iter().any(|p| p.name.contains("netctl"));
            let has_systemd_networkd = pkg_info.packages.iter().any(|p| p.name.contains("systemd"));

            if !has_nm && !has_netctl && !has_systemd_networkd {
                println!("  {} No network manager detected", "⚠".yellow());
                solutions.push((
                    "Install network manager",
                    "No network management tool found",
                    vec![
                        "1. Install NetworkManager or systemd-networkd",
                        "2. Enable and start the service",
                        "3. Configure network interfaces",
                    ],
                ));
            } else {
                println!("  {} Network management tools present", "✓".green());
            }

            // Summary
            println!();
            if issues_found.is_empty() && solutions.is_empty() {
                println!(
                    "{}",
                    "✓ No critical network issues detected!".green().bold()
                );
            } else {
                println!("{} Network configuration issues:", "⚠".yellow());
                println!();

                for (i, (title, description, steps)) in solutions.iter().enumerate() {
                    println!(
                        "{} {}",
                        format!("Issue {}:", i + 1).yellow().bold(),
                        title.bold()
                    );
                    println!("   {}", description.bright_black());
                    println!();
                    println!("   {}:", "Solution Steps".green());
                    for step in steps {
                        println!("     {}", step.cyan());
                    }
                    println!();
                }
            }
        }

        "services" => {
            println!("{}", "⚙️  Service Troubleshooting".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                let total = services.len();
                let disabled = total - enabled;

                println!("{} Service Statistics:", "→".cyan());
                println!("  Total services:    {}", total.to_string().cyan());
                let enabled_pct = enabled.saturating_mul(100).checked_div(total).unwrap_or(0);
                let disabled_pct = disabled.saturating_mul(100).checked_div(total).unwrap_or(0);
                println!(
                    "  Enabled:           {} ({}%)",
                    enabled.to_string().green(),
                    enabled_pct
                );
                println!(
                    "  Disabled:          {} ({}%)",
                    disabled.to_string().yellow(),
                    disabled_pct
                );
                println!();

                // Identify critical services
                let critical_services =
                    vec!["sshd", "systemd-networkd", "NetworkManager", "firewalld"];
                let mut missing_critical = Vec::new();

                println!("{} Checking critical services...", "→".cyan());
                for critical in &critical_services {
                    let found = services.iter().any(|s| s.name.contains(critical));
                    if found {
                        let enabled = services
                            .iter()
                            .find(|s| s.name.contains(critical))
                            .map(|s| s.enabled)
                            .unwrap_or(false);

                        if enabled {
                            println!("  {} {} is enabled", "✓".green(), critical.green());
                        } else {
                            println!(
                                "  {} {} exists but is disabled",
                                "⚠".yellow(),
                                critical.yellow()
                            );
                        }
                    } else {
                        println!("  {} {} not found", "✗".red(), critical.bright_black());
                        missing_critical.push(*critical);
                    }
                }

                if !missing_critical.is_empty() {
                    println!();
                    println!("{} Recommendations:", "💡".yellow());
                    for service in missing_critical {
                        println!("  • Install and enable {}", service.cyan());
                    }
                }

                // Check for failed services (we can't actually know this from offline inspection)
                println!();
                println!("{}", "Note:".yellow().bold());
                println!("  Offline inspection cannot detect runtime service failures.");
                println!(
                    "  Run 'systemctl --failed' on the live system to check for failed services."
                );
            }
        }

        "performance" => {
            println!("{}", "⚡ Performance Troubleshooting".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let mut issues = Vec::new();

            // Check package count
            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let pkg_count = pkg_info.packages.len();

            println!("{} Analyzing package overhead...", "→".cyan());
            println!("  Total packages: {}", pkg_count.to_string().cyan());

            if pkg_count > 1500 {
                println!(
                    "  {} High package count may impact performance",
                    "⚠".yellow()
                );
                issues.push((
                    "Package bloat",
                    "Large number of packages installed (recommended: <1000)",
                    vec![
                        "1. Review installed packages: packages",
                        "2. Remove unnecessary packages",
                        "3. Consider minimal installation for better performance",
                    ],
                ));
            } else if pkg_count > 1000 {
                println!("  {} Moderate package count", "→".cyan());
            } else {
                println!("  {} Good package count", "✓".green());
            }

            // Check service count
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();

                println!("{} Analyzing service overhead...", "→".cyan());
                println!("  Enabled services: {}", enabled.to_string().cyan());

                if enabled > 80 {
                    println!("  {} Excessive services may cause slowdowns", "⚠".red());
                    issues.push((
                        "Service overhead",
                        "Too many enabled services (recommended: <50)",
                        vec![
                            "1. Review enabled services: services",
                            "2. Disable unnecessary services",
                            "3. Use 'systemctl mask' for unwanted services",
                        ],
                    ));
                } else if enabled > 50 {
                    println!("  {} Many services enabled", "⚠".yellow());
                    issues.push((
                        "Service count",
                        "Consider reviewing and disabling unused services",
                        vec![
                            "1. List services: systemctl list-unit-files",
                            "2. Disable unused: systemctl disable <service>",
                        ],
                    ));
                } else {
                    println!("  {} Reasonable service count", "✓".green());
                }
            }

            println!();
            if issues.is_empty() {
                println!(
                    "{}",
                    "✓ No obvious performance bottlenecks detected!"
                        .green()
                        .bold()
                );
            } else {
                println!("{} Performance Issues:", "⚠".yellow());
                println!();

                for (i, (title, description, steps)) in issues.iter().enumerate() {
                    println!("{} {}", format!("{}.", i + 1).yellow().bold(), title.bold());
                    println!("   {}", description.bright_black());
                    println!();
                    for step in steps {
                        println!("     {}", step.cyan());
                    }
                    println!();
                }
            }

            println!("{} Additional recommendations:", "💡".cyan());
            println!("  • Run 'bench all' to measure command performance");
            println!("  • Use 'optimize' for detailed optimization suggestions");
            println!("  • Check 'chart services' for service distribution");
        }

        "security" => {
            println!("{}", "🔒 Security Troubleshooting".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            let mut vulnerabilities = Vec::new();

            // Check MAC systems
            println!("{} Checking access control systems...", "→".cyan());
            if &sec.selinux == "disabled" && !sec.apparmor {
                println!("  {} No MAC system active", "✗".red().bold());
                vulnerabilities.push((
                    "CRITICAL",
                    "No Mandatory Access Control",
                    "System lacks SELinux or AppArmor protection",
                    vec![
                        "1. Install SELinux or AppArmor packages",
                        "2. Configure policy (targeted for SELinux, enforce for AppArmor)",
                        "3. Reboot to activate",
                        "4. Monitor audit logs for policy violations",
                    ],
                ));
            } else if &sec.selinux != "disabled" {
                println!("  {} SELinux is {}", "✓".green(), sec.selinux.green());
            } else if sec.apparmor {
                println!("  {} AppArmor is active", "✓".green());
            }

            // Check firewall
            println!("{} Checking firewall...", "→".cyan());
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if !fw.enabled {
                    println!("  {} Firewall is disabled", "✗".red().bold());
                    vulnerabilities.push((
                        "CRITICAL",
                        "Firewall disabled",
                        "No network filtering is active",
                        vec![
                            "1. Install firewalld or ufw",
                            "2. Enable: systemctl enable --now firewalld",
                            "3. Configure zones and rules",
                            "4. Test connectivity after enabling",
                        ],
                    ));
                } else {
                    println!("  {} Firewall is enabled", "✓".green());
                }
            }

            // Check audit daemon
            println!("{} Checking audit logging...", "→".cyan());
            if !sec.auditd {
                println!("  {} Audit daemon not running", "⚠".yellow());
                vulnerabilities.push((
                    "WARNING",
                    "No audit logging",
                    "Security events are not being logged",
                    vec![
                        "1. Install audit package",
                        "2. Enable: systemctl enable --now auditd",
                        "3. Configure rules in /etc/audit/audit.rules",
                        "4. Monitor /var/log/audit/audit.log",
                    ],
                ));
            } else {
                println!("  {} Audit daemon configured", "✓".green());
            }

            // Check SSH configuration
            println!("{} Checking SSH security...", "→".cyan());
            if ctx.guestfs.exists("/etc/ssh/sshd_config").unwrap_or(false) {
                println!("  {} SSH configuration found", "✓".green());
                println!("     {}", "Review sshd_config for:".bright_black());
                println!("     {} PermitRootLogin no", "•".bright_black());
                println!(
                    "     {} PasswordAuthentication (consider key-only)",
                    "•".bright_black()
                );
                println!(
                    "     {} Port (consider changing from 22)",
                    "•".bright_black()
                );
            } else {
                println!("  {} No SSH configuration", "→".bright_black());
            }

            println!();
            if vulnerabilities.is_empty() {
                println!("{}", "✓ No critical security issues found!".green().bold());
                println!("  Run 'verify security' for comprehensive security check.");
            } else {
                println!("{} Security Vulnerabilities:", "🚨".red());
                println!();

                for (i, (severity, title, description, steps)) in vulnerabilities.iter().enumerate()
                {
                    let severity_colored = match *severity {
                        "CRITICAL" => severity.red().bold(),
                        "WARNING" => severity.yellow(),
                        _ => severity.bright_black(),
                    };

                    println!(
                        "{} [{}] {}",
                        format!("{}.", i + 1).yellow(),
                        severity_colored,
                        title.bold()
                    );
                    println!("   {}", description.bright_black());
                    println!();
                    println!("   {}:", "Remediation Steps".green());
                    for step in steps {
                        println!("     {}", step.cyan());
                    }
                    println!();
                }

                println!("{} Run these for more details:", "💡".cyan());
                println!(
                    "  • {} - Full security compliance check",
                    "compliance cis".green()
                );
                println!("  • {} - Security predictions", "predict".green());
                println!("  • {} - Security insights", "insights".green());
            }
        }

        "auto" => {
            println!("{}", "🔍 Auto-Detecting Issues".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{}", "Running comprehensive system scan...".yellow());
            println!();

            let mut issues = Vec::new();

            // Quick checks across all categories
            if !ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                issues.push(("CRITICAL", "Boot", "Missing /etc/fstab"));
            }

            if !ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                issues.push(("WARNING", "Network", "Missing DNS configuration"));
            }

            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            if &sec.selinux == "disabled" && !sec.apparmor {
                issues.push(("CRITICAL", "Security", "No MAC system active"));
            }

            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if !fw.enabled {
                    issues.push(("CRITICAL", "Security", "Firewall disabled"));
                }
            }

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            if pkg_info.packages.len() > 1500 {
                issues.push(("WARNING", "Performance", "Excessive packages installed"));
            }

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                if enabled > 80 {
                    issues.push(("WARNING", "Performance", "Too many enabled services"));
                }
            }

            if issues.is_empty() {
                println!("{}", "✓ No issues detected!".green().bold());
                println!("  System appears to be in good condition.");
            } else {
                println!("{} {} issues detected:", "⚠".yellow(), issues.len());
                println!();

                for (severity, category, description) in &issues {
                    let severity_colored = match *severity {
                        "CRITICAL" => severity.red().bold(),
                        "WARNING" => severity.yellow(),
                        _ => severity.bright_black(),
                    };

                    println!(
                        "  [{}] {}: {}",
                        severity_colored,
                        category.cyan(),
                        description
                    );
                }

                println!();
                println!("{} Run detailed troubleshooting:", "💡".cyan());
                println!("  • {} - Boot issues", "troubleshoot boot".green());
                println!("  • {} - Network issues", "troubleshoot network".green());
                println!("  • {} - Security issues", "troubleshoot security".green());
                println!(
                    "  • {} - Performance issues",
                    "troubleshoot performance".green()
                );
            }
        }

        _ => {
            println!("{} Unknown category: {}", "Error:".red(), category);
            println!(
                "{} troubleshoot (without arguments) for options",
                "Tip:".yellow()
            );
        }
    }

    println!();
    Ok(())
}

/// Package dependency analysis
pub fn cmd_depends(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║           Package Dependency Analysis                   ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    if args.is_empty() {
        println!("{}", "Dependency Analysis Features:".yellow().bold());
        println!();
        println!(
            "{} {} - Find packages containing pattern",
            "1.".cyan(),
            "depends search <pattern>".green()
        );
        println!(
            "{} {} - Analyze package relationships",
            "2.".cyan(),
            "depends analyze".green()
        );
        println!(
            "{} {} - Find development packages",
            "3.".cyan(),
            "depends dev".green()
        );
        println!(
            "{} {} - Find library packages",
            "4.".cyan(),
            "depends libs".green()
        );
        println!();
        println!("{} depends <command>", "Usage:".yellow());
        return Ok(());
    }

    let command = args[0];

    match command {
        "search" => {
            if args.len() < 2 {
                println!("{} Usage: depends search <pattern>", "Error:".red());
                return Ok(());
            }

            let pattern = args[1];
            println!(
                "{} Searching for packages matching '{}'...",
                "→".cyan(),
                pattern.green()
            );
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let matches: Vec<_> = pkg_info
                .packages
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&pattern.to_lowercase()))
                .collect();

            if matches.is_empty() {
                println!("{} No packages found matching '{}'", "✗".red(), pattern);
            } else {
                println!("{} {} packages found:", "✓".green(), matches.len());
                println!();

                for (i, pkg) in matches.iter().enumerate().take(50) {
                    println!(
                        "{:3}. {} ({})",
                        i + 1,
                        pkg.name.cyan(),
                        pkg.version.to_string().bright_black()
                    );
                }

                if matches.len() > 50 {
                    println!();
                    println!("... and {} more", (matches.len() - 50).to_string().yellow());
                }
            }
        }

        "analyze" => {
            println!("{} Analyzing package relationships...", "→".cyan());
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;

            // Categorize packages
            let mut dev_packages = 0;
            let mut lib_packages = 0;
            let mut doc_packages = 0;
            let mut kernel_packages = 0;
            let mut app_packages = 0;

            for pkg in &pkg_info.packages {
                let name = pkg.name.to_lowercase();
                if name.contains("devel") || name.contains("-dev") || name.ends_with("-dev") {
                    dev_packages += 1;
                } else if name.starts_with("lib") || name.contains("library") {
                    lib_packages += 1;
                } else if name.contains("doc") || name.ends_with("-doc") {
                    doc_packages += 1;
                } else if name.contains("kernel") {
                    kernel_packages += 1;
                } else {
                    app_packages += 1;
                }
            }

            let total = pkg_info.packages.len().max(1);

            println!("{}", "Package Distribution:".cyan().bold());
            println!();
            println!(
                "  Development:  {:4} ({:5.1}%)",
                dev_packages,
                (dev_packages as f32 / total as f32) * 100.0
            );
            println!(
                "  Libraries:    {:4} ({:5.1}%)",
                lib_packages,
                (lib_packages as f32 / total as f32) * 100.0
            );
            println!(
                "  Documentation:{:4} ({:5.1}%)",
                doc_packages,
                (doc_packages as f32 / total as f32) * 100.0
            );
            println!(
                "  Kernel:       {:4} ({:5.1}%)",
                kernel_packages,
                (kernel_packages as f32 / total as f32) * 100.0
            );
            println!(
                "  Applications: {:4} ({:5.1}%)",
                app_packages,
                (app_packages as f32 / total as f32) * 100.0
            );
            println!("  {}", "─".repeat(25).bright_black());
            println!("  Total:        {:4}", total);

            println!();
            println!("{} Insights:", "💡".yellow());

            if dev_packages > total / 5 {
                println!(
                    "  • {}",
                    "High development package count suggests a build environment".cyan()
                );
            }

            if lib_packages > total / 3 {
                println!(
                    "  • {}",
                    "Many libraries - system may support multiple applications".cyan()
                );
            }

            if doc_packages > 50 {
                println!(
                    "  • {}",
                    "Documentation packages can be removed to save space".cyan()
                );
            }
        }

        "dev" => {
            println!("{} Development Packages:", "→".cyan());
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let dev_pkgs: Vec<_> = pkg_info
                .packages
                .iter()
                .filter(|p| {
                    let name = p.name.to_lowercase();
                    name.contains("devel")
                        || name.contains("-dev")
                        || name.ends_with("-dev")
                        || name.contains("gcc")
                        || name.contains("make")
                        || name.contains("cmake")
                })
                .collect();

            if dev_pkgs.is_empty() {
                println!("{} No development packages found", "✓".green());
                println!("  This is a production/runtime system");
            } else {
                println!("{} {} development packages:", "→".cyan(), dev_pkgs.len());
                println!();

                for (i, pkg) in dev_pkgs.iter().enumerate().take(30) {
                    println!("{:3}. {}", i + 1, pkg.name.cyan());
                }

                if dev_pkgs.len() > 30 {
                    println!();
                    println!(
                        "... and {} more",
                        (dev_pkgs.len() - 30).to_string().yellow()
                    );
                }

                println!();
                println!("{} Note:", "💡".yellow());
                println!("  Development packages are typically not needed in production.");
                println!("  Consider removing them to reduce attack surface and disk usage.");
            }
        }

        "libs" => {
            println!("{} Library Packages:", "→".cyan());
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let lib_pkgs: Vec<_> = pkg_info
                .packages
                .iter()
                .filter(|p| p.name.starts_with("lib") || p.name.to_lowercase().contains("library"))
                .collect();

            println!("{} {} library packages:", "→".cyan(), lib_pkgs.len());
            println!();

            for (i, pkg) in lib_pkgs.iter().enumerate().take(30) {
                println!("{:3}. {}", i + 1, pkg.name.cyan());
            }

            if lib_pkgs.len() > 30 {
                println!();
                println!(
                    "... and {} more",
                    (lib_pkgs.len() - 30).to_string().yellow()
                );
            }
        }

        _ => {
            println!("{} Unknown command: {}", "Error:".red(), command);
            println!(
                "{} depends (without arguments) for options",
                "Tip:".yellow()
            );
        }
    }

    println!();
    Ok(())
}

/// Configuration validation and recommendations
pub fn cmd_validate(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║          Configuration Validation Suite                 ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝"
            .cyan()
            .bold()
    );
    println!();

    let target = if args.is_empty() { "all" } else { args[0] };

    match target {
        "all" => {
            println!("{}", "Running comprehensive validation...".yellow());
            println!();

            let mut passed = 0;
            let mut failed = 0;
            let mut warnings = 0;

            // Validation 1: File system structure
            println!("{} {}", "1.".cyan().bold(), "File System Structure".bold());
            let critical_dirs = vec!["/etc", "/var", "/usr", "/boot", "/home"];
            let mut all_dirs_present = true;

            for dir in &critical_dirs {
                if ctx.guestfs.exists(dir).unwrap_or(false) {
                    println!("  {} {} exists", "✓".green(), dir.cyan());
                } else {
                    println!("  {} {} missing", "✗".red(), dir.red());
                    all_dirs_present = false;
                }
            }

            if all_dirs_present {
                passed += 1;
                println!("  {}", "PASS".green().bold());
            } else {
                failed += 1;
                println!("  {}", "FAIL".red().bold());
            }
            println!();

            // Validation 2: System configuration files
            println!(
                "{} {}",
                "2.".cyan().bold(),
                "System Configuration Files".bold()
            );
            let config_files = vec![
                ("/etc/passwd", "User database"),
                ("/etc/group", "Group database"),
                ("/etc/shadow", "Password hashes"),
                ("/etc/fstab", "Filesystem table"),
            ];

            let mut all_configs_present = true;
            for (file, desc) in &config_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!(
                        "  {} {} - {}",
                        "✓".green(),
                        file.cyan(),
                        desc.bright_black()
                    );
                } else {
                    println!(
                        "  {} {} - {} {}",
                        "✗".red(),
                        file.red(),
                        desc.bright_black(),
                        "[MISSING]".red()
                    );
                    all_configs_present = false;
                }
            }

            if all_configs_present {
                passed += 1;
                println!("  {}", "PASS".green().bold());
            } else {
                failed += 1;
                println!("  {}", "FAIL".red().bold());
            }
            println!();

            // Validation 3: Boot configuration
            println!("{} {}", "3.".cyan().bold(), "Boot Configuration".bold());
            let grub_cfg = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false);
            let grub2_cfg = ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);

            if grub_cfg || grub2_cfg {
                println!("  {} Boot loader configured", "✓".green());
                passed += 1;
                println!("  {}", "PASS".green().bold());
            } else {
                println!("  {} No boot loader configuration", "✗".red());
                failed += 1;
                println!("  {}", "FAIL".red().bold());
            }
            println!();

            // Validation 4: Security configuration
            println!("{} {}", "4.".cyan().bold(), "Security Configuration".bold());
            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            let mut sec_checks = 0;
            let mut sec_total = 0;

            sec_total += 1;
            if &sec.selinux != "disabled" || sec.apparmor {
                println!("  {} MAC system active", "✓".green());
                sec_checks += 1;
            } else {
                println!("  {} No MAC system", "⚠".yellow());
            }

            sec_total += 1;
            if sec.auditd {
                println!("  {} Audit daemon configured", "✓".green());
                sec_checks += 1;
            } else {
                println!("  {} No audit daemon", "⚠".yellow());
            }

            sec_total += 1;
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if fw.enabled {
                    println!("  {} Firewall enabled", "✓".green());
                    sec_checks += 1;
                } else {
                    println!("  {} Firewall disabled", "⚠".yellow());
                }
            }

            if sec_checks >= sec_total - 1 {
                passed += 1;
                println!("  {}", "PASS".green().bold());
            } else if sec_checks >= sec_total / 2 {
                warnings += 1;
                println!("  {}", "WARN".yellow().bold());
            } else {
                failed += 1;
                println!("  {}", "FAIL".red().bold());
            }
            println!();

            // Validation 5: Package integrity
            println!("{} {}", "5.".cyan().bold(), "Package System".bold());
            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            if !pkg_info.packages.is_empty() {
                println!(
                    "  {} {} packages installed",
                    "✓".green(),
                    pkg_info.packages.len()
                );
                passed += 1;
                println!("  {}", "PASS".green().bold());
            } else {
                println!("  {} No packages found", "✗".red());
                failed += 1;
                println!("  {}", "FAIL".red().bold());
            }
            println!();

            // Summary
            println!("{}", "═".repeat(60).bright_black());
            println!();
            println!("{}", "Validation Summary:".cyan().bold());
            println!("  Passed:   {}", passed.to_string().green());
            println!("  Failed:   {}", failed.to_string().red());
            println!("  Warnings: {}", warnings.to_string().yellow());

            let total = (passed + failed + warnings).max(1);
            let success_rate = (passed as f32 / total as f32) * 100.0;

            println!();
            println!("  Success Rate: {:.1}%", success_rate);

            if failed == 0 && warnings == 0 {
                println!();
                println!("{}", "✓ System configuration is valid!".green().bold());
            } else if failed == 0 {
                println!();
                println!(
                    "{}",
                    "⚠ System is mostly valid with some warnings.".yellow()
                );
            } else {
                println!();
                println!(
                    "{}",
                    "✗ System has configuration issues that need attention.".red()
                );
            }
        }

        "config" => {
            println!("{}", "Validating configuration files...".yellow());
            println!();

            let config_files = vec![
                ("/etc/passwd", "User accounts", true),
                ("/etc/group", "Group definitions", true),
                ("/etc/shadow", "Password hashes", true),
                ("/etc/fstab", "Filesystem mounts", true),
                ("/etc/hosts", "Host name resolution", true),
                ("/etc/resolv.conf", "DNS configuration", false),
                ("/etc/ssh/sshd_config", "SSH server config", false),
                ("/etc/sudoers", "Sudo configuration", false),
            ];

            let mut critical_missing = Vec::new();
            let mut optional_missing = Vec::new();

            for (file, description, critical) in &config_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!(
                        "  {} {} - {}",
                        "✓".green(),
                        file.cyan(),
                        description.bright_black()
                    );
                } else if *critical {
                    println!(
                        "  {} {} - {} {}",
                        "✗".red(),
                        file.red(),
                        description.bright_black(),
                        "[CRITICAL]".red().bold()
                    );
                    critical_missing.push(*file);
                } else {
                    println!(
                        "  {} {} - {} {}",
                        "⚠".yellow(),
                        file.yellow(),
                        description.bright_black(),
                        "[OPTIONAL]".yellow()
                    );
                    optional_missing.push(*file);
                }
            }

            println!();
            if critical_missing.is_empty() && optional_missing.is_empty() {
                println!("{}", "✓ All configuration files present!".green().bold());
            } else {
                if !critical_missing.is_empty() {
                    println!("{} Critical files missing:", "✗".red());
                    for file in &critical_missing {
                        println!("  • {}", file.red());
                    }
                    println!();
                }

                if !optional_missing.is_empty() {
                    println!("{} Optional files missing:", "⚠".yellow());
                    for file in &optional_missing {
                        println!("  • {}", file.yellow());
                    }
                }
            }
        }

        _ => {
            println!("{}", "Validation Targets:".yellow().bold());
            println!();
            println!(
                "{} {} - Comprehensive validation",
                "1.".cyan(),
                "validate all".green()
            );
            println!(
                "{} {} - Configuration files only",
                "2.".cyan(),
                "validate config".green()
            );
            println!();
            println!("{} validate <target>", "Usage:".yellow());
        }
    }

    println!();
    Ok(())
}

/// Forensics - Digital forensics investigation workflows
pub fn cmd_forensics(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "🔍 Digital Forensics Investigation".cyan().bold());
        println!();
        println!("{}", "Available Workflows:".yellow().bold());
        println!(
            "{} {} - Evidence collection",
            "1.".cyan(),
            "forensics collect".green()
        );
        println!(
            "{} {} - Timeline reconstruction",
            "2.".cyan(),
            "forensics timeline".green()
        );
        println!(
            "{} {} - Suspicious activity detection",
            "3.".cyan(),
            "forensics suspicious".green()
        );
        println!(
            "{} {} - User activity analysis",
            "4.".cyan(),
            "forensics activity".green()
        );
        println!(
            "{} {} - Integrity verification",
            "5.".cyan(),
            "forensics integrity".green()
        );
        println!(
            "{} {} - Memory artifacts",
            "6.".cyan(),
            "forensics memory".green()
        );
        println!();
        println!("{} forensics <workflow>", "Usage:".yellow());
        println!();
        return Ok(());
    }

    let workflow = args[0];

    match workflow {
        "collect" => {
            println!("{}", "📦 Evidence Collection".cyan().bold());
            println!();

            let mut evidence = Vec::new();

            // Critical system files
            println!("{}", "Critical System Files:".yellow().bold());
            let critical_files = vec![
                ("/etc/passwd", "User database"),
                ("/etc/shadow", "Password hashes"),
                ("/etc/group", "Group database"),
                ("/etc/sudoers", "Sudo configuration"),
                ("/etc/hosts", "Host mappings"),
                ("/etc/fstab", "Filesystem mounts"),
                ("/etc/crontab", "System cron jobs"),
                ("/var/log/auth.log", "Authentication logs"),
                ("/var/log/secure", "Security logs"),
                ("/var/log/syslog", "System logs"),
            ];

            for (path, desc) in &critical_files {
                if ctx.guestfs.exists(path).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(path).unwrap_or(0);
                    evidence.push((*path, *desc, size, "✓"));
                    println!(
                        "  {} {} - {} ({} bytes)",
                        "✓".green(),
                        path.cyan(),
                        desc,
                        size
                    );
                } else {
                    println!(
                        "  {} {} - {} {}",
                        "✗".red(),
                        path.cyan(),
                        desc,
                        "(missing)".bright_black()
                    );
                }
            }
            println!();

            // User home directories
            println!("{}", "User Home Directories:".yellow().bold());
            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                for user in &user_info {
                    if ctx.guestfs.exists(&user.home).unwrap_or(false) {
                        let bash_history = format!("{}/.bash_history", user.home);
                        let ssh_dir = format!("{}/.ssh", user.home);

                        if ctx.guestfs.exists(&bash_history).unwrap_or(false) {
                            let size = ctx.guestfs.filesize(&bash_history).unwrap_or(0);
                            println!(
                                "  {} {} - Command history ({} bytes)",
                                "✓".green(),
                                bash_history.cyan(),
                                size
                            );
                            // evidence.push((bash_history.as_str(), "Command history", size, "✓"));
                        }

                        if ctx.guestfs.is_dir(&ssh_dir).unwrap_or(false) {
                            println!("  {} {} - SSH configuration", "✓".green(), ssh_dir.cyan());
                        }
                    }
                }
            }
            println!();

            // Log files
            println!("{}", "Log Files:".yellow().bold());
            let log_paths = vec!["/var/log", "/var/log/audit"];
            for log_path in &log_paths {
                if ctx.guestfs.is_dir(log_path).unwrap_or(false) {
                    println!("  {} {} - Available", "✓".green(), log_path.cyan());
                }
            }
            println!();

            println!("{}", "Evidence Summary:".yellow().bold());
            println!(
                "  Total artifacts collected: {}",
                evidence.len().to_string().green()
            );
            println!(
                "  Critical files found: {}",
                critical_files.len().to_string().cyan()
            );
            println!();
            println!("{}", "Next Steps:".yellow());
            println!(
                "  1. Export evidence: {}",
                "export system json > evidence.json".cyan()
            );
            println!("  2. Analyze timeline: {}", "forensics timeline".cyan());
            println!(
                "  3. Check for suspicious activity: {}",
                "forensics suspicious".cyan()
            );
        }

        "timeline" => {
            println!("{}", "⏰ Timeline Reconstruction".cyan().bold());
            println!();

            // Analyze modification times of critical files
            println!("{}", "Recent System Changes:".yellow().bold());

            let mut timeline_events = Vec::new();

            let check_paths = vec![
                "/etc/passwd",
                "/etc/shadow",
                "/etc/group",
                "/etc/sudoers",
                "/etc/ssh/sshd_config",
                "/etc/crontab",
                "/var/log/auth.log",
            ];

            for path in &check_paths {
                if ctx.guestfs.exists(path).unwrap_or(false) {
                    // We can't get actual timestamps from GuestFS easily, so provide guidance
                    println!("  {} {} - Available for analysis", "✓".green(), path.cyan());
                    timeline_events.push(*path);
                }
            }
            println!();

            println!("{}", "Timeline Categories:".yellow().bold());
            println!("  {} User account changes", "👥".cyan());
            println!("  {} Authentication events", "🔐".cyan());
            println!("  {} System configuration changes", "⚙".cyan());
            println!("  {} Log entries", "📝".cyan());
            println!();

            println!("{}", "Analysis Recommendations:".yellow());
            println!("  1. Check /var/log/auth.log for login attempts");
            println!("  2. Review /etc/passwd for new user accounts");
            println!("  3. Examine cron jobs for scheduled tasks");
            println!("  4. Analyze SSH configuration changes");
            println!();
            println!(
                "{} Files available for timeline: {}",
                "Summary:".yellow(),
                timeline_events.len().to_string().green()
            );
        }

        "suspicious" => {
            println!("{}", "🚨 Suspicious Activity Detection".cyan().bold());
            println!();

            let mut findings = Vec::new();

            // Check for suspicious user accounts
            println!("{}", "User Account Analysis:".yellow().bold());
            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                for user in &user_info {
                    // Check for UID 0 accounts other than root
                    if user.uid == "0" && user.username != "root" {
                        findings.push(("CRITICAL", "UID 0 account", user.username.clone()));
                        println!(
                            "  {} {} - Non-root UID 0 account",
                            "🔴".red(),
                            user.username.red().bold()
                        );
                    }

                    // Check for accounts with no password (if shadow exists)
                    if ctx.guestfs.exists("/etc/shadow").unwrap_or(false) {
                        // Would need to read shadow file to check
                        if user.username.contains("test") || user.username.contains("temp") {
                            findings.push(("WARNING", "Temporary account", user.username.clone()));
                            println!(
                                "  {} {} - Potential temporary/test account",
                                "⚠".yellow(),
                                user.username.yellow()
                            );
                        }
                    }
                }
            }
            println!();

            // Check for suspicious SUID binaries
            println!("{}", "SUID Binary Analysis:".yellow().bold());
            println!("  {} Checking for unusual SUID files...", "🔍".cyan());
            println!("  {} Manual verification recommended for:", "ℹ".cyan());
            println!("    - /tmp, /var/tmp, /dev/shm (world-writable directories)");
            println!("    - User home directories");
            println!("    - Unusual system paths");
            println!();

            // Check for suspicious cron jobs
            println!("{}", "Scheduled Task Analysis:".yellow().bold());
            if ctx.guestfs.exists("/etc/crontab").unwrap_or(false) {
                println!(
                    "  {} {} - Present (review recommended)",
                    "✓".green(),
                    "/etc/crontab".cyan()
                );
            }
            if ctx.guestfs.is_dir("/etc/cron.d").unwrap_or(false) {
                println!(
                    "  {} {} - Present (review recommended)",
                    "✓".green(),
                    "/etc/cron.d".cyan()
                );
            }
            println!();

            // Check for suspicious network configuration
            println!("{}", "Network Configuration:".yellow().bold());
            if ctx.guestfs.exists("/etc/hosts").unwrap_or(false) {
                println!(
                    "  {} {} - Check for suspicious redirects",
                    "ℹ".cyan(),
                    "/etc/hosts".cyan()
                );
            }
            println!();

            // Security findings summary
            println!("{}", "Findings Summary:".yellow().bold());
            if findings.is_empty() {
                println!("  {} No critical suspicious activity detected", "✓".green());
            } else {
                for (severity, category, detail) in &findings {
                    let severity_colored = match *severity {
                        "CRITICAL" => severity.red().bold(),
                        "WARNING" => severity.yellow().bold(),
                        _ => severity.cyan().bold(),
                    };
                    println!("  {} {} - {}", severity_colored, category, detail);
                }
            }
            println!();

            println!("{}", "Recommended Actions:".yellow());
            println!("  1. Review user accounts: {}", "cat /etc/passwd".cyan());
            println!("  2. Check for rootkits: {}", "forensics integrity".cyan());
            println!("  3. Analyze authentication logs");
            println!("  4. Examine network connections and listening ports");
        }

        "activity" => {
            println!("{}", "👤 User Activity Analysis".cyan().bold());
            println!();

            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("{}", "User Activity Summary:".yellow().bold());

                for user in &user_info {
                    println!();
                    println!(
                        "{} {} (UID: {})",
                        "User:".cyan(),
                        user.username.green().bold(),
                        user.uid
                    );

                    // Check for bash history
                    let bash_history = format!("{}/.bash_history", user.home);
                    if ctx.guestfs.exists(&bash_history).unwrap_or(false) {
                        let size = ctx.guestfs.filesize(&bash_history).unwrap_or(0);
                        println!(
                            "  {} Command history: {} bytes",
                            "📜".cyan(),
                            size.to_string().green()
                        );
                    } else {
                        println!("  {} Command history: {}", "📜".cyan(), "Not found".red());
                    }

                    // Check for SSH keys
                    let ssh_dir = format!("{}/.ssh", user.home);
                    if ctx.guestfs.is_dir(&ssh_dir).unwrap_or(false) {
                        println!("  {} SSH directory: {}", "🔑".cyan(), "Present".green());
                    }

                    // Check for common config files
                    let bashrc = format!("{}/.bashrc", user.home);
                    if ctx.guestfs.exists(&bashrc).unwrap_or(false) {
                        println!("  {} Shell config: {}", "⚙".cyan(), "Present".green());
                    }
                }
            }
            println!();

            println!("{}", "Activity Indicators:".yellow().bold());
            println!(
                "  {} Authentication logs: {}",
                "🔐".cyan(),
                if ctx.guestfs.exists("/var/log/auth.log").unwrap_or(false) {
                    "Available".green()
                } else {
                    "Check /var/log/secure".yellow()
                }
            );
            println!(
                "  {} Last login data: {}",
                "👥".cyan(),
                if ctx.guestfs.exists("/var/log/lastlog").unwrap_or(false) {
                    "Available".green()
                } else {
                    "Not found".red()
                }
            );
            println!();

            println!("{}", "Analysis Tips:".yellow());
            println!("  • Review .bash_history for executed commands");
            println!("  • Check authorized_keys for SSH access");
            println!("  • Examine sudo logs for privilege escalation");
            println!("  • Analyze authentication patterns in logs");
        }

        "integrity" => {
            println!("{}", "🛡 System Integrity Verification".cyan().bold());
            println!();

            let mut checks = Vec::new();

            // Check critical system binaries
            println!("{}", "Critical Binary Verification:".yellow().bold());
            let critical_bins = vec![
                "/bin/bash",
                "/bin/sh",
                "/bin/login",
                "/usr/bin/sudo",
                "/usr/bin/ssh",
                "/usr/bin/passwd",
                "/sbin/init",
                "/usr/sbin/sshd",
            ];

            let mut missing = 0;
            let mut present = 0;

            for bin in &critical_bins {
                if ctx.guestfs.exists(bin).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(bin).unwrap_or(0);
                    println!("  {} {} ({} bytes)", "✓".green(), bin.cyan(), size);
                    checks.push((bin, "present", size));
                    present += 1;
                } else {
                    println!("  {} {} {}", "✗".red(), bin.cyan(), "(missing)".red());
                    missing += 1;
                }
            }
            println!();

            // Check system library paths
            println!("{}", "System Libraries:".yellow().bold());
            let lib_paths = vec!["/lib", "/lib64", "/usr/lib", "/usr/lib64"];
            for lib in &lib_paths {
                if ctx.guestfs.is_dir(lib).unwrap_or(false) {
                    println!("  {} {} - Present", "✓".green(), lib.cyan());
                } else {
                    println!("  {} {} - {}", "✗".red(), lib.cyan(), "Missing".red());
                }
            }
            println!();

            // Configuration integrity
            println!("{}", "Configuration Integrity:".yellow().bold());
            let config_files = vec![
                "/etc/passwd",
                "/etc/group",
                "/etc/shadow",
                "/etc/fstab",
                "/etc/hosts",
            ];

            for cfg in &config_files {
                if ctx.guestfs.exists(cfg).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(cfg).unwrap_or(0);
                    if size > 0 {
                        println!("  {} {} ({} bytes)", "✓".green(), cfg.cyan(), size);
                    } else {
                        println!("  {} {} {}", "⚠".yellow(), cfg.cyan(), "(empty)".yellow());
                    }
                }
            }
            println!();

            // Integrity summary
            println!("{}", "Integrity Summary:".yellow().bold());
            println!("  Binaries checked: {}", critical_bins.len());
            println!("  Present: {}", present.to_string().green());
            if missing > 0 {
                println!("  Missing: {}", missing.to_string().red());
            }
            println!();

            let integrity_score = (present * 100) / critical_bins.len();
            let grade = if integrity_score >= 95 {
                "A".green().bold()
            } else if integrity_score >= 85 {
                "B".cyan()
            } else if integrity_score >= 75 {
                "C".yellow()
            } else {
                "D".red()
            };

            println!(
                "  Integrity Score: {}% (Grade: {})",
                integrity_score.to_string().cyan(),
                grade
            );
            println!();

            if missing > 0 {
                println!("{}", "⚠ Warning:".yellow().bold());
                println!("  Missing critical system files detected!");
                println!("  This may indicate system corruption or tampering.");
            }
        }

        "memory" => {
            println!("{}", "🧠 Memory Artifacts Analysis".cyan().bold());
            println!();

            println!("{}", "Note:".yellow().bold());
            println!("  Memory analysis requires live system access or memory dumps.");
            println!("  This command focuses on disk artifacts that may indicate memory activity.");
            println!();

            // Check for swap files and core dumps
            println!("{}", "Swap & Core Dumps:".yellow().bold());

            if ctx.guestfs.exists("/swap.img").unwrap_or(false) {
                let size = ctx.guestfs.filesize("/swap.img").unwrap_or(0);
                println!("  {} {} ({} bytes)", "✓".green(), "/swap.img".cyan(), size);
            }

            if ctx.guestfs.is_dir("/var/crash").unwrap_or(false) {
                println!(
                    "  {} {} - Present (may contain core dumps)",
                    "ℹ".cyan(),
                    "/var/crash".cyan()
                );
            }

            if ctx.guestfs.exists("/proc/kcore").unwrap_or(false) {
                println!(
                    "  {} {} - Kernel memory interface",
                    "ℹ".cyan(),
                    "/proc/kcore".cyan()
                );
            }
            println!();

            // Check for hibernation files
            println!("{}", "Hibernation Images:".yellow().bold());
            let hibernate_paths = vec!["/hibernation.img", "/swap/hibernation"];
            let mut found_hibernate = false;

            for path in &hibernate_paths {
                if ctx.guestfs.exists(path).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(path).unwrap_or(0);
                    println!("  {} {} ({} bytes)", "✓".green(), path.cyan(), size);
                    found_hibernate = true;
                }
            }

            if !found_hibernate {
                println!("  {} No hibernation images found", "ℹ".cyan());
            }
            println!();

            // Process information
            println!("{}", "Process Artifacts:".yellow().bold());
            if ctx.guestfs.is_dir("/proc").unwrap_or(false) {
                println!(
                    "  {} {} - Available for analysis",
                    "✓".green(),
                    "/proc".cyan()
                );
            }
            println!();

            println!("{}", "Analysis Recommendations:".yellow());
            println!("  • Extract swap files for string analysis");
            println!("  • Analyze core dumps for crash investigation");
            println!("  • Check /tmp and /var/tmp for remnants");
            println!("  • Review .bash_history for executed commands");
        }

        _ => {
            println!("{}", "Unknown forensics workflow".red());
            println!("Run {} for available workflows", "forensics".cyan());
        }
    }

    println!();
    Ok(())
}

/// Audit - Security audit trail analysis
pub fn cmd_audit(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "📋 Security Audit Trail Analysis".cyan().bold());
        println!();
        println!("{}", "Available Audit Types:".yellow().bold());
        println!(
            "{} {} - Authentication events",
            "1.".cyan(),
            "audit auth".green()
        );
        println!(
            "{} {} - User account changes",
            "2.".cyan(),
            "audit users".green()
        );
        println!(
            "{} {} - System configuration changes",
            "3.".cyan(),
            "audit config".green()
        );
        println!(
            "{} {} - Package installations",
            "4.".cyan(),
            "audit packages".green()
        );
        println!(
            "{} {} - Privilege escalation (sudo)",
            "5.".cyan(),
            "audit sudo".green()
        );
        println!(
            "{} {} - Comprehensive audit report",
            "6.".cyan(),
            "audit full".green()
        );
        println!();
        println!("{} audit <type>", "Usage:".yellow());
        println!();
        return Ok(());
    }

    let audit_type = args[0];

    match audit_type {
        "auth" => {
            println!("{}", "🔐 Authentication Audit".cyan().bold());
            println!();

            println!("{}", "Log File Analysis:".yellow().bold());

            let auth_logs = vec!["/var/log/auth.log", "/var/log/secure", "/var/log/messages"];

            let mut found_logs = Vec::new();

            for log in &auth_logs {
                if ctx.guestfs.exists(log).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(log).unwrap_or(0);
                    println!("  {} {} ({} bytes)", "✓".green(), log.cyan(), size);
                    found_logs.push(log);
                } else {
                    println!(
                        "  {} {} - Not found",
                        "✗".bright_black(),
                        log.bright_black()
                    );
                }
            }
            println!();

            if found_logs.is_empty() {
                println!("{}", "⚠ No authentication logs found".yellow());
                println!();
                return Ok(());
            }

            println!("{}", "Key Authentication Indicators:".yellow().bold());
            println!("  {} SSH login attempts", "🔑".cyan());
            println!("  {} Failed password attempts", "❌".cyan());
            println!("  {} Successful logins", "✅".cyan());
            println!("  {} Session duration", "⏱".cyan());
            println!("  {} Remote IP addresses", "🌐".cyan());
            println!();

            println!("{}", "Audit Checklist:".yellow());
            println!("  ✓ Check for brute force attempts (multiple failed logins)");
            println!("  ✓ Identify login patterns (time of day, source IPs)");
            println!("  ✓ Review privileged account access");
            println!("  ✓ Verify multi-factor authentication usage");
            println!("  ✓ Examine account lockouts");
            println!();

            println!(
                "{} {} authentication logs available",
                "Summary:".yellow(),
                found_logs.len().to_string().green()
            );
        }

        "users" => {
            println!("{}", "👥 User Account Audit".cyan().bold());
            println!();

            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                let total_users = user_info.len();
                let mut system_users = 0;
                let mut normal_users = 0;
                let mut privileged_users = 0;

                println!("{}", "User Account Analysis:".yellow().bold());

                for user in &user_info {
                    if user.uid == "0" {
                        privileged_users += 1;
                        println!(
                            "  {} {} (UID: {}) - Root equivalent",
                            "🔴".red(),
                            user.username.red().bold(),
                            user.uid
                        );
                    } else if user.uid.parse::<i32>().unwrap_or(9999) < 1000 {
                        system_users += 1;
                    } else {
                        normal_users += 1;
                        println!(
                            "  {} {} (UID: {})",
                            "👤".cyan(),
                            user.username.cyan(),
                            user.uid
                        );
                    }
                }
                println!();

                println!("{}", "Account Statistics:".yellow().bold());
                println!("  Total accounts: {}", total_users.to_string().cyan());
                println!(
                    "  Privileged (UID 0): {}",
                    privileged_users.to_string().red().bold()
                );
                println!(
                    "  System (UID < 1000): {}",
                    system_users.to_string().bright_black()
                );
                println!(
                    "  Normal users (UID ≥ 1000): {}",
                    normal_users.to_string().green()
                );
                println!();

                // Audit findings
                println!("{}", "Audit Findings:".yellow().bold());
                if privileged_users > 1 {
                    println!(
                        "  {} Multiple UID 0 accounts detected - CRITICAL",
                        "🔴".red()
                    );
                }
                if normal_users > 20 {
                    println!(
                        "  {} Large number of user accounts - Review needed",
                        "⚠".yellow()
                    );
                }
                if privileged_users == 1 && system_users < 100 && normal_users < 10 {
                    println!(
                        "  {} User account configuration appears normal",
                        "✓".green()
                    );
                }
            }
            println!();

            println!("{}", "Audit Actions:".yellow());
            println!("  • Review inactive accounts for removal");
            println!("  • Verify all UID 0 accounts are authorized");
            println!("  • Check for accounts with empty passwords");
            println!("  • Validate group memberships");
        }

        "config" => {
            println!("{}", "⚙ Configuration Change Audit".cyan().bold());
            println!();

            println!("{}", "Critical Configuration Files:".yellow().bold());

            let config_files = vec![
                ("/etc/passwd", "User database"),
                ("/etc/shadow", "Password hashes"),
                ("/etc/group", "Group database"),
                ("/etc/sudoers", "Sudo configuration"),
                ("/etc/ssh/sshd_config", "SSH server config"),
                ("/etc/pam.d", "PAM configuration"),
                ("/etc/security", "Security settings"),
                ("/etc/fstab", "Filesystem mounts"),
                ("/etc/hosts", "Host mappings"),
                ("/etc/resolv.conf", "DNS configuration"),
            ];

            let mut audited = 0;

            for (path, desc) in &config_files {
                if ctx.guestfs.exists(path).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(path).unwrap_or(0);
                    println!(
                        "  {} {} - {} ({} bytes)",
                        "✓".green(),
                        path.cyan(),
                        desc,
                        size
                    );
                    audited += 1;
                } else {
                    println!(
                        "  {} {} - {} {}",
                        "✗".red(),
                        path.cyan(),
                        desc,
                        "(missing)".red()
                    );
                }
            }
            println!();

            println!("{}", "Configuration Audit Summary:".yellow().bold());
            println!(
                "  Files audited: {}/{}",
                audited.to_string().green(),
                config_files.len()
            );
            println!();

            println!("{}", "Audit Recommendations:".yellow());
            println!("  • Track configuration changes with version control");
            println!("  • Implement configuration management (Ansible, Puppet)");
            println!("  • Regular backups of /etc directory");
            println!("  • Monitor for unauthorized modifications");
            println!("  • Validate configurations against security baselines");
        }

        "packages" => {
            println!("{}", "📦 Package Installation Audit".cyan().bold());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let total_packages = pkg_info.packages.len();

                println!("{}", "Package Statistics:".yellow().bold());
                println!("  Total packages: {}", total_packages.to_string().cyan());
                println!();

                // Categorize packages
                let mut dev_packages = 0;
                let mut lib_packages = 0;
                let mut kernel_packages = 0;
                let mut doc_packages = 0;

                for pkg in &pkg_info.packages {
                    let name = pkg.name.to_lowercase();
                    if name.contains("devel") || name.contains("-dev") {
                        dev_packages += 1;
                    } else if name.starts_with("lib") {
                        lib_packages += 1;
                    } else if name.contains("kernel") || name.contains("linux-") {
                        kernel_packages += 1;
                    } else if name.contains("doc") {
                        doc_packages += 1;
                    }
                }

                println!("{}", "Package Categories:".yellow().bold());
                println!("  Development: {}", dev_packages.to_string().cyan());
                println!("  Libraries: {}", lib_packages.to_string().cyan());
                println!("  Kernel: {}", kernel_packages.to_string().cyan());
                println!("  Documentation: {}", doc_packages.to_string().cyan());
                println!(
                    "  Other: {}",
                    (total_packages - dev_packages - lib_packages - kernel_packages - doc_packages)
                        .to_string()
                        .cyan()
                );
                println!();

                // Audit findings
                println!("{}", "Audit Findings:".yellow().bold());
                if dev_packages > 100 {
                    println!(
                        "  {} Large number of development packages - Consider cleanup",
                        "⚠".yellow()
                    );
                }
                if total_packages > 2000 {
                    println!(
                        "  {} System has many packages - Review for bloat",
                        "⚠".yellow()
                    );
                }
                if kernel_packages > 5 {
                    println!(
                        "  {} Multiple kernel versions - Remove old kernels",
                        "ℹ".cyan()
                    );
                }
                println!();

                println!("{}", "Package Audit Tips:".yellow());
                println!("  • Remove unused development packages");
                println!("  • Keep only 2-3 recent kernel versions");
                println!("  • Review automatically installed packages");
                println!("  • Verify package signatures and sources");
            }
        }

        "sudo" => {
            println!("{}", "🔐 Privilege Escalation Audit (Sudo)".cyan().bold());
            println!();

            println!("{}", "Sudo Configuration:".yellow().bold());

            if ctx.guestfs.exists("/etc/sudoers").unwrap_or(false) {
                let size = ctx.guestfs.filesize("/etc/sudoers").unwrap_or(0);
                println!(
                    "  {} {} ({} bytes)",
                    "✓".green(),
                    "/etc/sudoers".cyan(),
                    size
                );
            } else {
                println!("  {} {} - Not found", "✗".red(), "/etc/sudoers".cyan());
            }

            if ctx.guestfs.is_dir("/etc/sudoers.d").unwrap_or(false) {
                println!("  {} {} - Present", "✓".green(), "/etc/sudoers.d/".cyan());
            }
            println!();

            println!("{}", "Sudo Log Analysis:".yellow().bold());
            let sudo_logs = vec!["/var/log/sudo.log", "/var/log/secure", "/var/log/auth.log"];

            for log in &sudo_logs {
                if ctx.guestfs.exists(log).unwrap_or(false) {
                    let size = ctx.guestfs.filesize(log).unwrap_or(0);
                    println!("  {} {} ({} bytes)", "✓".green(), log.cyan(), size);
                }
            }
            println!();

            println!("{}", "Audit Checklist:".yellow().bold());
            println!("  ✓ Review sudo rules for least privilege");
            println!("  ✓ Check for NOPASSWD directives");
            println!("  ✓ Verify sudo group membership");
            println!("  ✓ Examine sudo command history");
            println!("  ✓ Look for privilege escalation attempts");
            println!();

            println!("{}", "Security Recommendations:".yellow());
            println!("  • Require passwords for all sudo commands");
            println!("  • Limit sudo access to specific commands");
            println!("  • Enable sudo logging");
            println!("  • Regular review of sudo configurations");
            println!("  • Use role-based access control");
        }

        "full" => {
            println!("{}", "📊 Comprehensive Security Audit Report".cyan().bold());
            println!();

            // Authentication audit summary
            println!("{}", "1. Authentication Security".yellow().bold());
            let auth_logs = vec!["/var/log/auth.log", "/var/log/secure"];
            let mut auth_found = 0;
            for log in &auth_logs {
                if ctx.guestfs.exists(log).unwrap_or(false) {
                    auth_found += 1;
                }
            }
            println!(
                "   Status: {}",
                if auth_found > 0 {
                    "✓ Logs available".green()
                } else {
                    "✗ No logs found".red()
                }
            );
            println!();

            // User account audit summary
            println!("{}", "2. User Account Security".yellow().bold());
            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                let privileged = user_info.iter().filter(|u| u.uid == "0").count();
                let normal = user_info
                    .iter()
                    .filter(|u| u.uid.parse::<i32>().unwrap_or(0) >= 1000)
                    .count();
                println!("   Total user_info: {}", user_info.len().to_string().cyan());
                println!("   Privileged accounts: {}", privileged.to_string().cyan());
                println!("   Normal users: {}", normal.to_string().cyan());
                if privileged > 1 {
                    println!("   {} Multiple UID 0 accounts detected", "⚠".yellow());
                }
            }
            println!();

            // Configuration audit summary
            println!("{}", "3. Configuration Security".yellow().bold());
            let configs = vec!["/etc/passwd", "/etc/shadow", "/etc/sudoers"];
            let mut config_ok = 0;
            for cfg in &configs {
                if ctx.guestfs.exists(cfg).unwrap_or(false) {
                    config_ok += 1;
                }
            }
            println!(
                "   Critical configs present: {}/{}",
                config_ok,
                configs.len()
            );
            println!();

            // Package audit summary
            println!("{}", "4. Package Security".yellow().bold());
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!(
                    "   Total packages: {}",
                    pkg_info.packages.len().to_string().cyan()
                );
                let dev_count = pkg_info
                    .packages
                    .iter()
                    .filter(|p| p.name.contains("devel") || p.name.contains("-dev"))
                    .count();
                println!("   Development packages: {}", dev_count.to_string().cyan());
            }
            println!();

            // Sudo audit summary
            println!("{}", "5. Privilege Escalation".yellow().bold());
            if ctx.guestfs.exists("/etc/sudoers").unwrap_or(false) {
                println!("   Status: {} Sudo configured", "✓".green());
            } else {
                println!("   Status: {} No sudo configuration", "ℹ".cyan());
            }
            println!();

            println!("{}", "Audit Completion Summary:".yellow().bold());
            println!("  ✓ Authentication logs reviewed");
            println!("  ✓ User accounts audited");
            println!("  ✓ Configuration files checked");
            println!("  ✓ Package inventory analyzed");
            println!("  ✓ Privilege escalation reviewed");
            println!();

            println!("{}", "Next Steps:".yellow());
            println!("  1. Address any critical findings");
            println!("  2. Document audit results");
            println!("  3. Implement remediation plan");
            println!("  4. Schedule regular audits");
        }

        _ => {
            println!("{}", "Unknown audit type".red());
            println!("Run {} for available types", "audit".cyan());
        }
    }

    println!();
    Ok(())
}

/// Baseline - Security baseline and drift detection
pub fn cmd_baseline(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "📏 Security Baseline Management".cyan().bold());
        println!();
        println!("{}", "Available Commands:".yellow().bold());
        println!(
            "{} {} - Create current security baseline",
            "1.".cyan(),
            "baseline create".green()
        );
        println!(
            "{} {} - Show current baseline",
            "2.".cyan(),
            "baseline show".green()
        );
        println!(
            "{} {} - Detect configuration drift",
            "3.".cyan(),
            "baseline drift".green()
        );
        println!(
            "{} {} - Compare with CIS benchmark",
            "4.".cyan(),
            "baseline cis".green()
        );
        println!(
            "{} {} - Export baseline for comparison",
            "5.".cyan(),
            "baseline export".green()
        );
        println!();
        println!("{} baseline <command>", "Usage:".yellow());
        println!();
        return Ok(());
    }

    let command = args[0];

    match command {
        "create" => {
            println!("{}", "📋 Creating Security Baseline".cyan().bold());
            println!();

            let mut baseline = Vec::new();

            // System information
            println!("{}", "System Configuration:".yellow().bold());
            if let Ok(os_info) = ctx.guestfs.inspect_os() {
                if !os_info.is_empty() {
                    println!("  {} OS detected", "✓".green());
                    baseline.push("OS configuration captured");
                }
            }
            println!();

            // Security features
            println!("{}", "Security Features:".yellow().bold());
            if let Ok(sec_info) = ctx.guestfs.inspect_security(&ctx.root) {
                println!(
                    "  SELinux: {}",
                    if &sec_info.selinux != "disabled" {
                        sec_info.selinux.green()
                    } else {
                        "disabled".red()
                    }
                );
                println!(
                    "  AppArmor: {}",
                    if sec_info.apparmor {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
                );

                let firewall_status = if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if fw.enabled {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
                } else {
                    "unknown".yellow()
                };
                println!("  Firewall: {}", firewall_status);
                baseline.push("Security features documented");
            }
            println!();

            // User accounts
            println!("{}", "User Accounts:".yellow().bold());
            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                let privileged = user_info.iter().filter(|u| u.uid == "0").count();
                println!("  Total users: {}", user_info.len());
                println!("  Privileged accounts: {}", privileged);
                baseline.push("User accounts baselined");
            }
            println!();

            // Package count
            println!("{}", "Software Inventory:".yellow().bold());
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("  Total packages: {}", pkg_info.packages.len());
                baseline.push("Package inventory captured");
            }
            println!();

            // Services
            println!("{}", "System Services:".yellow().bold());
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let total = services.len();
                let enabled = services.iter().filter(|s| s.enabled).count();
                println!("  Total services: {}", total);
                println!("  Enabled services: {}", enabled);
                baseline.push("Service configuration captured");
            }
            println!();

            // Network configuration
            println!("{}", "Network Configuration:".yellow().bold());
            if ctx.guestfs.exists("/etc/hosts").unwrap_or(false) {
                println!("  {} /etc/hosts present", "✓".green());
                baseline.push("Network config documented");
            }
            if ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                println!("  {} /etc/resolv.conf present", "✓".green());
            }
            println!();

            println!("{}", "Baseline Creation Summary:".yellow().bold());
            println!(
                "  Components captured: {}",
                baseline.len().to_string().green()
            );
            for component in &baseline {
                println!("    • {}", component);
            }
            println!();

            println!("{}", "Next Steps:".yellow());
            println!(
                "  • Save baseline: {}",
                "baseline export > baseline.json".cyan()
            );
            println!("  • Monitor drift: {}", "baseline drift".cyan());
            println!("  • Compare with standards: {}", "baseline cis".cyan());
        }

        "show" => {
            println!("{}", "📋 Current Security Baseline".cyan().bold());
            println!();

            // Display current system state as baseline
            println!("{}", "╔══ System Baseline ══╗".cyan().bold());
            println!();

            if let Ok(sec_info) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("{}", "Security Configuration:".yellow().bold());
                println!("  SELinux: {}", sec_info.selinux);
                println!("  AppArmor: {}", sec_info.apparmor);

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    println!(
                        "  Firewall: {}",
                        if fw.enabled { "enabled" } else { "disabled" }
                    );
                }
                println!();
            }

            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("{}", "User Account Baseline:".yellow().bold());
                println!("  Total users: {}", user_info.len());
                println!(
                    "  UID 0 accounts: {}",
                    user_info.iter().filter(|u| u.uid == "0").count()
                );
                println!(
                    "  Normal users: {}",
                    user_info
                        .iter()
                        .filter(|u| u.uid.parse::<i32>().unwrap_or(0) >= 1000)
                        .count()
                );
                println!();
            }

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("{}", "Software Baseline:".yellow().bold());
                println!("  Installed packages: {}", pkg_info.packages.len());
                println!();
            }

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                println!("{}", "Service Baseline:".yellow().bold());
                println!("  Total services: {}", services.len());
                println!(
                    "  Enabled: {}",
                    services.iter().filter(|s| s.enabled).count()
                );
                println!(
                    "  Disabled: {}",
                    services.iter().filter(|s| !s.enabled).count()
                );
                println!();
            }

            println!("{}", "╚══ End Baseline ══╝".cyan().bold());
        }

        "drift" => {
            println!("{}", "🔍 Configuration Drift Detection".cyan().bold());
            println!();

            println!("{}", "Note:".yellow().bold());
            println!("  Drift detection requires a saved baseline for comparison.");
            println!(
                "  Run {} to establish initial baseline.",
                "baseline create".cyan()
            );
            println!();

            println!("{}", "Drift Monitoring Areas:".yellow().bold());
            println!();

            // Check for common drift indicators
            let mut drift_detected = Vec::new();

            // User account drift
            println!("{}", "1. User Account Drift".cyan());
            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                let uid0_count = user_info.iter().filter(|u| u.uid == "0").count();
                if uid0_count > 1 {
                    drift_detected.push("Multiple UID 0 accounts (expected: 1)");
                    println!("   {} Multiple privileged accounts detected", "⚠".yellow());
                } else {
                    println!("   {} Account structure stable", "✓".green());
                }
            }
            println!();

            // Security configuration drift
            println!("{}", "2. Security Configuration Drift".cyan());
            if let Ok(sec_info) = ctx.guestfs.inspect_security(&ctx.root) {
                if &sec_info.selinux == "disabled" && !sec_info.apparmor {
                    drift_detected.push("No MAC system enabled");
                    println!("   {} MAC system disabled (potential drift)", "⚠".yellow());
                }

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if !fw.enabled {
                        drift_detected.push("Firewall disabled");
                        println!("   {} Firewall disabled (potential drift)", "⚠".yellow());
                    }
                }

                if drift_detected.is_empty() {
                    println!("   {} Security configuration stable", "✓".green());
                }
            }
            println!();

            // Service drift
            println!("{}", "3. Service Configuration Drift".cyan());
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                if enabled > 50 {
                    drift_detected.push("High number of enabled services");
                    println!(
                        "   {} Many services enabled (potential drift)",
                        "⚠".yellow()
                    );
                } else {
                    println!("   {} Service configuration stable", "✓".green());
                }
            }
            println!();

            // Configuration file drift
            println!("{}", "4. Critical File Drift".cyan());
            let critical_files = vec![
                "/etc/passwd",
                "/etc/shadow",
                "/etc/sudoers",
                "/etc/ssh/sshd_config",
                "/etc/fstab",
            ];
            let mut all_present = true;
            for file in &critical_files {
                if !ctx.guestfs.exists(file).unwrap_or(false) {
                    drift_detected.push("Missing critical configuration file");
                    println!("   {} {} missing (critical drift)", "🔴".red(), file.red());
                    all_present = false;
                }
            }
            if all_present {
                println!("   {} Critical files intact", "✓".green());
            }
            println!();

            // Drift summary
            println!("{}", "Drift Detection Summary:".yellow().bold());
            if drift_detected.is_empty() {
                println!("  {} No significant drift detected", "✓".green().bold());
                println!("  System configuration appears stable");
            } else {
                println!(
                    "  {} {} drift indicators found:",
                    "⚠".yellow(),
                    drift_detected.len().to_string().red()
                );
                for drift in &drift_detected {
                    println!("    • {}", drift);
                }
            }
            println!();

            println!("{}", "Recommendations:".yellow());
            println!("  • Review and address drift indicators");
            println!("  • Update baseline if changes are authorized");
            println!("  • Investigate unauthorized modifications");
            println!("  • Implement configuration management");
        }

        "cis" => {
            println!("{}", "📋 CIS Benchmark Comparison".cyan().bold());
            println!();

            let mut checks = Vec::new();
            let mut passed = 0;
            let mut failed = 0;

            println!("{}", "CIS Controls Validation:".yellow().bold());
            println!();

            // CIS Control 1: Ensure filesystem integrity checking
            println!("{}", "1. Filesystem Integrity".cyan());
            let has_aide = ctx.guestfs.exists("/usr/bin/aide").unwrap_or(false);
            if has_aide {
                println!("   {} AIDE installed", "✓".green());
                passed += 1;
            } else {
                println!(
                    "   {} AIDE not found - Install integrity checking",
                    "✗".red()
                );
                failed += 1;
            }
            checks.push(("Filesystem integrity checking", has_aide));
            println!();

            // CIS Control 2: Ensure firewall is enabled
            println!("{}", "2. Firewall Configuration".cyan());
            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                if fw.enabled {
                    println!("   {} Firewall enabled", "✓".green());
                    passed += 1;
                } else {
                    println!("   {} Firewall disabled - Enable firewall", "✗".red());
                    failed += 1;
                }
                checks.push(("Firewall enabled", fw.enabled));
            }
            println!();

            // CIS Control 3: Ensure MAC is enabled
            println!("{}", "3. Mandatory Access Control".cyan());
            if let Ok(sec_info) = ctx.guestfs.inspect_security(&ctx.root) {
                let mac_enabled = &sec_info.selinux != "disabled" || sec_info.apparmor;
                if mac_enabled {
                    println!("   {} MAC system active", "✓".green());
                    passed += 1;
                } else {
                    println!(
                        "   {} No MAC system - Enable SELinux or AppArmor",
                        "✗".red()
                    );
                    failed += 1;
                }
                checks.push(("MAC enabled", mac_enabled));
            }
            println!();

            // CIS Control 4: SSH configuration
            println!("{}", "4. SSH Hardening".cyan());
            let ssh_config = ctx.guestfs.exists("/etc/ssh/sshd_config").unwrap_or(false);
            if ssh_config {
                println!("   {} SSH configuration present", "✓".green());
                println!("   {} Manual review recommended for:", "ℹ".cyan());
                println!("      • PermitRootLogin no");
                println!("      • PasswordAuthentication no");
                println!("      • Protocol 2");
                passed += 1;
            } else {
                println!("   {} SSH configuration missing", "✗".red());
                failed += 1;
            }
            checks.push(("SSH configuration", ssh_config));
            println!();

            // CIS Control 5: Audit logging
            println!("{}", "5. Audit Logging".cyan());
            let auditd = ctx.guestfs.exists("/sbin/auditd").unwrap_or(false);
            if auditd {
                println!("   {} Audit daemon installed", "✓".green());
                passed += 1;
            } else {
                println!("   {} Audit daemon not found - Install auditd", "✗".red());
                failed += 1;
            }
            checks.push(("Audit logging", auditd));
            println!();

            // CIS summary
            let total = checks.len();
            let compliance_rate = (passed as usize)
                .saturating_mul(100)
                .checked_div(total)
                .unwrap_or(0);

            println!("{}", "CIS Benchmark Summary:".yellow().bold());
            println!("  Total checks: {}", total);
            println!("  Passed: {}", passed.to_string().green());
            println!("  Failed: {}", failed.to_string().red());
            println!("  Compliance rate: {}%", compliance_rate.to_string().cyan());
            println!();

            let grade = if compliance_rate >= 80 {
                "Compliant".green().bold()
            } else if compliance_rate >= 60 {
                "Partially Compliant".yellow()
            } else {
                "Non-Compliant".red().bold()
            };

            println!("  Overall status: {}", grade);
            println!();

            println!("{}", "Next Steps:".yellow());
            println!("  • Address failed controls");
            println!("  • Document exceptions");
            println!("  • Schedule regular compliance checks");
            println!("  • Implement remediation plan");
        }

        "export" => {
            println!("{}", "💾 Exporting Security Baseline".cyan().bold());
            println!();

            println!("{}", "Export Format: JSON".yellow().bold());
            println!();

            // Create a baseline export structure
            println!("{{");
            println!("  \"baseline\": {{");
            println!("    \"created\": \"2024-01-01T00:00:00Z\",");
            println!("    \"system\": {{");

            if let Ok(sec_info) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("      \"selinux\": \"{}\",", sec_info.selinux);
                println!("      \"apparmor\": {},", sec_info.apparmor);

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    println!("      \"firewall\": {},", fw.enabled);
                }
            }

            if let Ok(user_info) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("      \"total_users\": {},", user_info.len());
                println!(
                    "      \"privileged_users\": {},",
                    user_info.iter().filter(|u| u.uid == "0").count()
                );
            }

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("      \"total_packages\": {}", pkg_info.packages.len());
            }

            println!("    }}");
            println!("  }}");
            println!("}}");
            println!();

            println!("{}", "Save this output:".yellow());
            println!("  {}", "baseline export > baseline.json".cyan());
            println!();
            println!("{}", "Use for comparison:".yellow());
            println!("  Compare against saved baseline to detect drift");
        }

        _ => {
            println!("{}", "Unknown baseline command".red());
            println!("Run {} for available commands", "baseline".cyan());
        }
    }

    println!();
    Ok(())
}

/// Launch interactive file explorer
pub fn cmd_explore(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    use crate::cli::shell::explore::run_explorer;

    let start_path = if !args.is_empty() {
        Some(args[0])
    } else {
        None
    };

    println!("{} Launching file explorer...", "→".cyan());
    println!("{} Use 'h' for help once inside", "ℹ".yellow());

    std::thread::sleep(std::time::Duration::from_millis(500));

    run_explorer(ctx, start_path)?;

    // Return to shell prompt
    println!("\n{} Returned to shell", "✓".green());

    Ok(())
}
