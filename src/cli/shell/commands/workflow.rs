// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Workflow and automation commands for interactive shell

use super::*;
use anyhow::Result;
use colored::Colorize;

pub fn cmd_export(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "Usage: export <type> <format> [output_file]".yellow());
        println!();
        println!("{}", "Types:".green().bold());
        println!("  {} - Export package list", "packages".cyan());
        println!("  {} - Export user accounts", "users".cyan());
        println!("  {} - Export services", "services".cyan());
        println!("  {} - Export system info", "system".cyan());
        println!();
        println!("{}", "Formats:".green().bold());
        println!("  {} - JSON format", "json".cyan());
        println!("  {} - CSV format", "csv".cyan());
        println!("  {} - Markdown table", "md".cyan());
        println!("  {} - Plain text", "txt".cyan());
        println!();
        println!("{}", "Examples:".yellow());
        println!("  export packages json packages.json");
        println!("  export users csv users.csv");
        println!("  export system md system.md");
        return Ok(());
    }

    let export_type = args[0];
    let format = if args.len() > 1 { args[1] } else { "json" };
    let output = if args.len() > 2 { Some(args[2]) } else { None };

    println!(
        "{} Exporting {} as {}...",
        "→".cyan(),
        export_type.yellow(),
        format.green()
    );

    match export_type {
        "packages" => export_packages(ctx, format, output)?,
        "users" => export_users(ctx, format, output)?,
        "services" => export_services(ctx, format, output)?,
        "system" => export_system(ctx, format, output)?,
        _ => {
            println!("{} Unknown export type: {}", "Error:".red(), export_type);
            return Ok(());
        }
    }

    println!("{} Export completed!", "✓".green());
    Ok(())
}

/// Show directory tree
pub fn cmd_tips(_ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    use rand::Rng;

    let tips = vec![
        (
            "⚡",
            "Use aliases to speed up common commands",
            "Try: alias ll 'ls -l'",
        ),
        (
            "🔖",
            "Bookmark frequently visited directories",
            "Try: bookmark config /etc",
        ),
        ("⏱", "Commands >100ms show execution time automatically", ""),
        (
            "🔍",
            "Use grep with patterns",
            "Try: grep 'error' /var/log/syslog",
        ),
        ("📊", "View system overview", "Try: dashboard"),
        (
            "💾",
            "Export data for analysis",
            "Try: export packages json",
        ),
        ("🌳", "Visualize directory structure", "Try: tree /etc 2"),
        ("↑↓", "Navigate command history with arrow keys", ""),
        ("Tab", "Use Tab for command completion", ""),
        ("📈", "Check shell statistics", "Try: stats"),
    ];

    let mut rng = rand::thread_rng();
    let tip = &tips[rng.gen_range(0..tips.len())];

    println!("\n{} {}", "💡 Tip:".yellow().bold(), tip.1.green());
    if !tip.2.is_empty() {
        println!("   {}", tip.2.cyan());
    }
    println!();

    Ok(())
}

/// Generate comprehensive system snapshot report
pub fn cmd_quick(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║            Quick Actions Menu                   ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Snapshots & Reports:".yellow().bold());
        println!("  {} - Full system snapshot", "quick snapshot".cyan());
        println!("  {} - Security report", "quick security".cyan());
        println!("  {} - Package inventory", "quick packages".cyan());

        println!("\n{}", "Common Tasks:".yellow().bold());
        println!("  {} - List all users", "quick users".cyan());
        println!("  {} - Show enabled services", "quick services".cyan());
        println!("  {} - Network overview", "quick network".cyan());

        println!("\n{}", "Analysis:".yellow().bold());
        println!("  {} - Show recent files", "quick recent".cyan());
        println!("  {} - Find large files", "quick large".cyan());
        println!("  {} - System health", "quick health".cyan());

        println!();
        return Ok(());
    }

    let action = args[0];

    match action {
        "snapshot" => {
            println!("{} Generating quick snapshot...", "→".cyan());
            cmd_snapshot(ctx, &[])?;
        }
        "security" => {
            println!("{} Security overview:", "→".cyan());
            println!();
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let selinux_icon = if &sec.selinux != "disabled" {
                    "✓".green()
                } else {
                    "✗".red()
                };
                let apparmor_icon = if sec.apparmor {
                    "✓".green()
                } else {
                    "✗".red()
                };
                let auditd_icon = if sec.auditd {
                    "✓".green()
                } else {
                    "✗".red()
                };

                println!("  {} SELinux:  {}", selinux_icon, sec.selinux.yellow());
                println!(
                    "  {} AppArmor: {}",
                    apparmor_icon,
                    if sec.apparmor { "enabled" } else { "disabled" }
                );
                println!(
                    "  {} Auditd:   {}",
                    auditd_icon,
                    if sec.auditd { "enabled" } else { "disabled" }
                );

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    let fw_icon = if fw.enabled {
                        "✓".green()
                    } else {
                        "✗".red()
                    };
                    println!(
                        "  {} Firewall: {} ({})",
                        fw_icon,
                        if fw.enabled { "enabled" } else { "disabled" },
                        fw.firewall_type
                    );
                }
            }
            println!();
        }
        "packages" => {
            println!("{} Package summary:", "→".cyan());
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!(
                    "  Total packages: {}",
                    pkg_info.packages.len().to_string().green().bold()
                );
                println!("  Use {} for details", "'export packages json'".cyan());
            }
            println!();
        }
        "users" => {
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("{} User accounts:", "→".cyan());
                for user in users {
                    let marker = if user.uid == "0" { " ⚠️ " } else { "   " };
                    println!(
                        "{}  {} ({})",
                        marker,
                        user.username.green(),
                        user.uid.bright_black()
                    );
                }
                println!();
            }
        }
        "services" => {
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled: Vec<_> = services.iter().filter(|s| s.enabled).collect();
                println!("{} Enabled services ({}):", "→".cyan(), enabled.len());
                for svc in enabled.iter().take(20) {
                    println!("  {} {}", "✓".green(), svc.name.cyan());
                }
                if enabled.len() > 20 {
                    println!("  ... and {} more", enabled.len() - 20);
                }
                println!();
            }
        }
        "network" => {
            if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
                println!("{} Network interfaces:", "→".cyan());
                for iface in interfaces {
                    println!("  {} {}", "•".cyan(), iface.name.green());
                }

                if let Ok(dns) = ctx.guestfs.inspect_dns(&ctx.root) {
                    if !dns.is_empty() {
                        println!("\n  DNS servers:");
                        for server in dns {
                            println!("    {} {}", "•".cyan(), server.yellow());
                        }
                    }
                }
                println!();
            }
        }
        "recent" => {
            cmd_recent(ctx, &["/etc", "20"])?;
        }
        "large" => {
            println!("{} Finding large files...", "→".cyan());
            println!("{} Use: find . -type f -size +10M", "Hint:".yellow());
            println!();
        }
        "health" => {
            println!("{} Quick health check:", "→".cyan());
            cmd_summary(ctx, &[])?;
        }
        _ => {
            println!("{} Unknown quick action: {}", "Error:".red(), action);
            println!("{} Use 'quick' to see available actions", "Tip:".yellow());
        }
    }

    Ok(())
}

/// Show command cheat sheet
pub fn cmd_cheat(ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                  Command Cheat Sheet                     ║"
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

    let cheat = vec![
        (
            "📂",
            "File Operations",
            vec![
                ("ls /etc", "List directory contents"),
                ("cat /etc/fstab", "View file contents"),
                ("tree /etc 2", "Directory tree (2 levels)"),
                ("find . passwd", "Find files by name"),
            ],
        ),
        (
            "📊",
            "System Overview",
            vec![
                ("dashboard", "Beautiful system overview"),
                ("summary", "Quick one-liner status"),
                ("info", "Detailed system info"),
            ],
        ),
        (
            "💾",
            "Data Export",
            vec![
                ("export packages json", "Export to JSON"),
                ("snapshot report.md", "Full system snapshot"),
                ("diff package kernel", "Compare packages"),
            ],
        ),
        (
            "👥",
            "User & Security",
            vec![
                ("users", "List all users"),
                ("security", "Security status"),
                ("services", "System services"),
            ],
        ),
        (
            "⚡",
            "Quick Actions",
            vec![
                ("quick", "Show quick actions menu"),
                ("quick security", "Security overview"),
                ("recent /var/log", "Recent files"),
            ],
        ),
        (
            "🎯",
            "Productivity",
            vec![
                ("alias ll 'ls -l'", "Create alias"),
                ("bookmark cfg /etc", "Bookmark path"),
                ("goto cfg", "Jump to bookmark"),
                ("tips", "Random tip"),
            ],
        ),
    ];

    for (icon, category, commands) in cheat {
        println!("{} {}", icon, category.yellow().bold());
        for (cmd, desc) in commands {
            println!("  {} - {}", cmd.green(), desc.bright_black());
        }
        println!();
    }

    println!("📍 Current path: {}", ctx.current_path.cyan());
    println!(
        "{} Type 'help' for complete command list",
        "💡".to_string().yellow()
    );
    println!();

    Ok(())
}

/// Smart search with filters
pub fn cmd_search(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "Usage: search <pattern> [options]".yellow());
        println!();
        println!("{}", "Options:".green().bold());
        println!("  {} - Search in specific path", "--path <path>".cyan());
        println!(
            "  {} - Filter by file type (file/dir)",
            "--type <type>".cyan()
        );
        println!(
            "  {} - Size filter (e.g., +1M, -100K)",
            "--size <size>".cyan()
        );
        println!("  {} - Name pattern only (default)", "--name".cyan());
        println!("  {} - Search file contents", "--content".cyan());
        println!();
        println!("{}", "Examples:".yellow());
        println!("  search passwd --path /etc");
        println!("  search *.conf --type file");
        println!("  search error --content --path /var/log");
        return Ok(());
    }

    let pattern = args[0];
    let mut search_path = ctx.current_path.clone();
    let mut search_content = false;
    let mut type_filter = None;

    // Parse options
    let mut i = 1;
    while i < args.len() {
        match args[i] {
            "--path" if i + 1 < args.len() => {
                search_path = args[i + 1].to_string();
                i += 2;
            }
            "--content" => {
                search_content = true;
                i += 1;
            }
            "--type" if i + 1 < args.len() => {
                type_filter = Some(args[i + 1]);
                i += 2;
            }
            _ => i += 1,
        }
    }

    println!(
        "{} Searching for: {} in {}",
        "→".cyan(),
        pattern.yellow(),
        search_path.cyan()
    );
    println!();

    let mut results = Vec::new();

    // Simple recursive search (simplified version)
    if let Ok(entries) = ctx.guestfs.ls(&search_path) {
        for entry in entries {
            let full_path = format!("{}/{}", search_path.trim_end_matches('/'), entry);

            // Name matching
            if entry.to_lowercase().contains(&pattern.to_lowercase()) {
                if let Some(filter) = type_filter {
                    let is_dir = ctx.guestfs.is_dir(&full_path).unwrap_or(false);
                    if (filter == "dir" && !is_dir) || (filter == "file" && is_dir) {
                        continue;
                    }
                }
                results.push((full_path.clone(), entry.clone(), "name".to_string()));
            }

            // Content search for files
            if search_content && !ctx.guestfs.is_dir(&full_path).unwrap_or(true) {
                if let Ok(content) = ctx.guestfs.read_file(&full_path) {
                    if let Ok(text) = std::str::from_utf8(&content) {
                        if text.contains(pattern) {
                            let lines: Vec<&str> = text
                                .lines()
                                .filter(|l| l.contains(pattern))
                                .take(3)
                                .collect();
                            for line in lines {
                                results.push((
                                    full_path.clone(),
                                    line.to_string(),
                                    "content".to_string(),
                                ));
                            }
                        }
                    }
                }
            }
        }
    }

    if results.is_empty() {
        println!("{} No results found", "⚠".yellow());
    } else {
        println!(
            "{} ({} results)",
            "Search Results:".yellow().bold(),
            results.len()
        );
        println!("{}", "─".repeat(80).cyan());

        for (path, content, match_type) in results.iter().take(50) {
            if match_type == "name" {
                println!("  📄 {}", path.green());
            } else {
                println!(
                    "  {} {} {}",
                    "→".cyan(),
                    path.bright_black(),
                    content.yellow()
                );
            }
        }

        if results.len() > 50 {
            println!(
                "\n{} Showing 50 of {} results",
                "Note:".yellow(),
                results.len()
            );
        }
    }
    println!();

    Ok(())
}

/// Watch files/directories for changes (simulation)
pub fn cmd_watch(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "Usage: watch <path> [interval]".yellow());
        println!();
        println!("{}", "Examples:".green().bold());
        println!("  watch /var/log 5     - Watch /var/log every 5 seconds");
        println!("  watch /etc/fstab     - Watch single file");
        println!();
        println!(
            "{} This is a snapshot-based watch (not live)",
            "Note:".yellow()
        );
        return Ok(());
    }

    let watch_path = args[0];
    let full_path = resolve_path(&ctx.current_path, watch_path);

    println!("{} Watching: {}", "→".cyan(), full_path.yellow());
    println!("{} Taking initial snapshot...", "→".cyan());
    println!();

    // Take initial snapshot
    let initial_stat = ctx.guestfs.stat(&full_path)?;
    let initial_size = initial_stat.size;
    let initial_mtime = initial_stat.mtime;

    println!("{}", "Initial State:".yellow().bold());
    println!("  Size: {} bytes", initial_size.to_string().green());
    println!("  Modified: {}", initial_mtime.to_string().bright_black());

    if ctx.guestfs.is_dir(&full_path).unwrap_or(false) {
        if let Ok(entries) = ctx.guestfs.ls(&full_path) {
            println!("  Files: {}", entries.len().to_string().green());
        }
    }

    println!();
    println!(
        "{} Use Ctrl+C to stop watching (in a real implementation)",
        "Tip:".yellow()
    );
    println!(
        "{} VM filesystems are static snapshots, so changes won't be detected in real-time",
        "Note:".bright_black()
    );

    Ok(())
}

/// Batch operations on files
pub fn cmd_batch(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║              Batch Operations                   ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Available Operations:".yellow().bold());
        println!("  {} - List multiple files", "batch cat <pattern>".cyan());
        println!(
            "  {} - Find in multiple locations",
            "batch find <pattern> <paths...>".cyan()
        );
        println!("  {} - Export multiple types", "batch export <dir>".cyan());
        println!(
            "  {} - Analyze multiple logs",
            "batch analyze <paths...>".cyan()
        );
        println!();

        println!("{}", "Examples:".green().bold());
        println!("  batch cat /etc/*.conf");
        println!("  batch find passwd /etc /home");
        println!("  batch export /tmp/reports");
        println!();
        return Ok(());
    }

    let operation = args[0];

    match operation {
        "cat" => {
            if args.len() < 2 {
                println!("{} Usage: batch cat <file1> [file2...]", "Error:".red());
                return Ok(());
            }

            println!("{} Reading multiple files...", "→".cyan());
            println!();

            for file in &args[1..] {
                let full_path = resolve_path(&ctx.current_path, file);
                println!("{}", format!("=== {} ===", full_path).yellow().bold());

                match ctx.guestfs.read_file(&full_path) {
                    Ok(content) => {
                        if let Ok(text) = std::str::from_utf8(&content) {
                            let lines: Vec<&str> = text.lines().take(20).collect();
                            for line in lines {
                                println!("{}", line);
                            }
                            if text.lines().count() > 20 {
                                println!("{}", "... (truncated)".bright_black());
                            }
                        }
                    }
                    Err(e) => {
                        println!("{} Failed to read: {}", "Error:".red(), e);
                    }
                }
                println!();
            }
        }
        "export" => {
            let output_dir = if args.len() > 1 { args[1] } else { "." };

            println!(
                "{} Exporting all data types to: {}",
                "→".cyan(),
                output_dir.yellow()
            );

            let exports = vec![
                ("packages", "json"),
                ("users", "json"),
                ("services", "json"),
                ("system", "md"),
            ];

            for (export_type, format) in exports {
                let filename = format!("{}/{}.{}", output_dir, export_type, format);
                println!(
                    "  {} Exporting {} to {}",
                    "→".cyan(),
                    export_type.green(),
                    filename.bright_black()
                );

                match export_type {
                    "packages" => {
                        let _ = export_packages(ctx, format, Some(&filename));
                    }
                    "users" => {
                        let _ = export_users(ctx, format, Some(&filename));
                    }
                    "services" => {
                        let _ = export_services(ctx, format, Some(&filename));
                    }
                    "system" => {
                        let _ = export_system(ctx, format, Some(&filename));
                    }
                    _ => {}
                }
            }

            println!();
            println!("{} Batch export complete!", "✓".green());
        }
        "find" => {
            if args.len() < 3 {
                println!(
                    "{} Usage: batch find <pattern> <path1> [path2...]",
                    "Error:".red()
                );
                return Ok(());
            }

            let pattern = args[1];
            let paths = &args[2..];

            println!(
                "{} Searching for '{}' in {} locations",
                "→".cyan(),
                pattern.yellow(),
                paths.len()
            );
            println!();

            for path in paths {
                println!("{} Searching in: {}", "→".cyan(), path.yellow());
                if let Ok(entries) = ctx.guestfs.ls(path) {
                    let matches: Vec<_> = entries
                        .iter()
                        .filter(|e| e.to_lowercase().contains(&pattern.to_lowercase()))
                        .collect();

                    if !matches.is_empty() {
                        for entry in matches {
                            let full_path = format!("{}/{}", path.trim_end_matches('/'), entry);
                            println!("  {} {}", "•".cyan(), full_path.green());
                        }
                    }
                }
                println!();
            }
        }
        _ => {
            println!("{} Unknown batch operation: {}", "Error:".red(), operation);
            println!(
                "{} Use 'batch' to see available operations",
                "Tip:".yellow()
            );
        }
    }

    Ok(())
}

/// Favorites/pinned commands
pub fn cmd_pin(_ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    // For simplicity, we'll store pins in a static location
    // In a real implementation, this would be persistent

    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║              Pinned Commands                    ║"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "╚═══════════════════════════════════════════════════╝"
                .cyan()
                .bold()
        );
        println!();

        println!("{}", "Usage:".yellow().bold());
        println!("  {} - Show all pins", "pin".cyan());
        println!("  {} - Pin a command", "pin <name> <command>".cyan());
        println!("  {} - Run a pinned command", "pin run <name>".cyan());
        println!("  {} - Remove a pin", "pin remove <name>".cyan());
        println!();

        println!("{}", "Examples:".green().bold());
        println!("  pin logs 'cat /var/log/syslog'");
        println!("  pin security 'quick security'");
        println!("  pin run logs");
        println!();

        println!("{}", "Suggested Pins:".yellow().bold());
        println!("  📌 pin errors 'grep ERROR /var/log'");
        println!("  📌 pin users 'quick users'");
        println!("  📌 pin snap 'snapshot'");
        println!();

        return Ok(());
    }

    let action = args[0];

    match action {
        "run" => {
            if args.len() < 2 {
                println!("{} Usage: pin run <name>", "Error:".red());
                return Ok(());
            }
            let pin_name = args[1];
            println!(
                "{} Would execute pinned command: {}",
                "→".cyan(),
                pin_name.yellow()
            );
            println!(
                "{} Pin functionality requires persistent storage",
                "Note:".bright_black()
            );
        }
        "remove" => {
            if args.len() < 2 {
                println!("{} Usage: pin remove <name>", "Error:".red());
                return Ok(());
            }
            let pin_name = args[1];
            println!("{} Would remove pin: {}", "→".cyan(), pin_name.yellow());
        }
        _ => {
            // Assume it's "pin <name> <command>"
            if args.len() < 2 {
                println!("{} Usage: pin <name> <command>", "Error:".red());
                return Ok(());
            }
            let pin_name = args[0];
            let command = args[1..].join(" ");
            println!(
                "{} Would pin command: {} = {}",
                "→".cyan(),
                pin_name.yellow(),
                command.green()
            );
            println!(
                "{} Pin functionality requires persistent storage",
                "Note:".bright_black()
            );
        }
    }

    Ok(())
}

/// Show command history with analysis
pub fn cmd_wizard(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Interactive Wizards                     ║"
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

        println!("{}", "Available Wizards:".yellow().bold());
        println!("  {} - Security audit wizard", "wizard security".cyan());
        println!("  {} - System health check wizard", "wizard health".cyan());
        println!("  {} - Package analysis wizard", "wizard packages".cyan());
        println!("  {} - Configuration review wizard", "wizard config".cyan());
        println!("  {} - Export/report wizard", "wizard export".cyan());
        println!();

        println!("{}", "What are wizards?".green().bold());
        println!("  Interactive step-by-step guides for complex tasks");
        println!("  Automated checks with detailed explanations");
        println!("  Perfect for learning and thorough analysis");
        println!();

        return Ok(());
    }

    let wizard_type = args[0];

    match wizard_type {
        "security" => {
            println!("\n{}", "🔒 Security Audit Wizard".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            println!("{} Step 1/5: Checking security features...", "→".cyan());
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let mut score = 0;
                let mut issues = Vec::new();

                if &sec.selinux != "disabled" {
                    println!(
                        "  {} SELinux: {} (enforcing)",
                        "✓".green(),
                        sec.selinux.green()
                    );
                    score += 20;
                } else {
                    println!("  {} SELinux: disabled", "✗".red());
                    issues.push("Enable SELinux for mandatory access control");
                }

                if sec.apparmor {
                    println!("  {} AppArmor: enabled", "✓".green());
                    score += 20;
                } else {
                    println!("  {} AppArmor: disabled", "✗".red());
                    issues.push("Enable AppArmor for application confinement");
                }

                if sec.auditd {
                    println!("  {} Auditd: enabled", "✓".green());
                    score += 15;
                } else {
                    println!("  {} Auditd: disabled", "✗".yellow());
                    issues.push("Enable auditd for system auditing");
                }

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if fw.enabled {
                        println!("  {} Firewall: enabled ({})", "✓".green(), fw.firewall_type);
                        score += 25;
                    } else {
                        println!("  {} Firewall: disabled", "✗".red());
                        issues.push("Enable firewall for network protection");
                    }
                }

                println!();
                println!("{} Step 2/5: Checking user accounts...", "→".cyan());
                if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                    let root_users: Vec<_> = users.iter().filter(|u| u.uid == "0").collect();
                    if root_users.len() == 1 {
                        println!("  {} Single root account", "✓".green());
                        score += 10;
                    } else {
                        println!(
                            "  {} Multiple root accounts: {}",
                            "✗".red(),
                            root_users.len()
                        );
                        issues.push("Multiple root accounts detected - security risk");
                    }
                }

                println!();
                println!("{} Step 3/5: Security score calculation...", "→".cyan());
                let grade = if score >= 80 {
                    "A (Excellent)".green().bold()
                } else if score >= 60 {
                    "B (Good)".cyan().bold()
                } else if score >= 40 {
                    "C (Fair)".yellow().bold()
                } else {
                    "D (Poor)".red().bold()
                };

                println!(
                    "  Security Score: {}/100 - Grade: {}",
                    score.to_string().bold(),
                    grade
                );

                println!();
                println!("{} Step 4/5: Recommendations...", "→".cyan());
                if issues.is_empty() {
                    println!("  {} No critical issues found!", "✓".green());
                } else {
                    for (i, issue) in issues.iter().enumerate() {
                        println!("  {}) {}", i + 1, issue.yellow());
                    }
                }

                println!();
                println!("{} Step 5/5: Next steps...", "→".cyan());
                println!("  • Run {} for detailed security info", "'security'".cyan());
                println!(
                    "  • Generate report: {}",
                    "'snapshot security-audit.md'".cyan()
                );
                println!("  • Export data: {}", "'export system json'".cyan());
            }
            println!();
        }
        "health" => {
            println!("\n{}", "🏥 System Health Check Wizard".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            let mut health_score = 100;
            let mut warnings = Vec::new();

            println!("{} Checking system health...", "→".cyan());
            println!();

            // Check 1: Services
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                let ratio = (enabled as f64 / services.len().max(1) as f64) * 100.0;

                println!(
                    "  {} Services: {}/{} enabled ({:.1}%)",
                    "✓".green(),
                    enabled,
                    services.len(),
                    ratio
                );

                if ratio < 30.0 {
                    warnings.push("Low service count - system may be minimal");
                    health_score -= 10;
                }
            }

            // Check 2: Packages
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let count = pkg_info.packages.len();
                println!("  {} Packages: {} installed", "✓".green(), count);

                if count < 100 {
                    warnings.push("Very minimal package set - may lack essential tools");
                    health_score -= 5;
                }
            }

            // Check 3: Users
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("  {} Users: {} accounts", "✓".green(), users.len());
            }

            println!();
            let health_grade = if health_score >= 90 {
                "Excellent".green().bold()
            } else if health_score >= 70 {
                "Good".cyan().bold()
            } else {
                "Fair".yellow().bold()
            };

            println!(
                "{} Health Score: {}/100 ({})",
                "→".cyan(),
                health_score,
                health_grade
            );

            if !warnings.is_empty() {
                println!();
                println!("{}", "Warnings:".yellow().bold());
                for warning in warnings {
                    println!("  {} {}", "⚠".yellow(), warning);
                }
            }

            println!();
            println!(
                "{} Use {} for detailed overview",
                "Tip:".yellow(),
                "'dashboard'".cyan()
            );
            println!();
        }
        "packages" => {
            println!("\n{}", "📦 Package Analysis Wizard".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let packages = &pkg_info.packages;

                println!("{} Analyzing {} packages...", "→".cyan(), packages.len());
                println!();

                // Find interesting packages
                let security_pkgs: Vec<_> = packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("security")
                            || p.name.contains("firewall")
                            || p.name.contains("selinux")
                    })
                    .collect();

                let dev_pkgs: Vec<_> = packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("dev") || p.name.contains("gcc") || p.name.contains("make")
                    })
                    .collect();

                let server_pkgs: Vec<_> = packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("httpd")
                            || p.name.contains("nginx")
                            || p.name.contains("apache")
                    })
                    .collect();

                println!("{}", "Package Categories:".yellow().bold());
                println!("  🔒 Security: {} packages", security_pkgs.len());
                println!("  ⚙ Development: {} packages", dev_pkgs.len());
                println!("  🌐 Web Servers: {} packages", server_pkgs.len());

                println!();
                println!("{}", "Recommendations:".green().bold());
                if server_pkgs.is_empty() {
                    println!("  • No web servers detected - workstation/desktop system");
                } else {
                    println!(
                        "  • Web server detected - review {} output",
                        "'services'".cyan()
                    );
                }

                if dev_pkgs.len() > 50 {
                    println!("  • Heavy development environment detected");
                }

                println!();
                println!(
                    "{} Export package list: {}",
                    "Tip:".yellow(),
                    "'export packages json'".cyan()
                );
            }
            println!();
        }
        "config" => {
            println!("\n{}", "⚙ Configuration Review Wizard".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            println!("{} Reviewing critical configuration files...", "→".cyan());
            println!();

            let config_files = vec![
                "/etc/fstab",
                "/etc/hosts",
                "/etc/resolv.conf",
                "/etc/ssh/sshd_config",
            ];

            for config_file in config_files {
                if ctx.guestfs.exists(config_file).unwrap_or(false) {
                    if let Ok(stat) = ctx.guestfs.stat(config_file) {
                        println!(
                            "  {} {} ({} bytes)",
                            "✓".green(),
                            config_file.cyan(),
                            stat.size
                        );
                    }
                } else {
                    println!("  {} {} (not found)", "✗".red(), config_file);
                }
            }

            println!();
            println!(
                "{} Use {} to examine files",
                "Tip:".yellow(),
                "'cat /etc/fstab'".cyan()
            );
            println!();
        }
        "export" => {
            println!("\n{}", "💾 Export/Report Wizard".yellow().bold());
            println!("{}", "═".repeat(60).cyan());
            println!();

            println!("{} What would you like to export?", "→".cyan());
            println!();
            println!("  1) {} - Complete system snapshot", "Full Report".green());
            println!(
                "  2) {} - All data in JSON format",
                "All Data (JSON)".green()
            );
            println!(
                "  3) {} - Security configuration only",
                "Security Report".green()
            );
            println!("  4) {} - Package inventory", "Package List".green());
            println!();
            println!("{}", "Quick commands:".yellow().bold());
            println!("  Full: {}", "snapshot system-report.md".cyan());
            println!("  JSON: {}", "batch export /tmp/data".cyan());
            println!("  Security: {}", "quick security > security.txt".cyan());
            println!(
                "  Packages: {}",
                "export packages json packages.json".cyan()
            );
            println!();
        }
        _ => {
            println!("{} Unknown wizard: {}", "Error:".red(), wizard_type);
            println!("{} Use 'wizard' to see available wizards", "Tip:".yellow());
        }
    }

    Ok(())
}

/// Comprehensive scanning (security, health, issues)
pub fn cmd_auto(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Automation System                       ║"
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

        println!("{}", "Automation Commands:".yellow().bold());
        println!("  {} - Run preset automation", "auto run <preset>".cyan());
        println!("  {} - List available presets", "auto list".cyan());
        println!("  {} - Show preset details", "auto show <preset>".cyan());
        println!();

        println!("{}", "Available Presets:".green().bold());
        println!("  {} - Complete security audit", "security-audit".cyan());
        println!("  {} - Full system analysis", "full-analysis".cyan());
        println!("  {} - Quick health check", "health-check".cyan());
        println!("  {} - Export all data", "export-all".cyan());
        println!("  {} - Documentation package", "documentation".cyan());
        println!();

        println!("{}", "Example:".yellow());
        println!("  auto run security-audit");
        println!();

        return Ok(());
    }

    let auto_command = args[0];

    match auto_command {
        "run" => {
            if args.len() < 2 {
                println!("{} Usage: auto run <preset>", "Error:".red());
                return Ok(());
            }

            let preset = args[1];

            println!(
                "\n{}",
                "╔═══════════════════════════════════════════════════════════╗"
                    .cyan()
                    .bold()
            );
            println!(
                "{}",
                format!("║  Automation: {}                           ║", preset)
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

            match preset {
                "security-audit" => {
                    println!("{} Running security audit automation...", "→".cyan());
                    println!();

                    println!("[1/4] {} Running security wizard...", "→".cyan());
                    cmd_wizard(ctx, &["security"])?;

                    println!("[2/4] {} Running security scan...", "→".cyan());
                    cmd_scan(ctx, &["security"])?;

                    println!("[3/4] {} Generating recommendations...", "→".cyan());
                    cmd_recommend(ctx, &[])?;

                    println!("[4/4] {} Creating security report...", "→".cyan());
                    cmd_report(ctx, &["security", "--output", "security-audit.md"])?;

                    println!("{}", "═".repeat(60).cyan());
                    println!("{} Security audit complete!", "✓".green());
                    println!(
                        "{} Report saved to: {}",
                        "→".cyan(),
                        "security-audit.md".yellow()
                    );
                    println!();
                }
                "full-analysis" => {
                    println!("{} Running full system analysis...", "→".cyan());
                    println!();

                    println!("[1/5] {} System dashboard...", "→".cyan());
                    cmd_dashboard(ctx, &[])?;

                    println!("[2/5] {} System discovery...", "→".cyan());
                    cmd_discover(ctx, &["all"])?;

                    println!("[3/5] {} Health check...", "→".cyan());
                    cmd_wizard(ctx, &["health"])?;

                    println!("[4/5] {} Creating snapshot...", "→".cyan());
                    cmd_snapshot(ctx, &["full-analysis-snapshot"])?;

                    println!("[5/5] {} Generating executive report...", "→".cyan());
                    cmd_report(ctx, &["executive", "--output", "executive-summary.md"])?;

                    println!("{}", "═".repeat(60).cyan());
                    println!("{} Full analysis complete!", "✓".green());
                    println!();
                }
                "health-check" => {
                    println!("{} Running health check...", "→".cyan());
                    println!();

                    cmd_wizard(ctx, &["health"])?;
                    cmd_scan(ctx, &["issues"])?;
                    cmd_summary(ctx, &[])?;

                    println!("{} Health check complete!", "✓".green());
                    println!();
                }
                "export-all" => {
                    println!("{} Exporting all data...", "→".cyan());
                    println!();

                    cmd_batch(ctx, &["export", "/tmp/guestkit-export"])?;

                    println!(
                        "{} Export complete! Check /tmp/guestkit-export/",
                        "✓".green()
                    );
                    println!();
                }
                "documentation" => {
                    println!("{} Creating documentation package...", "→".cyan());
                    println!();

                    cmd_snapshot(ctx, &["system-documentation"])?;
                    cmd_report(ctx, &["executive", "--output", "executive-summary.md"])?;
                    cmd_report(ctx, &["security", "--output", "security-report.md"])?;
                    cmd_profile(ctx, &["create", "system-profile"])?;

                    println!("{} Documentation package created!", "✓".green());
                    println!("{} Files created:", "→".cyan());
                    println!("  - system-documentation.md");
                    println!("  - executive-summary.md");
                    println!("  - security-report.md");
                    println!("  - system-profile.md");
                    println!();
                }
                _ => {
                    println!("{} Unknown preset: {}", "Error:".red(), preset);
                    println!(
                        "{} Use 'auto list' to see available presets",
                        "Tip:".yellow()
                    );
                }
            }
        }
        "list" => {
            println!("\n{}", "Available Automation Presets:".yellow().bold());
            println!();

            let presets = vec![
                (
                    "security-audit",
                    "Complete security audit with report",
                    "4 steps",
                ),
                ("full-analysis", "Comprehensive system analysis", "5 steps"),
                ("health-check", "Quick system health check", "3 steps"),
                ("export-all", "Export all data types", "1 step"),
                (
                    "documentation",
                    "Create full documentation package",
                    "4 files",
                ),
            ];

            for (name, description, info) in presets {
                println!(
                    "  {} {} - {}",
                    name.cyan().bold(),
                    info.bright_black(),
                    description
                );
            }
            println!();
        }
        "show" => {
            if args.len() < 2 {
                println!("{} Usage: auto show <preset>", "Error:".red());
                return Ok(());
            }

            let preset = args[1];
            println!(
                "\n{} Preset Details: {}",
                "→".cyan(),
                preset.yellow().bold()
            );
            println!();

            match preset {
                "security-audit" => {
                    println!("Steps:");
                    println!("  1. Run security wizard");
                    println!("  2. Run security scan");
                    println!("  3. Generate recommendations");
                    println!("  4. Create security report");
                    println!();
                    println!("Output: security-audit.md");
                }
                _ => {
                    println!("{} Preset not found", "Error:".red());
                }
            }
            println!();
        }
        _ => {
            println!(
                "{} Unknown automation command: {}",
                "Error:".red(),
                auto_command
            );
        }
    }

    Ok(())
}

/// Interactive menu system
pub fn cmd_menu(_ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                  Interactive Menu                        ║"
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

    println!("{}", "Main Categories:".yellow().bold());
    println!();
    println!("  {} System Overview & Analysis", "1.".cyan().bold());
    println!("  {} Security & Compliance", "2.".cyan().bold());
    println!("  {} Data Export & Reports", "3.".cyan().bold());
    println!("  {} Search & Discovery", "4.".cyan().bold());
    println!("  {} Automation & Workflows", "5.".cyan().bold());
    println!("  {} Help & Documentation", "6.".cyan().bold());
    println!();

    println!("{}", "━".repeat(60).bright_black());
    println!();

    println!("{}", "Quick Actions:".green().bold());
    println!("  {} Quick security check", "S.".yellow());
    println!("  {} System dashboard", "D.".yellow());
    println!("  {} Create snapshot", "N.".yellow());
    println!("  {} Smart recommendations", "R.".yellow());
    println!();

    println!("{}", "Suggestions:".bright_black());
    println!("  • First time? Try: {}", "dashboard".cyan());
    println!("  • Security review? Try: {}", "wizard security".cyan());
    println!("  • Need export? Try: {}", "auto run export-all".cyan());
    println!("  • Want guidance? Try: {}", "wizard".cyan());
    println!();

    Ok(())
}

/// Visual timeline and progress tracking
pub fn cmd_presets(_ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║                  Command Presets                         ║"
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

    println!("{}", "Quick Start Presets:".yellow().bold());
    println!();

    let presets = vec![
        (
            "First Time User",
            vec![
                ("Start here", "dashboard"),
                ("Learn commands", "cheat"),
                ("Get a tip", "tips"),
            ],
        ),
        (
            "Security Analyst",
            vec![
                ("Security audit", "wizard security"),
                ("Security scan", "scan security"),
                ("Get recommendations", "recommend"),
            ],
        ),
        (
            "System Administrator",
            vec![
                ("Full analysis", "auto run full-analysis"),
                ("Health check", "wizard health"),
                ("Create snapshot", "snapshot"),
            ],
        ),
        (
            "Auditor/Compliance",
            vec![
                ("Executive report", "report executive --output summary.md"),
                ("Security report", "report security --output security.md"),
                ("Export all data", "batch export /tmp/audit"),
            ],
        ),
        (
            "Developer/Researcher",
            vec![
                ("Discover apps", "discover apps"),
                ("Find files", "search <pattern> --path /etc"),
                ("Profile system", "profile detect"),
            ],
        ),
    ];

    for (role, commands) in presets {
        println!("{}", role.green().bold());
        for (description, command) in commands {
            println!(
                "  {} {} {}",
                "•".cyan(),
                description.bright_black(),
                "-".bright_black()
            );
            println!("    {}", command.yellow());
        }
        println!();
    }

    println!("{}", "Workflow Templates:".yellow().bold());
    println!();
    println!("  {} Complete Audit", "1.".cyan());
    println!("     auto run security-audit");
    println!();
    println!("  {} Documentation Package", "2.".cyan());
    println!("     auto run documentation");
    println!();
    println!("  {} Quick Health Check", "3.".cyan());
    println!("     wizard health && recommend");
    println!();

    Ok(())
}

// Helper functions for new commands

fn export_packages(ctx: &mut ShellContext, format: &str, output: Option<&str>) -> Result<()> {
    let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
    let packages = &pkg_info.packages;

    let content = match format {
        "json" => {
            let mut json_items = Vec::new();
            for pkg in packages {
                json_items.push(format!(
                    r#"  {{"name": "{}", "version": "{}"}}"#,
                    pkg.name, pkg.version
                ));
            }
            format!("[\n{}\n]", json_items.join(",\n"))
        }
        "csv" => {
            let mut lines = vec!["name,version".to_string()];
            for pkg in packages {
                lines.push(format!("{},{}", pkg.name, pkg.version));
            }
            lines.join("\n")
        }
        "md" => {
            let mut lines = vec![
                "| Package | Version |".to_string(),
                "|---------|---------|".to_string(),
            ];
            for pkg in packages {
                lines.push(format!("| {} | {} |", pkg.name, pkg.version));
            }
            lines.join("\n")
        }
        _ => {
            let mut lines = Vec::new();
            for pkg in packages {
                lines.push(format!("{} - {}", pkg.name, pkg.version));
            }
            lines.join("\n")
        }
    };

    if let Some(file) = output {
        use std::fs;
        fs::write(file, content)?;
        println!("{} Written to: {}", "→".cyan(), file.yellow());
    } else {
        println!("{}", content);
    }

    Ok(())
}

fn export_users(ctx: &mut ShellContext, format: &str, output: Option<&str>) -> Result<()> {
    let users = ctx.guestfs.inspect_users(&ctx.root)?;

    let content = match format {
        "json" => {
            let mut json_items = Vec::new();
            for user in users {
                json_items.push(format!(
                    r#"  {{"username": "{}", "uid": "{}", "gid": "{}", "home": "{}"}}"#,
                    user.username, user.uid, user.gid, user.home
                ));
            }
            format!("[\n{}\n]", json_items.join(",\n"))
        }
        "csv" => {
            let mut lines = vec!["username,uid,gid,home".to_string()];
            for user in users {
                lines.push(format!(
                    "{},{},{},{}",
                    user.username, user.uid, user.gid, user.home
                ));
            }
            lines.join("\n")
        }
        "md" => {
            let mut lines = vec![
                "| Username | UID | GID | Home |".to_string(),
                "|----------|-----|-----|------|".to_string(),
            ];
            for user in users {
                lines.push(format!(
                    "| {} | {} | {} | {} |",
                    user.username, user.uid, user.gid, user.home
                ));
            }
            lines.join("\n")
        }
        _ => {
            let mut lines = Vec::new();
            for user in users {
                lines.push(format!(
                    "{} ({}:{}) - {}",
                    user.username, user.uid, user.gid, user.home
                ));
            }
            lines.join("\n")
        }
    };

    if let Some(file) = output {
        use std::fs;
        fs::write(file, content)?;
        println!("{} Written to: {}", "→".cyan(), file.yellow());
    } else {
        println!("{}", content);
    }

    Ok(())
}

fn export_services(ctx: &mut ShellContext, format: &str, output: Option<&str>) -> Result<()> {
    let services = ctx.guestfs.inspect_systemd_services(&ctx.root)?;

    let content = match format {
        "json" => {
            let mut json_items = Vec::new();
            for svc in services {
                json_items.push(format!(
                    r#"  {{"name": "{}", "enabled": {}}}"#,
                    svc.name, svc.enabled
                ));
            }
            format!("[\n{}\n]", json_items.join(",\n"))
        }
        "csv" => {
            let mut lines = vec!["name,enabled".to_string()];
            for svc in services {
                lines.push(format!("{},{}", svc.name, svc.enabled));
            }
            lines.join("\n")
        }
        "md" => {
            let mut lines = vec![
                "| Service | Enabled |".to_string(),
                "|---------|---------|".to_string(),
            ];
            for svc in services {
                lines.push(format!("| {} | {} |", svc.name, svc.enabled));
            }
            lines.join("\n")
        }
        _ => {
            let mut lines = Vec::new();
            for svc in services {
                let status = if svc.enabled { "enabled" } else { "disabled" };
                lines.push(format!("{} - {}", svc.name, status));
            }
            lines.join("\n")
        }
    };

    if let Some(file) = output {
        use std::fs;
        fs::write(file, content)?;
        println!("{} Written to: {}", "→".cyan(), file.yellow());
    } else {
        println!("{}", content);
    }

    Ok(())
}

fn export_system(ctx: &mut ShellContext, format: &str, output: Option<&str>) -> Result<()> {
    let os_type = ctx
        .guestfs
        .inspect_get_type(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());
    let distro = ctx
        .guestfs
        .inspect_get_distro(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());
    let version = ctx
        .guestfs
        .inspect_get_product_name(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());
    let arch = ctx
        .guestfs
        .inspect_get_arch(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());
    let hostname = ctx
        .guestfs
        .inspect_get_hostname(&ctx.root)
        .unwrap_or_else(|_| "unknown".to_string());

    let content = match format {
        "json" => {
            format!(
                r#"{{
  "type": "{}",
  "distribution": "{}",
  "version": "{}",
  "architecture": "{}",
  "hostname": "{}"
}}"#,
                os_type, distro, version, arch, hostname
            )
        }
        "md" => {
            format!(
                "# System Information\n\n\
                | Property | Value |\n\
                |----------|-------|\n\
                | Type | {} |\n\
                | Distribution | {} |\n\
                | Version | {} |\n\
                | Architecture | {} |\n\
                | Hostname | {} |",
                os_type, distro, version, arch, hostname
            )
        }
        _ => {
            format!(
                "System Information:\n\
                  Type: {}\n\
                  Distribution: {}\n\
                  Version: {}\n\
                  Architecture: {}\n\
                  Hostname: {}",
                os_type, distro, version, arch, hostname
            )
        }
    };

    if let Some(file) = output {
        use std::fs;
        fs::write(file, content)?;
        println!("{} Written to: {}", "→".cyan(), file.yellow());
    } else {
        println!("{}", content);
    }

    Ok(())
}

pub fn cmd_learn(_ctx: &ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  Learning Center                         ║"
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

        println!("{}", "Available Tutorials:".yellow().bold());
        println!("{}", "─".repeat(70).cyan());
        println!();

        let tutorials = vec![
            ("basics", "Getting started with GuestKit", "5 min", "🎓"),
            ("navigation", "Filesystem navigation", "3 min", "🗺"),
            ("security", "Security analysis workflow", "10 min", "🔒"),
            ("export", "Exporting data and reports", "5 min", "💾"),
            (
                "advanced",
                "Advanced search and batch operations",
                "8 min",
                "⚡",
            ),
            ("automation", "Automation and presets", "7 min", "🤖"),
        ];

        for (name, desc, duration, icon) in tutorials {
            println!(
                "  {} {} - {} {}",
                icon,
                name.green().bold(),
                desc,
                format!("({})", duration).bright_black()
            );
        }

        println!();
        println!("{} learn <tutorial>", "Usage:".yellow());
        println!("{} learn basics", "Example:".cyan());
        println!();
        return Ok(());
    }

    let tutorial = args[0];

    match tutorial {
        "basics" => {
            println!(
                "\n{} {}",
                "📚".cyan(),
                "Tutorial: Getting Started with GuestKit".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Introduction", "Step 1:".green().bold());
            println!("GuestKit is a powerful VM inspection shell. You're currently inside");
            println!("a mounted VM filesystem, allowing you to explore it safely.\n");

            println!("{} Basic Navigation", "Step 2:".green().bold());
            println!("  • {} - See where you are", "pwd".cyan());
            println!("  • {} - List files and directories", "ls".cyan());
            println!("  • {} - Change directory", "cd <path>".cyan());
            println!("  • {} - Read file contents", "cat <file>".cyan());
            println!();

            println!("{} Getting Information", "Step 3:".green().bold());
            println!("  • {} - Beautiful system overview", "dashboard".cyan());
            println!("  • {} - Quick one-line summary", "summary".cyan());
            println!("  • {} - System information", "info".cyan());
            println!();

            println!("{} Try it now!", "💡".yellow());
            println!("  Type: {}", "dashboard".green());
            println!();
        }

        "navigation" => {
            println!(
                "\n{} {}",
                "🗺".cyan(),
                "Tutorial: Filesystem Navigation".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Understanding Paths", "Lesson 1:".green().bold());
            println!(
                "  • Absolute paths start with /   Example: {}",
                "/etc/fstab".cyan()
            );
            println!("  • Relative paths are from current location");
            println!("  • {} goes up one directory", "..".cyan());
            println!("  • {} stays in current directory", ".".cyan());
            println!();

            println!("{} Quick Navigation", "Lesson 2:".green().bold());
            println!(
                "  • {} - Create bookmarks for favorite locations",
                "bookmark".cyan()
            );
            println!("  • {} - Jump to a bookmark", "goto <name>".cyan());
            println!("  • {} - Quick command aliases", "alias".cyan());
            println!();

            println!("{} Visual Tools", "Lesson 3:".green().bold());
            println!("  • {} - Visualize directory structure", "tree".cyan());
            println!("  • {} - Find files by pattern", "find <pattern>".cyan());
            println!("  • {} - Search with filters", "search <pattern>".cyan());
            println!();

            println!("{} Try it!", "💡".yellow());
            println!("  1. {}", "bookmark myspot".green());
            println!("  2. {}", "cd /etc".green());
            println!("  3. {}", "goto myspot".green());
            println!();
        }

        "security" => {
            println!(
                "\n{} {}",
                "🔒".cyan(),
                "Tutorial: Security Analysis".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Quick Security Check", "Step 1:".green().bold());
            println!("  Run: {}", "security".cyan());
            println!("  This shows SELinux, AppArmor, Firewall, and audit status\n");

            println!("{} Security Wizard", "Step 2:".green().bold());
            println!("  Run: {}", "wizard security".cyan());
            println!("  Get a security score (A-D grade) with detailed analysis\n");

            println!("{} Vulnerability Scanning", "Step 3:".green().bold());
            println!("  Run: {}", "scan security".cyan());
            println!("  Find security issues with severity ratings\n");

            println!("{} Get Recommendations", "Step 4:".green().bold());
            println!("  Run: {}", "recommend".cyan());
            println!("  Receive prioritized security recommendations\n");

            println!("{} Complete Audit", "Step 5:".green().bold());
            println!("  Run: {}", "auto run security-audit".cyan());
            println!("  Automated full security audit with report generation\n");

            println!("{} Try the wizard now!", "💡".yellow());
            println!("  Type: {}", "wizard security".green());
            println!();
        }

        "export" => {
            println!(
                "\n{} {}",
                "💾".cyan(),
                "Tutorial: Exporting Data".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Export Formats", "Step 1:".green().bold());
            println!("  GuestKit supports: JSON, CSV, Markdown, Plain text\n");

            println!("{} Export Examples", "Step 2:".green().bold());
            println!(
                "  • {} - Package list as JSON",
                "export packages json packages.json".cyan()
            );
            println!("  • {} - Users as CSV", "export users csv users.csv".cyan());
            println!(
                "  • {} - Services as Markdown",
                "export services md services.md".cyan()
            );
            println!();

            println!("{} Snapshots", "Step 3:".green().bold());
            println!(
                "  • {} - Complete system snapshot",
                "snapshot report.md".cyan()
            );
            println!("  Creates comprehensive Markdown report with all data\n");

            println!("{} Batch Export", "Step 4:".green().bold());
            println!(
                "  • {} - Export everything",
                "batch export /tmp/reports".cyan()
            );
            println!(
                "  • {} - All data in one command",
                "auto run export-all".cyan()
            );
            println!();

            println!("{} Try it!", "💡".yellow());
            println!("  Type: {}", "snapshot my-system.md".green());
            println!();
        }

        "advanced" => {
            println!(
                "\n{} {}",
                "⚡".cyan(),
                "Tutorial: Advanced Features".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Smart Search", "Technique 1:".green().bold());
            println!(
                "  • {} - Search by path",
                "search <pattern> --path /etc".cyan()
            );
            println!(
                "  • {} - Search by type",
                "search <pattern> --type file".cyan()
            );
            println!(
                "  • {} - Search in content",
                "search <pattern> --content".cyan()
            );
            println!();

            println!("{} Batch Operations", "Technique 2:".green().bold());
            println!(
                "  • {} - Read multiple files",
                "batch cat file1 file2 file3".cyan()
            );
            println!(
                "  • {} - Search multiple dirs",
                "batch find pattern /etc /var".cyan()
            );
            println!();

            println!("{} Pin Favorites", "Technique 3:".green().bold());
            println!(
                "  • {} - Save command",
                "pin errors 'grep ERROR /var/log'".cyan()
            );
            println!("  • {} - Run pinned command", "pin run errors".cyan());
            println!();

            println!("{} Recent Files", "Technique 4:".green().bold());
            println!("  • {} - Recently modified", "recent /var/log 20".cyan());
            println!();

            println!("{} Try it!", "💡".yellow());
            println!(
                "  Type: {}",
                "search error --content --path /var/log".green()
            );
            println!();
        }

        "automation" => {
            println!(
                "\n{} {}",
                "🤖".cyan(),
                "Tutorial: Automation & Presets".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Automation Presets", "Step 1:".green().bold());
            println!(
                "  • {} - Complete security workflow",
                "auto run security-audit".cyan()
            );
            println!(
                "  • {} - Full system analysis",
                "auto run full-analysis".cyan()
            );
            println!("  • {} - Health assessment", "auto run health-check".cyan());
            println!("  • {} - Export everything", "auto run export-all".cyan());
            println!();

            println!("{} Interactive Menu", "Step 2:".green().bold());
            println!("  • {} - Navigate via menu", "menu".cyan());
            println!("  Choose from categorized options\n");

            println!("{} Role-Based Presets", "Step 3:".green().bold());
            println!("  • {} - Get commands for your role", "presets".cyan());
            println!("  Roles: Security Analyst, SysAdmin, Developer, Auditor\n");

            println!("{} Benchmarking", "Step 4:".green().bold());
            println!("  • {} - Performance testing", "bench <type>".cyan());
            println!();

            println!("{} Try a full analysis!", "💡".yellow());
            println!("  Type: {}", "auto run full-analysis".green());
            println!();
        }

        _ => {
            println!("{} Unknown tutorial: {}", "Error:".red(), tutorial);
            println!("{} learn", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Focus mode for specific system aspects
pub fn cmd_focus(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("\n{}", "Usage: focus <aspect>".red());
        println!();
        println!("{}", "Available Focus Areas:".yellow().bold());
        println!(
            "  {} - Security posture and vulnerabilities",
            "security".green()
        );
        println!(
            "  {} - Performance and resource usage",
            "performance".green()
        );
        println!(
            "  {} - Network configuration and connectivity",
            "network".green()
        );
        println!("  {} - Storage and filesystems", "storage".green());
        println!("  {} - User accounts and permissions", "users".green());
        println!();
        return Ok(());
    }

    let aspect = args[0];

    match aspect {
        "security" => {
            println!(
                "\n{} {}",
                "🔒".cyan(),
                "Security Focus Mode".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("{}", "Security Status:".green().bold());
                println!(
                    "  SELinux:  {}",
                    if &sec.selinux != "disabled" {
                        sec.selinux.green()
                    } else {
                        sec.selinux.red()
                    }
                );
                println!(
                    "  AppArmor: {}",
                    if sec.apparmor {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
                );
                println!(
                    "  auditd:   {}",
                    if sec.auditd {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
                );
                println!();
            }

            if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                println!("{}", "Firewall Configuration:".green().bold());
                println!("  Type:   {}", fw.firewall_type.cyan());
                println!(
                    "  Status: {}",
                    if fw.enabled {
                        "enabled".green()
                    } else {
                        "disabled".red()
                    }
                );
                println!();
            }

            println!("{}", "Critical Files to Review:".yellow().bold());
            let security_files = vec![
                "/etc/shadow",
                "/etc/sudoers",
                "/etc/ssh/sshd_config",
                "/etc/pam.d/",
                "/etc/security/",
                "/etc/selinux/config",
            ];
            for file in security_files {
                let exists = ctx.guestfs.exists(file).unwrap_or(false);
                let status = if exists { "✓".green() } else { "✗".red() };
                println!("  {} {}", status, file.cyan());
            }
            println!();

            println!("{} Next Steps:", "💡".yellow());
            println!("  • {}", "wizard security - Get security score".cyan());
            println!("  • {}", "scan security - Find vulnerabilities".cyan());
            println!("  • {}", "recommend - Get security recommendations".cyan());
            println!();
        }

        "performance" => {
            println!(
                "\n{} {}",
                "⚡".cyan(),
                "Performance Focus Mode".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("{}", "Package Statistics:".green().bold());
                println!(
                    "  Total packages: {}",
                    pkg_info.packages.len().to_string().yellow()
                );
                println!();
            }

            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                println!("{}", "Service Statistics:".green().bold());
                println!("  Total services: {}", services.len().to_string().yellow());
                println!("  Enabled: {}", enabled.to_string().green());
                println!(
                    "  Disabled: {}",
                    (services.len() - enabled).to_string().bright_black()
                );
                println!();
            }

            println!("{} Benchmarking:", "💡".yellow());
            println!("  • {}", "bench files - Test filesystem operations".cyan());
            println!("  • {}", "bench list - Test directory listing".cyan());
            println!("  • {}", "bench packages - Test package queries".cyan());
            println!();
        }

        "network" => {
            println!("\n{} {}", "🌐".cyan(), "Network Focus Mode".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
                println!(
                    "{} ({} total)",
                    "Network Interfaces:".green().bold(),
                    interfaces.len()
                );
                for iface in interfaces {
                    println!("  • {}", iface.name.cyan());
                }
                println!();
            }

            if let Ok(dns) = ctx.guestfs.inspect_dns(&ctx.root) {
                if !dns.is_empty() {
                    println!("{}", "DNS Servers:".green().bold());
                    for server in dns {
                        println!("  • {}", server.yellow());
                    }
                    println!();
                }
            }

            println!("{}", "Network Configuration Files:".yellow().bold());
            let net_files = vec![
                "/etc/hosts",
                "/etc/resolv.conf",
                "/etc/hostname",
                "/etc/sysconfig/network",
                "/etc/network/interfaces",
            ];
            for file in net_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!("  {} {}", "✓".green(), file.cyan());
                }
            }
            println!();

            println!("{} Explore further:", "💡".yellow());
            println!("  • {}", "cat /etc/hosts".cyan());
            println!("  • {}", "discover network".cyan());
            println!();
        }

        "storage" => {
            println!("\n{} {}", "💾".cyan(), "Storage Focus Mode".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(devices) = ctx.guestfs.list_devices() {
                println!(
                    "{} ({} total)",
                    "Block Devices:".green().bold(),
                    devices.len()
                );
                for device in devices {
                    println!("  • {}", device.cyan());
                }
                println!();
            }

            if let Ok(filesystems) = ctx.guestfs.list_filesystems() {
                println!("{}", "Filesystems:".green().bold());
                for (device, fstype) in filesystems {
                    println!("  {} - {}", device.yellow(), fstype.cyan());
                }
                println!();
            }

            println!("{} Storage Commands:", "💡".yellow());
            println!("  • {}", "mounts - View mounted filesystems".cyan());
            println!("  • {}", "cat /etc/fstab - View mount configuration".cyan());
            println!("  • {}", "tree / 2 - Filesystem overview".cyan());
            println!();
        }

        "users" => {
            println!(
                "\n{} {}",
                "👥".cyan(),
                "User Accounts Focus Mode".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let root_users: Vec<_> = users.iter().filter(|u| u.uid == "0").collect();
                let system_users: Vec<_> = users
                    .iter()
                    .filter(|u| {
                        if let Ok(uid) = u.uid.parse::<u32>() {
                            uid > 0 && uid < 1000
                        } else {
                            false
                        }
                    })
                    .collect();
                let regular_users: Vec<_> = users
                    .iter()
                    .filter(|u| {
                        if let Ok(uid) = u.uid.parse::<u32>() {
                            uid >= 1000
                        } else {
                            false
                        }
                    })
                    .collect();

                println!("{}", "User Statistics:".green().bold());
                println!(
                    "  Root accounts:    {} {}",
                    root_users.len().to_string().red().bold(),
                    if root_users.len() > 1 {
                        "(⚠ Multiple root accounts!)".yellow()
                    } else {
                        "".normal()
                    }
                );
                println!(
                    "  System accounts:  {}",
                    system_users.len().to_string().cyan()
                );
                println!(
                    "  Regular accounts: {}",
                    regular_users.len().to_string().green()
                );
                println!();

                println!("{}", "Regular Users:".yellow().bold());
                for user in regular_users.iter().take(10) {
                    println!(
                        "  {} (UID: {}, Home: {})",
                        user.username.green(),
                        user.uid.bright_black(),
                        user.home.cyan()
                    );
                }
                println!();
            }

            println!("{}", "User Configuration Files:".yellow().bold());
            let user_files = vec![
                ("/etc/passwd", "User accounts"),
                ("/etc/shadow", "Password hashes"),
                ("/etc/group", "Group definitions"),
                ("/etc/sudoers", "Sudo privileges"),
            ];
            for (file, desc) in user_files {
                let exists = ctx.guestfs.exists(file).unwrap_or(false);
                let status = if exists { "✓".green() } else { "✗".red() };
                println!("  {} {} - {}", status, file.cyan(), desc.bright_black());
            }
            println!();

            println!("{} Deep dive:", "💡".yellow());
            println!("  • {}", "users - Full user list".cyan());
            println!("  • {}", "cat /etc/passwd".cyan());
            println!();
        }

        _ => {
            println!("{} Unknown focus area: {}", "Error:".red(), aspect);
            println!("{} focus <aspect>", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Security and operations playbooks
pub fn cmd_playbook(_ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                    Playbook Library                      ║"
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

        println!("{}", "Available Playbooks:".yellow().bold());
        println!("{}", "─".repeat(70).cyan());
        println!();

        let playbooks = vec![
            ("incident", "🚨", "Security incident response", "Advanced"),
            (
                "hardening",
                "🔒",
                "System security hardening",
                "Intermediate",
            ),
            ("audit", "📋", "Compliance audit checklist", "Intermediate"),
            (
                "forensics",
                "🔍",
                "Digital forensics investigation",
                "Advanced",
            ),
            (
                "migration",
                "📦",
                "VM migration preparation",
                "Intermediate",
            ),
            (
                "recovery",
                "🔧",
                "System recovery procedures",
                "Intermediate",
            ),
        ];

        for (name, icon, desc, level) in playbooks {
            let level_color = match level {
                "Advanced" => level.red(),
                "Intermediate" => level.yellow(),
                _ => level.green(),
            };
            println!(
                "  {} {} - {} {}",
                icon,
                name.green().bold(),
                desc,
                format!("[{}]", level_color).bright_black()
            );
        }

        println!();
        println!("{} playbook <name>", "Usage:".yellow());
        println!("{} playbook incident", "Example:".cyan());
        println!();
        return Ok(());
    }

    let playbook = args[0];

    match playbook {
        "incident" => {
            println!(
                "\n{} {}",
                "🚨".cyan(),
                "Security Incident Response Playbook".red().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Immediate Actions", "Phase 1:".red().bold());
            println!(
                "  {} Document current time and create snapshot",
                "1.".yellow()
            );
            println!(
                "     Command: {}",
                "snapshot incident-$(date +%Y%m%d-%H%M%S).md".cyan()
            );
            println!("  {} Capture system state", "2.".yellow());
            println!("     Command: {}", "dashboard".cyan());
            println!("  {} Check currently logged in users", "3.".yellow());
            println!("     Command: {}", "users".cyan());
            println!();

            println!("{} Investigation", "Phase 2:".yellow().bold());
            println!("  {} Review security configuration", "4.".yellow());
            println!("     Command: {}", "security".cyan());
            println!("  {} Scan for security issues", "5.".yellow());
            println!("     Command: {}", "scan security".cyan());
            println!("  {} Check recent file modifications", "6.".yellow());
            println!("     Command: {}", "recent /etc 50".cyan());
            println!("     Command: {}", "recent /var/log 50".cyan());
            println!("  {} Search for suspicious activity", "7.".yellow());
            println!(
                "     Command: {}",
                "search failed --content --path /var/log".cyan()
            );
            println!(
                "     Command: {}",
                "search unauthorized --content --path /var/log".cyan()
            );
            println!();

            println!("{} Analysis", "Phase 3:".green().bold());
            println!("  {} Review network configuration", "8.".yellow());
            println!("     Command: {}", "network".cyan());
            println!("  {} Check running services", "9.".yellow());
            println!("     Command: {}", "services".cyan());
            println!("  {} Analyze installed packages", "10.".yellow());
            println!("     Command: {}", "packages".cyan());
            println!();

            println!("{} Reporting", "Phase 4:".cyan().bold());
            println!("  {} Generate comprehensive report", "11.".yellow());
            println!(
                "     Command: {}",
                "report security --output incident-report.md".cyan()
            );
            println!("  {} Export all evidence", "12.".yellow());
            println!(
                "     Command: {}",
                "batch export /tmp/incident-evidence".cyan()
            );
            println!();

            println!(
                "{} This playbook helps investigate security incidents systematically",
                "Note:".yellow().bold()
            );
            println!();
        }

        "hardening" => {
            println!(
                "\n{} {}",
                "🔒".cyan(),
                "System Security Hardening Playbook".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Assessment", "Step 1:".green().bold());
            println!("  • Run security wizard: {}", "wizard security".cyan());
            println!("  • Get recommendations: {}", "recommend".cyan());
            println!();

            println!("{} Security Features", "Step 2:".green().bold());
            println!("  {} Check SELinux status", "•".yellow());
            println!("     Location: {}", "/etc/selinux/config".cyan());
            println!("     Command: {}", "cat /etc/selinux/config".cyan());
            println!("  {} Verify AppArmor profiles", "•".yellow());
            println!("     Command: {}", "cat /etc/apparmor.d/".cyan());
            println!("  {} Review firewall rules", "•".yellow());
            println!("     Command: {}", "security".cyan());
            println!();

            println!("{} User Security", "Step 3:".green().bold());
            println!("  {} Audit user accounts", "•".yellow());
            println!("     Command: {}", "users".cyan());
            println!("  {} Check sudo configuration", "•".yellow());
            println!("     Command: {}", "cat /etc/sudoers".cyan());
            println!("  {} Review SSH configuration", "•".yellow());
            println!("     Command: {}", "cat /etc/ssh/sshd_config".cyan());
            println!();

            println!("{} System Services", "Step 4:".green().bold());
            println!("  {} List all enabled services", "•".yellow());
            println!("     Command: {}", "services".cyan());
            println!("  {} Identify unnecessary services", "•".yellow());
            println!("     Review output and disable unused services");
            println!();

            println!("{} Verification", "Step 5:".green().bold());
            println!("  {} Run security scan", "•".yellow());
            println!("     Command: {}", "scan security".cyan());
            println!("  {} Generate compliance report", "•".yellow());
            println!("     Command: {}", "report compliance".cyan());
            println!();
        }

        "audit" => {
            println!(
                "\n{} {}",
                "📋".cyan(),
                "Compliance Audit Checklist".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "System Information:".green().bold());
            println!(
                "  {} {}",
                "☐".yellow(),
                "System overview - dashboard".cyan()
            );
            println!("  {} {}", "☐".yellow(), "OS and version - info".cyan());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Installed packages - packages".cyan()
            );
            println!();

            println!("{}", "Security Controls:".green().bold());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Security features - security".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "Firewall configuration - security".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "SELinux/AppArmor status - security".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "Security audit - wizard security".cyan()
            );
            println!();

            println!("{}", "Access Controls:".green().bold());
            println!("  {} {}", "☐".yellow(), "User accounts - users".cyan());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Sudo privileges - cat /etc/sudoers".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "SSH configuration - cat /etc/ssh/sshd_config".cyan()
            );
            println!();

            println!("{}", "Network Security:".green().bold());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Network configuration - network".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "Open ports and services - services".cyan()
            );
            println!();

            println!("{}", "Logging & Monitoring:".green().bold());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Audit daemon status - security".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "Log files review - recent /var/log 50".cyan()
            );
            println!();

            println!("{}", "Documentation:".green().bold());
            println!(
                "  {} {}",
                "☐".yellow(),
                "Generate snapshot - snapshot audit-report.md".cyan()
            );
            println!(
                "  {} {}",
                "☐".yellow(),
                "Compliance report - report compliance".cyan()
            );
            println!();
        }

        "forensics" => {
            println!(
                "\n{} {}",
                "🔍".cyan(),
                "Digital Forensics Investigation Playbook".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Preservation", "Phase 1:".red().bold());
            println!("  {} Create complete snapshot immediately", "1.".yellow());
            println!(
                "     {}",
                "snapshot forensics-$(date +%Y%m%d-%H%M%S).md".cyan()
            );
            println!("  {} Export all data for analysis", "2.".yellow());
            println!("     {}", "auto run export-all".cyan());
            println!();

            println!("{} System Analysis", "Phase 2:".yellow().bold());
            println!("  {} Profile the system", "3.".yellow());
            println!("     {}", "profile detect".cyan());
            println!("  {} Review system configuration", "4.".yellow());
            println!("     {}", "info".cyan());
            println!();

            println!("{} Timeline Analysis", "Phase 3:".green().bold());
            println!("  {} Find recently modified files", "5.".yellow());
            println!("     {}", "recent / 100".cyan());
            println!("  {} Check specific directories", "6.".yellow());
            println!("     {}", "recent /etc 50".cyan());
            println!("     {}", "recent /var 50".cyan());
            println!("     {}", "recent /home 50".cyan());
            println!();

            println!("{} Evidence Collection", "Phase 4:".cyan().bold());
            println!("  {} User activity", "7.".yellow());
            println!("     {}", "users".cyan());
            println!("     {}", "cat /var/log/auth.log".cyan());
            println!("  {} Network connections", "8.".yellow());
            println!("     {}", "network".cyan());
            println!("  {} Installed software", "9.".yellow());
            println!("     {}", "packages".cyan());
            println!("  {} Running services", "10.".yellow());
            println!("     {}", "services".cyan());
            println!();

            println!("{} Content Analysis", "Phase 5:".blue().bold());
            println!("  {} Search for indicators of compromise", "11.".yellow());
            println!("     {}", "search <ioc> --content".cyan());
            println!("  {} Batch file examination", "12.".yellow());
            println!("     {}", "batch cat <files...>".cyan());
            println!();

            println!("{} Reporting", "Phase 6:".magenta().bold());
            println!("  {} Generate technical report", "13.".yellow());
            println!(
                "     {}",
                "report technical --output forensics-report.md".cyan()
            );
            println!();
        }

        "migration" => {
            println!(
                "\n{} {}",
                "📦".cyan(),
                "VM Migration Preparation Playbook".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Discovery", "Step 1:".green().bold());
            println!("  {} System profile", "•".yellow());
            println!("     {}", "profile create".cyan());
            println!("  {} Full analysis", "•".yellow());
            println!("     {}", "auto run full-analysis".cyan());
            println!();

            println!("{} Documentation", "Step 2:".green().bold());
            println!("  {} Create comprehensive snapshot", "•".yellow());
            println!("     {}", "snapshot pre-migration.md".cyan());
            println!("  {} Export configuration data", "•".yellow());
            println!("     {}", "export system json system-config.json".cyan());
            println!();

            println!("{} Configuration Review", "Step 3:".green().bold());
            println!("  {} Network settings", "•".yellow());
            println!("     {}", "network".cyan());
            println!("     {}", "cat /etc/hosts".cyan());
            println!("  {} Storage and mounts", "•".yellow());
            println!("     {}", "mounts".cyan());
            println!("     {}", "cat /etc/fstab".cyan());
            println!("  {} Services", "•".yellow());
            println!("     {}", "services".cyan());
            println!();

            println!("{} Dependencies", "Step 4:".green().bold());
            println!("  {} Installed packages", "•".yellow());
            println!("     {}", "export packages csv packages.csv".cyan());
            println!("  {} User accounts", "•".yellow());
            println!("     {}", "export users csv users.csv".cyan());
            println!();

            println!("{} Final Report", "Step 5:".green().bold());
            println!("  {} Generate executive summary", "•".yellow());
            println!(
                "     {}",
                "report executive --output migration-plan.md".cyan()
            );
            println!();
        }

        "recovery" => {
            println!(
                "\n{} {}",
                "🔧".cyan(),
                "System Recovery Procedures".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{} Assessment", "Phase 1:".green().bold());
            println!("  {} Check system health", "1.".yellow());
            println!("     {}", "wizard health".cyan());
            println!("  {} Identify issues", "2.".yellow());
            println!("     {}", "scan issues".cyan());
            println!();

            println!("{} Critical Files", "Phase 2:".green().bold());
            println!("  {} Verify boot configuration", "3.".yellow());
            println!("     {}", "cat /etc/fstab".cyan());
            println!("     {}", "cat /boot/grub/grub.cfg".cyan());
            println!("  {} Check network configuration", "4.".yellow());
            println!("     {}", "network".cyan());
            println!();

            println!("{} Services", "Phase 3:".green().bold());
            println!("  {} Review service status", "5.".yellow());
            println!("     {}", "services".cyan());
            println!("  {} Check critical services", "6.".yellow());
            println!("     Look for failed or disabled critical services");
            println!();

            println!("{} Logs", "Phase 4:".green().bold());
            println!("  {} Search for errors", "7.".yellow());
            println!("     {}", "search error --content --path /var/log".cyan());
            println!("     {}", "search fail --content --path /var/log".cyan());
            println!("  {} Recent log activity", "8.".yellow());
            println!("     {}", "recent /var/log 50".cyan());
            println!();

            println!("{} Documentation", "Phase 5:".green().bold());
            println!("  {} Create recovery snapshot", "9.".yellow());
            println!("     {}", "snapshot recovery-assessment.md".cyan());
            println!();
        }

        _ => {
            println!("{} Unknown playbook: {}", "Error:".red(), playbook);
            println!("{} playbook", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Deep inspection of specific components
pub fn cmd_story(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("\n{}", "Usage: story <topic>".red());
        println!();
        println!("{}", "Available Story Topics:".yellow().bold());
        println!("  {} - System origin and purpose story", "system".green());
        println!("  {} - Security posture narrative", "security".green());
        println!("  {} - Configuration journey", "config".green());
        println!("  {} - What happened to this system", "timeline".green());
        println!();
        return Ok(());
    }

    let topic = args[0];

    match topic {
        "system" => {
            println!("\n{} {}", "📖".cyan(), "System Story".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            // Gather information
            let os_type = ctx
                .guestfs
                .inspect_get_type(&ctx.root)
                .unwrap_or_else(|_| "unknown".to_string());
            let distro = ctx
                .guestfs
                .inspect_get_distro(&ctx.root)
                .unwrap_or_else(|_| "unknown".to_string());
            let arch = ctx
                .guestfs
                .inspect_get_arch(&ctx.root)
                .unwrap_or_else(|_| "unknown".to_string());

            println!(
                "{}",
                "Once upon a time, in a datacenter far away...".italic()
            );
            println!();

            println!(
                "This is a {} system, specifically a {} distribution running on {} architecture.",
                os_type.yellow(),
                distro.green(),
                arch.cyan()
            );
            println!();

            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                let pkg_count = pkg_info.packages.len();
                println!("The system has been carefully assembled with {} packages, each serving its purpose",
                    pkg_count.to_string().yellow());
                println!("in the grand tapestry of this computing environment.");
                println!();

                // Identify character
                let web_packages = pkg_info
                    .packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("httpd")
                            || p.name.contains("nginx")
                            || p.name.contains("apache")
                    })
                    .count();

                let db_packages = pkg_info
                    .packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("mysql")
                            || p.name.contains("postgres")
                            || p.name.contains("mariadb")
                    })
                    .count();

                let dev_packages = pkg_info
                    .packages
                    .iter()
                    .filter(|p| {
                        p.name.contains("gcc")
                            || p.name.contains("make")
                            || p.name.contains("python-devel")
                    })
                    .count();

                if web_packages > 0 {
                    println!("This system bears the marks of a {}, with {} web server packages installed.",
                        "web server".green().bold(), web_packages.to_string().yellow());
                    println!("It has likely served countless HTTP requests, delivering content to users worldwide.");
                }

                if db_packages > 0 {
                    println!(
                        "Database packages ({}) suggest this system has been entrusted with {}.",
                        db_packages.to_string().yellow(),
                        "storing precious data".green().bold()
                    );
                    println!("Countless queries have been executed within its digital walls.");
                }

                if dev_packages > 0 {
                    println!("Development tools ({}) indicate this is a {}, where code is crafted and compiled.",
                        dev_packages.to_string().yellow(), "builder's workshop".green().bold());
                }

                if web_packages == 0 && db_packages == 0 && dev_packages == 0 {
                    println!(
                        "This appears to be a {}, lean and purpose-built for specific tasks.",
                        "minimalist system".green().bold()
                    );
                }
                println!();
            }

            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let regular_users: Vec<_> = users
                    .iter()
                    .filter(|u| {
                        if let Ok(uid) = u.uid.parse::<u32>() {
                            uid >= 1000
                        } else {
                            false
                        }
                    })
                    .collect();

                if !regular_users.is_empty() {
                    println!("{} user accounts have called this system home, each leaving their unique imprint.",
                        regular_users.len().to_string().yellow());
                    println!("Their files and configurations tell tales of work accomplished and challenges overcome.");
                } else {
                    println!(
                        "This is a {}, without regular user accounts - a pure service machine.",
                        "sentinel system".green().bold()
                    );
                }
                println!();
            }

            println!(
                "{}",
                "And so our system continues its journey, faithfully executing its duties,"
                    .italic()
            );
            println!(
                "{}",
                "waiting for its next chapter to be written...".italic()
            );
            println!();
        }

        "security" => {
            println!("\n{} {}", "🔒".cyan(), "Security Narrative".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("{}", "A Tale of Protection and Defense".green().bold());
                println!();

                // SELinux story
                if &sec.selinux != "disabled" {
                    println!(
                        "This system is guarded by the watchful eyes of {}, operating in {} mode.",
                        "SELinux".green().bold(),
                        sec.selinux.yellow()
                    );
                    println!("Like a vigilant sentinel, it enforces mandatory access controls,");
                    println!("ensuring that every process stays within its designated boundaries.");
                } else {
                    println!(
                        "SELinux, the guardian of mandatory access controls, {} on this system.",
                        "stands silent".red()
                    );
                    println!("Its protective embrace has been forgone, for better or worse.");
                }
                println!();

                // Firewall story
                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if fw.enabled {
                        println!(
                            "The {} stands as a mighty barrier, filtering network traffic",
                            fw.firewall_type.green().bold()
                        );
                        println!(
                            "with rules carefully crafted to protect against the outside world."
                        );
                    } else {
                        println!(
                            "The firewall gates {}. This system trusts the network around it,",
                            "stand open".red()
                        );
                        println!("or perhaps operates within a protected enclave.");
                    }
                    println!();
                }

                // Audit story
                if sec.auditd {
                    println!(
                        "The {} chronicles every significant event,",
                        "audit daemon".green().bold()
                    );
                    println!("maintaining detailed logs for forensic analysis and compliance.");
                    println!("Nothing escapes its watchful recording.");
                } else {
                    println!(
                        "No audit daemon watches and records. Events pass by {},",
                        "unchronicled".red()
                    );
                    println!("leaving no detailed trail for future investigators.");
                }
                println!();

                println!(
                    "{}",
                    "Thus the security posture is revealed - a balance between".italic()
                );
                println!(
                    "{}",
                    "protection and accessibility, security and convenience.".italic()
                );
                println!();
            }
        }

        "config" => {
            println!(
                "\n{} {}",
                "⚙".cyan(),
                "Configuration Journey".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "The Journey of System Configuration".green().bold());
            println!();

            // Network configuration
            println!("{}", "Chapter 1: Connectivity".yellow());
            if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
                println!("The system was blessed with {} network interfaces, each a gateway to communication.",
                    interfaces.len().to_string().green());
                for iface in interfaces.iter().take(3) {
                    println!("  • {} - a conduit for data flow", iface.name.cyan());
                }
            }
            println!();

            // Storage configuration
            println!("{}", "Chapter 2: Storage".yellow());
            if let Ok(devices) = ctx.guestfs.list_devices() {
                println!(
                    "Storage was provisioned across {} devices, the foundation of persistent data.",
                    devices.len().to_string().green()
                );
            }
            if ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                println!(
                    "The sacred {} defines how these storage realms are mounted,",
                    "/etc/fstab".cyan()
                );
                println!("a map for the system to understand its storage landscape.");
            }
            println!();

            // Services
            println!("{}", "Chapter 3: Services and Daemons".yellow());
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                println!(
                    "Of {} services defined, {} were chosen to run at startup,",
                    services.len().to_string().green(),
                    enabled.to_string().yellow()
                );
                println!("each playing its role in the system's daily operations.");
            }
            println!();

            println!(
                "{}",
                "And thus the system was configured, piece by piece,".italic()
            );
            println!(
                "{}",
                "each setting a deliberate choice in its creation.".italic()
            );
            println!();
        }

        "timeline" => {
            println!("\n{} {}", "⏰".cyan(), "System Timeline".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "A Chronicle of Recent Events".green().bold());
            println!();

            println!("{}", "In recent times...".italic());
            println!();

            // Check /etc modifications
            if let Ok(files) = ctx.guestfs.find("/etc") {
                let etc_files: Vec<_> = files.into_iter().take(5).collect();
                println!("Configuration files in /etc have been touched and modified,");
                println!("administrators shaping the system's behavior through careful edits.");
                for file in etc_files {
                    if ctx.guestfs.is_file(&file).unwrap_or(false) {
                        println!("  • {}", file.bright_black());
                    }
                }
            }
            println!();

            // Check logs
            if ctx.guestfs.is_dir("/var/log").unwrap_or(false) {
                println!(
                    "The {} directory continues to grow, chronicling system events,",
                    "/var/log".cyan()
                );
                println!("errors encountered, and successes achieved.");
                println!("Each log file a diary entry in the system's ongoing story.");
            }
            println!();

            println!(
                "{}",
                "The system's journey continues, writing new chapters daily...".italic()
            );
            println!();
        }

        _ => {
            println!("{} Unknown story topic: {}", "Error:".red(), topic);
            println!("{} story <topic>", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Interactive advisor system
pub fn cmd_advisor(_ctx: &ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║                  System Advisor                          ║"
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

        println!("{}", "Ask the Advisor:".yellow().bold());
        println!("{}", "─".repeat(70).cyan());
        println!();

        let questions = vec![
            ("secure", "How can I improve security?"),
            ("performance", "How can I optimize performance?"),
            ("troubleshoot", "How do I troubleshoot issues?"),
            ("backup", "What backup strategy should I use?"),
            ("monitoring", "How should I monitor this system?"),
            ("upgrade", "How do I plan for upgrades?"),
            ("compliance", "How do I achieve compliance?"),
            ("migration", "How do I prepare for migration?"),
        ];

        for (cmd, question) in questions {
            println!("  {} {}", cmd.green().bold(), question.bright_black());
        }

        println!();
        println!("{} advisor <question>", "Usage:".yellow());
        println!("{} advisor secure", "Example:".cyan());
        println!();
        return Ok(());
    }

    let question = args[0];

    match question {
        "secure" => {
            println!(
                "\n{} {}",
                "🛡".cyan(),
                "Security Improvement Advice".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Step 1: Assess Current State".green().bold());
            println!("  Run: {}", "wizard security".cyan());
            println!("  This gives you a security score and identifies gaps.\n");

            println!("{}", "Step 2: Enable Core Security Features".green().bold());
            println!("  • SELinux or AppArmor - Mandatory access control");
            println!("  • Firewall - Network filtering (iptables/firewalld)");
            println!("  • auditd - Security event logging");
            println!("  Check with: {}\n", "security".cyan());

            println!("{}", "Step 3: Harden User Access".green().bold());
            println!("  • Review user accounts: {}", "users".cyan());
            println!("  • Check sudo privileges: {}", "cat /etc/sudoers".cyan());
            println!("  • Strengthen SSH: {}", "cat /etc/ssh/sshd_config".cyan());
            println!("  • Disable unnecessary accounts\n");

            println!("{}", "Step 4: Minimize Attack Surface".green().bold());
            println!("  • Disable unnecessary services: {}", "services".cyan());
            println!("  • Remove unused packages: {}", "packages".cyan());
            println!("  • Close unused network ports\n");

            println!("{}", "Step 5: Implement Monitoring".green().bold());
            println!("  • Enable intrusion detection (fail2ban, AIDE)");
            println!("  • Set up log monitoring");
            println!("  • Configure alerting\n");

            println!("{}", "Step 6: Validate".green().bold());
            println!("  Run: {}", "scan security".cyan());
            println!("  Then: {}", "recommend".cyan());
            println!();

            println!(
                "{} Use {} for a complete security workflow",
                "💡".yellow(),
                "auto run security-audit".cyan()
            );
            println!();
        }

        "performance" => {
            println!(
                "\n{} {}",
                "⚡".cyan(),
                "Performance Optimization Advice".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Performance Tuning Strategy:".green().bold());
            println!();

            println!("{}", "1. Benchmark Current Performance".yellow());
            println!("  • Run: {}", "bench all".cyan());
            println!("  • Identify bottlenecks\n");

            println!("{}", "2. Optimize Services".yellow());
            println!("  • Review enabled services: {}", "services".cyan());
            println!("  • Disable unnecessary startup services");
            println!("  • Reduce service footprint\n");

            println!("{}", "3. Storage Optimization".yellow());
            println!("  • Review mount options: {}", "cat /etc/fstab".cyan());
            println!("  • Consider: noatime, barrier=0 (if safe)");
            println!("  • Check filesystem type efficiency\n");

            println!("{}", "4. Reduce Package Overhead".yellow());
            println!("  • Remove unused packages: {}", "packages".cyan());
            println!("  • Fewer packages = smaller footprint\n");

            println!("{}", "5. Network Tuning".yellow());
            println!("  • Review network configuration: {}", "network".cyan());
            println!("  • Optimize TCP/IP stack parameters");
            println!("  • Adjust buffer sizes\n");

            println!("{}", "6. Kernel Parameters".yellow());
            println!("  • Review: {}", "inspect kernel".cyan());
            println!("  • Tune /etc/sysctl.conf");
            println!("  • Load only necessary modules\n");

            println!(
                "{} Start with: {}",
                "💡".yellow(),
                "focus performance".cyan()
            );
            println!();
        }

        "troubleshoot" => {
            println!(
                "\n{} {}",
                "🔧".cyan(),
                "Troubleshooting Guide".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Systematic Troubleshooting Approach:".green().bold());
            println!();

            println!("{}", "Phase 1: Gather Information".yellow());
            println!("  • System overview: {}", "dashboard".cyan());
            println!("  • Check health: {}", "wizard health".cyan());
            println!("  • Review configuration: {}", "info".cyan());
            println!();

            println!("{}", "Phase 2: Identify Issues".yellow());
            println!("  • Scan for problems: {}", "scan issues".cyan());
            println!(
                "  • Search error logs: {}",
                "search error --content --path /var/log".cyan()
            );
            println!("  • Review recent changes: {}", "recent /etc 50".cyan());
            println!();

            println!("{}", "Phase 3: Isolate the Problem".yellow());
            println!("  • Focus on specific areas: {}", "focus <aspect>".cyan());
            println!("  • Inspect components: {}", "inspect <component>".cyan());
            println!("  • Check dependencies\n");

            println!("{}", "Phase 4: Research Solution".yellow());
            println!("  • Get recommendations: {}", "recommend".cyan());
            println!("  • Check playbooks: {}", "playbook".cyan());
            println!("  • Use context help: {}", "context".cyan());
            println!();

            println!("{}", "Phase 5: Document Findings".yellow());
            println!(
                "  • Create snapshot: {}",
                "snapshot troubleshooting.md".cyan()
            );
            println!(
                "  • Export evidence: {}",
                "batch export /tmp/evidence".cyan()
            );
            println!();

            println!(
                "{} For systematic investigation: {}",
                "💡".yellow(),
                "playbook forensics".cyan()
            );
            println!();
        }

        "backup" => {
            println!(
                "\n{} {}",
                "💾".cyan(),
                "Backup Strategy Advice".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Comprehensive Backup Strategy:".green().bold());
            println!();

            println!("{}", "1. Document Current State".yellow());
            println!("  • Create snapshot: {}", "snapshot pre-backup.md".cyan());
            println!(
                "  • Export configurations: {}",
                "export system json config.json".cyan()
            );
            println!(
                "  • List packages: {}",
                "export packages csv packages.csv".cyan()
            );
            println!("  • Export users: {}", "export users csv users.csv".cyan());
            println!();

            println!("{}", "2. Identify Critical Data".yellow());
            println!("  • Configuration files in /etc");
            println!("  • User data in /home");
            println!("  • Application data in /var");
            println!("  • Custom scripts and tools\n");

            println!("{}", "3. Backup Key Configurations".yellow());
            println!("  • Network: {}", "cat /etc/hosts /etc/resolv.conf".cyan());
            println!("  • Storage: {}", "cat /etc/fstab".cyan());
            println!("  • Services: {}", "export services md services.md".cyan());
            println!();

            println!("{}", "4. Regular Automation".yellow());
            println!("  • Schedule periodic snapshots");
            println!("  • Automated exports");
            println!("  • Version control for configs\n");

            println!("{}", "5. Test Recovery".yellow());
            println!("  • Verify backup integrity");
            println!("  • Practice restoration");
            println!("  • Document recovery procedures\n");

            println!(
                "{} Quick backup: {}",
                "💡".yellow(),
                "auto run export-all".cyan()
            );
            println!();
        }

        "monitoring" => {
            println!(
                "\n{} {}",
                "📊".cyan(),
                "Monitoring Strategy".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Effective System Monitoring:".green().bold());
            println!();

            println!("{}", "1. Security Monitoring".yellow());
            println!("  • Audit logs: Check auditd status");
            println!("  • Failed logins: Monitor /var/log/auth.log");
            println!("  • File integrity: Use AIDE or similar");
            println!("  • Firewall logs: Review firewall activity\n");

            println!("{}", "2. Performance Monitoring".yellow());
            println!("  • Service health: {}", "services".cyan());
            println!("  • Resource usage: CPU, memory, disk");
            println!("  • Network throughput\n");

            println!("{}", "3. Log Management".yellow());
            println!("  • Centralize logs");
            println!("  • Set retention policies");
            println!("  • Implement log rotation");
            println!("  • Check: {}", "inspect logging".cyan());
            println!();

            println!("{}", "4. Alerting".yellow());
            println!("  • Configure thresholds");
            println!("  • Set up notifications");
            println!("  • Define escalation paths\n");

            println!("{}", "5. Regular Reviews".yellow());
            println!("  • Weekly: {}", "wizard health".cyan());
            println!("  • Monthly: {}", "scan security".cyan());
            println!("  • Quarterly: Full audit\n");

            println!(
                "{} Get current status: {}",
                "💡".yellow(),
                "dashboard".cyan()
            );
            println!();
        }

        "upgrade" => {
            println!(
                "\n{} {}",
                "⬆".cyan(),
                "Upgrade Planning Advice".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Safe Upgrade Strategy:".green().bold());
            println!();

            println!("{}", "Phase 1: Pre-Upgrade Assessment".yellow());
            println!(
                "  • Document current state: {}",
                "snapshot pre-upgrade.md".cyan()
            );
            println!("  • Check compatibility");
            println!("  • Review release notes");
            println!("  • Export packages: {}", "export packages json".cyan());
            println!();

            println!("{}", "Phase 2: Dependency Analysis".yellow());
            println!("  • Review package dependencies");
            println!("  • Check service dependencies: {}", "services".cyan());
            println!("  • Identify potential conflicts\n");

            println!("{}", "Phase 3: Backup Everything".yellow());
            println!("  • Full system backup");
            println!(
                "  • Configuration exports: {}",
                "auto run export-all".cyan()
            );
            println!("  • Test backup restoration\n");

            println!("{}", "Phase 4: Test Upgrade".yellow());
            println!("  • Use test environment first");
            println!("  • Validate functionality");
            println!("  • Performance testing: {}", "bench all".cyan());
            println!();

            println!("{}", "Phase 5: Production Upgrade".yellow());
            println!("  • Schedule maintenance window");
            println!("  • Execute upgrade");
            println!("  • Validate: {}", "wizard health".cyan());
            println!();

            println!("{}", "Phase 6: Post-Upgrade".yellow());
            println!("  • Verify services: {}", "services".cyan());
            println!("  • Check security: {}", "security".cyan());
            println!("  • Create snapshot: {}", "snapshot post-upgrade.md".cyan());
            println!();
        }

        "compliance" => {
            println!(
                "\n{} {}",
                "📋".cyan(),
                "Compliance Achievement Guide".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Path to Compliance:".green().bold());
            println!();

            println!("{}", "1. Understand Requirements".yellow());
            println!("  • Identify applicable standards (PCI-DSS, HIPAA, etc.)");
            println!("  • Document requirements");
            println!("  • Map to controls\n");

            println!("{}", "2. Current State Assessment".yellow());
            println!("  • Run audit checklist: {}", "playbook audit".cyan());
            println!("  • Security assessment: {}", "wizard security".cyan());
            println!("  • Document gaps\n");

            println!("{}", "3. Implement Controls".yellow());
            println!("  • Security hardening: {}", "playbook hardening".cyan());
            println!("  • Access controls: Review {}", "users".cyan());
            println!("  • Audit logging: Enable auditd");
            println!("  • Network security: Configure firewall\n");

            println!("{}", "4. Documentation".yellow());
            println!(
                "  • System documentation: {}",
                "snapshot compliance-docs.md".cyan()
            );
            println!("  • Configuration records");
            println!("  • Change management logs\n");

            println!("{}", "5. Validation".yellow());
            println!("  • Self-assessment: {}", "scan security".cyan());
            println!("  • Generate reports: {}", "report compliance".cyan());
            println!("  • Third-party audit\n");

            println!("{}", "6. Continuous Compliance".yellow());
            println!("  • Regular reviews");
            println!("  • Automated scanning");
            println!("  • Ongoing documentation\n");

            println!("{} Start here: {}", "💡".yellow(), "playbook audit".cyan());
            println!();
        }

        "migration" => {
            println!(
                "\n{} {}",
                "🚀".cyan(),
                "Migration Preparation Guide".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Complete Migration Strategy:".green().bold());
            println!();

            println!("{}", "Step 1: Discovery & Documentation".yellow());
            println!(
                "  • Full system analysis: {}",
                "auto run full-analysis".cyan()
            );
            println!("  • Detect purpose: {}", "profile detect".cyan());
            println!(
                "  • Create baseline: {}",
                "snapshot pre-migration.md".cyan()
            );
            println!();

            println!("{}", "Step 2: Dependency Mapping".yellow());
            println!("  • Services: {}", "export services csv".cyan());
            println!("  • Packages: {}", "export packages csv".cyan());
            println!("  • Network: {}", "discover network".cyan());
            println!("  • Users: {}", "export users csv".cyan());
            println!();

            println!("{}", "Step 3: Configuration Export".yellow());
            println!("  • Export all data: {}", "auto run export-all".cyan());
            println!("  • Document customizations");
            println!("  • Backup critical files\n");

            println!("{}", "Step 4: Planning".yellow());
            println!(
                "  • Use migration playbook: {}",
                "playbook migration".cyan()
            );
            println!("  • Define cutover plan");
            println!("  • Identify risks\n");

            println!("{}", "Step 5: Testing".yellow());
            println!("  • Build target environment");
            println!("  • Migrate test data");
            println!("  • Validate functionality\n");

            println!("{}", "Step 6: Execution & Validation".yellow());
            println!("  • Execute migration");
            println!("  • Post-migration verification");
            println!("  • Performance check: {}", "bench all".cyan());
            println!();

            println!(
                "{} Complete workflow: {}",
                "💡".yellow(),
                "playbook migration".cyan()
            );
            println!();
        }

        _ => {
            println!("{} Unknown question: {}", "Error:".red(), question);
            println!("{} advisor", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// System verification and validation
pub fn cmd_verify(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("\n{}", "Usage: verify <check>".red());
        println!();
        println!("{}", "Available Verifications:".yellow().bold());
        println!("  {} - Verify system integrity", "integrity".green());
        println!("  {} - Verify security configuration", "security".green());
        println!("  {} - Verify boot configuration", "boot".green());
        println!("  {} - Verify network setup", "network".green());
        println!("  {} - Run all verifications", "all".green());
        println!();
        return Ok(());
    }

    let check = args[0];

    match check {
        "integrity" => {
            println!(
                "\n{} {}",
                "✓".cyan(),
                "System Integrity Verification".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            let mut passed = 0;
            let mut failed = 0;
            let mut warnings = 0;

            println!("{}", "Critical System Files:".green().bold());
            let critical_files = vec![
                ("/etc/passwd", "User account database", true),
                ("/etc/shadow", "Password hashes", true),
                ("/etc/group", "Group definitions", true),
                ("/etc/fstab", "Filesystem mount table", true),
                ("/etc/hosts", "Host name resolution", false),
                ("/etc/resolv.conf", "DNS configuration", false),
                ("/boot/grub/grub.cfg", "Boot configuration", false),
                ("/boot/grub2/grub.cfg", "Boot configuration (grub2)", false),
            ];

            for (file, desc, critical) in critical_files {
                if ctx.guestfs.exists(file).unwrap_or(false) {
                    println!(
                        "  {} {} - {}",
                        "✓".green(),
                        file.cyan(),
                        desc.bright_black()
                    );
                    passed += 1;
                } else if critical {
                    println!(
                        "  {} {} - {} {}",
                        "✗".red(),
                        file.cyan(),
                        desc.bright_black(),
                        "[CRITICAL]".red().bold()
                    );
                    failed += 1;
                } else {
                    println!(
                        "  {} {} - {} {}",
                        "⚠".yellow(),
                        file.bright_black(),
                        desc.bright_black(),
                        "[OPTIONAL]".yellow()
                    );
                    warnings += 1;
                }
            }
            println!();

            println!("{}", "Results:".green().bold());
            println!("  Passed:   {}", passed.to_string().green());
            if warnings > 0 {
                println!("  Warnings: {}", warnings.to_string().yellow());
            }
            if failed > 0 {
                println!("  Failed:   {}", failed.to_string().red().bold());
            }
            println!();

            if failed == 0 && warnings == 0 {
                println!(
                    "{} System integrity: {}",
                    "✓".green().bold(),
                    "EXCELLENT".green().bold()
                );
            } else if failed == 0 {
                println!(
                    "{} System integrity: {} ({} warnings)",
                    "✓".green(),
                    "GOOD".green(),
                    warnings.to_string().yellow()
                );
            } else {
                println!(
                    "{} System integrity: {} ({} critical failures)",
                    "✗".red().bold(),
                    "POOR".red().bold(),
                    failed.to_string().red()
                );
            }
            println!();
        }

        "security" => {
            println!(
                "\n{} {}",
                "🔒".cyan(),
                "Security Configuration Verification".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let mut score = 0;
                let mut max_score = 0;

                println!("{}", "Security Features:".green().bold());

                // SELinux
                max_score += 25;
                if &sec.selinux != "disabled" {
                    println!(
                        "  {} SELinux: {} {}",
                        "✓".green(),
                        sec.selinux.green(),
                        "[25 points]".bright_black()
                    );
                    score += 25;
                } else {
                    println!(
                        "  {} SELinux: {} {}",
                        "✗".red(),
                        "disabled".red(),
                        "[0/25 points]".bright_black()
                    );
                }

                // AppArmor
                max_score += 25;
                if sec.apparmor {
                    println!(
                        "  {} AppArmor: {} {}",
                        "✓".green(),
                        "enabled".green(),
                        "[25 points]".bright_black()
                    );
                    score += 25;
                } else {
                    println!(
                        "  {} AppArmor: {} {}",
                        "✗".red(),
                        "disabled".red(),
                        "[0/25 points]".bright_black()
                    );
                }

                // Firewall
                max_score += 25;
                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if fw.enabled {
                        println!(
                            "  {} Firewall: {} ({}) {}",
                            "✓".green(),
                            "enabled".green(),
                            fw.firewall_type,
                            "[25 points]".bright_black()
                        );
                        score += 25;
                    } else {
                        println!(
                            "  {} Firewall: {} {}",
                            "✗".red(),
                            "disabled".red(),
                            "[0/25 points]".bright_black()
                        );
                    }
                }

                // Auditd
                max_score += 25;
                if sec.auditd {
                    println!(
                        "  {} Auditd: {} {}",
                        "✓".green(),
                        "enabled".green(),
                        "[25 points]".bright_black()
                    );
                    score += 25;
                } else {
                    println!(
                        "  {} Auditd: {} {}",
                        "✗".red(),
                        "disabled".red(),
                        "[0/25 points]".bright_black()
                    );
                }

                println!();
                println!("{}", "Security Score:".green().bold());
                println!(
                    "  {}/{} points ({}%)",
                    score.to_string().yellow(),
                    max_score,
                    ((score as f64 / max_score as f64) * 100.0) as i32
                );

                let grade = if score >= 80 {
                    "A (Excellent)".green().bold()
                } else if score >= 60 {
                    "B (Good)".green()
                } else if score >= 40 {
                    "C (Fair)".yellow()
                } else {
                    "D (Poor)".red().bold()
                };

                println!("  Grade: {}", grade);
                println!();

                println!(
                    "{} For detailed security analysis: {}",
                    "💡".yellow(),
                    "wizard security".cyan()
                );
                println!();
            }
        }

        "boot" => {
            println!(
                "\n{} {}",
                "🚀".cyan(),
                "Boot Configuration Verification".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            let mut issues = Vec::new();

            println!("{}", "Boot Components:".green().bold());

            // Check fstab
            if ctx.guestfs.exists("/etc/fstab")? {
                println!("  {} /etc/fstab present", "✓".green());
            } else {
                println!("  {} /etc/fstab missing", "✗".red());
                issues.push("Missing /etc/fstab");
            }

            // Check grub
            let grub_found = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false)
                || ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);

            if grub_found {
                println!("  {} GRUB configuration present", "✓".green());
            } else {
                println!("  {} GRUB configuration not found", "⚠".yellow());
                issues.push("No GRUB configuration found");
            }

            // Check boot directory
            if ctx.guestfs.is_dir("/boot").unwrap_or(false) {
                println!("  {} /boot directory present", "✓".green());
            } else {
                println!("  {} /boot directory missing", "✗".red());
                issues.push("Missing /boot directory");
            }

            println!();

            if issues.is_empty() {
                println!(
                    "{} Boot configuration: {}",
                    "✓".green().bold(),
                    "VALID".green().bold()
                );
            } else {
                println!(
                    "{} Boot configuration: {} ({} issues)",
                    "⚠".yellow(),
                    "WARNING".yellow(),
                    issues.len()
                );
                println!();
                println!("{}", "Issues:".yellow());
                for issue in issues {
                    println!("  • {}", issue.red());
                }
            }
            println!();

            println!(
                "{} For detailed inspection: {}",
                "💡".yellow(),
                "inspect boot".cyan()
            );
            println!();
        }

        "network" => {
            println!(
                "\n{} {}",
                "🌐".cyan(),
                "Network Configuration Verification".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            let mut checks = 0;
            let mut passed = 0;

            println!("{}", "Network Configuration:".green().bold());

            // Check interfaces
            checks += 1;
            if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
                if !interfaces.is_empty() {
                    println!(
                        "  {} {} network interfaces configured",
                        "✓".green(),
                        interfaces.len().to_string().yellow()
                    );
                    passed += 1;
                } else {
                    println!("  {} No network interfaces configured", "⚠".yellow());
                }
            }

            // Check hosts file
            checks += 1;
            if ctx.guestfs.exists("/etc/hosts").unwrap_or(false) {
                println!("  {} /etc/hosts present", "✓".green());
                passed += 1;
            } else {
                println!("  {} /etc/hosts missing", "✗".red());
            }

            // Check DNS
            checks += 1;
            if ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                println!("  {} /etc/resolv.conf present", "✓".green());
                passed += 1;
            } else {
                println!("  {} /etc/resolv.conf missing", "⚠".yellow());
            }

            // Check hostname
            checks += 1;
            if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
                println!(
                    "  {} Hostname configured: {}",
                    "✓".green(),
                    hostname.yellow()
                );
                passed += 1;
            } else {
                println!("  {} Hostname not configured", "⚠".yellow());
            }

            println!();
            println!(
                "{} {}/{} checks passed",
                "Results:".green().bold(),
                passed,
                checks
            );
            println!();

            println!(
                "{} For detailed analysis: {}",
                "💡".yellow(),
                "focus network".cyan()
            );
            println!();
        }

        "all" => {
            println!(
                "\n{} {}",
                "🔍".cyan(),
                "Complete System Verification".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("Running all verification checks...\n");

            println!("{}", "[1/4] Integrity Check".cyan());
            cmd_verify(ctx, &["integrity"])?;

            println!("{}", "[2/4] Security Check".cyan());
            cmd_verify(ctx, &["security"])?;

            println!("{}", "[3/4] Boot Check".cyan());
            cmd_verify(ctx, &["boot"])?;

            println!("{}", "[4/4] Network Check".cyan());
            cmd_verify(ctx, &["network"])?;

            println!("{}", "═".repeat(70).cyan());
            println!(
                "{} Complete system verification finished",
                "✓".green().bold()
            );
            println!();
        }

        _ => {
            println!("{} Unknown verification: {}", "Error:".red(), check);
            println!("{} verify <check>", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Optimization recommendations
pub fn cmd_optimize(_ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Optimization Recommendations                ║"
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

    println!("{}", "System Optimization Guide".yellow().bold());
    println!("{}", "─".repeat(70).cyan());
    println!();

    let categories = vec![
        (
            "Performance",
            vec![
                (
                    "Disable unnecessary services",
                    "services -> disable unused",
                    "Medium",
                ),
                (
                    "Remove unused packages",
                    "packages -> uninstall unused",
                    "Low",
                ),
                (
                    "Optimize mount options",
                    "cat /etc/fstab -> add noatime",
                    "Medium",
                ),
                ("Tune kernel parameters", "/etc/sysctl.conf tuning", "High"),
            ],
        ),
        (
            "Security",
            vec![
                (
                    "Enable SELinux/AppArmor",
                    "Mandatory access control",
                    "High",
                ),
                ("Configure firewall", "Network filtering", "High"),
                ("Enable audit logging", "auditd configuration", "Medium"),
                ("Harden SSH", "/etc/ssh/sshd_config", "Medium"),
            ],
        ),
        (
            "Storage",
            vec![
                ("Clean log files", "Log rotation and cleanup", "Low"),
                ("Remove old kernels", "Keep only recent kernels", "Low"),
                ("Optimize filesystem", "Choice of fs type", "Medium"),
            ],
        ),
        (
            "Network",
            vec![
                ("Optimize TCP/IP stack", "sysctl network tuning", "Medium"),
                ("Configure DNS properly", "/etc/resolv.conf", "Low"),
                ("Use connection pooling", "For applications", "Medium"),
            ],
        ),
    ];

    for (category, optimizations) in categories {
        println!("{}", format!("{}:", category).green().bold());
        for (name, action, impact) in optimizations {
            let impact_colored = match impact {
                "High" => impact.red().bold(),
                "Medium" => impact.yellow(),
                _ => impact.green(),
            };
            println!(
                "  {} {} - {} [{}]",
                "•".cyan(),
                name,
                action.bright_black(),
                impact_colored
            );
        }
        println!();
    }

    println!("{}", "Getting Started:".yellow().bold());
    println!("  • {} - Performance analysis", "focus performance".cyan());
    println!("  • {} - Security improvements", "advisor secure".cyan());
    println!(
        "  • {} - Full system analysis",
        "auto run full-analysis".cyan()
    );
    println!();

    Ok(())
}

/// Improvement roadmap generator
pub fn cmd_roadmap(_ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    let timeframe = if args.is_empty() { "30-day" } else { args[0] };

    println!(
        "\n{} {}",
        "🗺".cyan(),
        format!("{} Improvement Roadmap", timeframe.to_uppercase())
            .yellow()
            .bold()
    );
    println!("{}", "═".repeat(70).cyan());
    println!();

    match timeframe {
        "30-day" | "short" => {
            println!("{} (Priority: Quick Wins)", "30-Day Roadmap".green().bold());
            println!();

            println!("{} Week 1: Assessment", "📅".yellow());
            println!("  • Run: {}", "auto run full-analysis".cyan());
            println!("  • Run: {}", "wizard security".cyan());
            println!("  • Run: {}", "wizard health".cyan());
            println!("  • Document baseline: {}", "snapshot baseline.md".cyan());
            println!();

            println!("{} Week 2: Quick Security Fixes", "📅".yellow());
            println!("  • Enable missing security features");
            println!("  • Remove unnecessary user accounts: {}", "users".cyan());
            println!("  • Disable unused services: {}", "services".cyan());
            println!("  • Verify: {}", "verify security".cyan());
            println!();

            println!("{} Week 3: Performance Tuning", "📅".yellow());
            println!("  • Benchmark: {}", "bench all".cyan());
            println!("  • Remove unused packages: {}", "packages".cyan());
            println!("  • Optimize startup services");
            println!("  • Test improvements");
            println!();

            println!("{} Week 4: Documentation & Validation", "📅".yellow());
            println!(
                "  • Create documentation: {}",
                "auto run documentation".cyan()
            );
            println!("  • Run all verifications: {}", "verify all".cyan());
            println!("  • Generate reports: {}", "report executive".cyan());
            println!("  • Archive baseline for future comparison");
            println!();
        }

        "90-day" | "medium" => {
            println!(
                "{} (Priority: Substantial Improvements)",
                "90-Day Roadmap".green().bold()
            );
            println!();

            println!("{} Month 1: Foundation", "📅".yellow());
            println!("  • Complete 30-day roadmap");
            println!("  • Establish monitoring");
            println!("  • Implement backup strategy: {}", "advisor backup".cyan());
            println!();

            println!("{} Month 2: Security Hardening", "📅".yellow());
            println!(
                "  • Follow hardening playbook: {}",
                "playbook hardening".cyan()
            );
            println!("  • Implement intrusion detection");
            println!("  • Configure log centralization");
            println!("  • Security scan: {}", "scan security".cyan());
            println!();

            println!("{} Month 3: Optimization & Compliance", "📅".yellow());
            println!(
                "  • Performance optimization: {}",
                "advisor performance".cyan()
            );
            println!("  • Compliance assessment: {}", "playbook audit".cyan());
            println!("  • Automated monitoring setup");
            println!("  • Final validation: {}", "verify all".cyan());
            println!();
        }

        "annual" | "long" => {
            println!(
                "{} (Priority: Strategic Transformation)",
                "Annual Roadmap".green().bold()
            );
            println!();

            println!("{} Q1: Assessment & Planning", "📅".yellow());
            println!("  • Complete current state analysis");
            println!("  • Define target state");
            println!("  • Create detailed project plan");
            println!("  • Stakeholder alignment");
            println!();

            println!("{} Q2: Security & Compliance", "📅".yellow());
            println!("  • Complete security hardening");
            println!("  • Achieve compliance: {}", "advisor compliance".cyan());
            println!("  • Implement monitoring");
            println!("  • Staff training");
            println!();

            println!("{} Q3: Optimization & Automation", "📅".yellow());
            println!("  • Performance optimization");
            println!("  • Automation implementation");
            println!("  • Disaster recovery setup");
            println!("  • Documentation: {}", "auto run documentation".cyan());
            println!();

            println!("{} Q4: Migration & Modernization", "📅".yellow());
            println!("  • Migration planning: {}", "playbook migration".cyan());
            println!("  • Infrastructure modernization");
            println!("  • Continuous improvement process");
            println!("  • Year-end review and reporting");
            println!();
        }

        _ => {
            println!("{} Unknown timeframe: {}", "Error:".red(), timeframe);
            println!(
                "{}",
                "Available timeframes: 30-day, 90-day, annual".yellow()
            );
            return Ok(());
        }
    }

    println!("{}", "Key Success Metrics:".green().bold());
    println!(
        "  • Security score improvement: Track with {}",
        "wizard security".cyan()
    );
    println!(
        "  • Health score improvement: Track with {}",
        "wizard health".cyan()
    );
    println!("  • Performance gains: Measure with {}", "bench all".cyan());
    println!("  • Compliance status: Verify with {}", "verify all".cyan());
    println!();

    println!("{} Start now: {}", "💡".yellow(), "verify all".cyan());
    println!();

    Ok(())
}

/// AI-like intelligent insights
pub fn cmd_goals(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!(
            "\n{}",
            "╔═══════════════════════════════════════════════════════════╗"
                .cyan()
                .bold()
        );
        println!(
            "{}",
            "║              System Improvement Goals                    ║"
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

        println!("{}", "Track your system improvement journey!".yellow());
        println!();

        println!("{}", "Available Commands:".green().bold());
        println!("  {} - Show suggested goals", "goals suggest".cyan());
        println!("  {} - Set a custom goal", "goals set <name>".cyan());
        println!("  {} - List all goals", "goals list".cyan());
        println!("  {} - Check goal status", "goals check <name>".cyan());
        println!();

        println!("{}", "Example Goals:".yellow());
        println!("  • Achieve security score of 80+");
        println!("  • Reduce enabled services by 20%");
        println!("  • Remove 100+ unused packages");
        println!("  • Enable all security features");
        println!("  • Document all configurations");
        println!();

        return Ok(());
    }

    let subcommand = args[0];

    match subcommand {
        "suggest" => {
            println!("\n{} {}", "🎯".cyan(), "Suggested Goals".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Based on current system state:".green().bold());
            println!();

            // Security goals
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let mut security_goals = 0;

                if &sec.selinux == "disabled" {
                    security_goals += 1;
                    println!("{} {} Enable SELinux", "1.".yellow(), "🔒".cyan());
                    println!("   Target: Activate mandatory access control");
                    println!("   Command: Check /etc/selinux/config");
                    println!();
                }

                if !sec.auditd {
                    security_goals += 1;
                    println!(
                        "{} {} Enable Audit Daemon",
                        format!("{}.", security_goals + 1).yellow(),
                        "📝".cyan()
                    );
                    println!("   Target: Start security event logging");
                    println!("   Verify: Run 'security' command");
                    println!();
                }

                if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
                    if !fw.enabled {
                        security_goals += 1;
                        println!(
                            "{} {} Enable Firewall",
                            format!("{}.", security_goals + 1).yellow(),
                            "🛡️".cyan()
                        );
                        println!("   Target: Configure network filtering");
                        println!("   Verify: Run 'verify security'");
                        println!();
                    }
                }
            }

            // Performance goals
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                if enabled > 50 {
                    println!("{} {} Optimize Services", "4.".yellow(), "⚙️".cyan());
                    println!("   Target: Reduce enabled services to <40");
                    println!("   Current: {} enabled", enabled);
                    println!("   Command: 'services' to review");
                    println!();
                }
            }

            // Documentation goals
            println!("{} {} Complete Documentation", "5.".yellow(), "📚".cyan());
            println!("   Target: Generate comprehensive system documentation");
            println!("   Command: 'auto run documentation'");
            println!();

            println!(
                "{} Use {} to track progress",
                "💡".yellow(),
                "verify all".cyan()
            );
            println!();
        }

        "list" => {
            println!("\n{} {}", "📋".cyan(), "Goal Tracking".yellow().bold());
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Common Goals:".green().bold());

            let goals = [
                (
                    "Security Excellence",
                    "Achieve security score 90+",
                    "verify security",
                ),
                (
                    "Performance Optimization",
                    "Reduce service count by 25%",
                    "services",
                ),
                (
                    "Compliance Ready",
                    "Pass all audit checks",
                    "playbook audit",
                ),
                (
                    "Documentation Complete",
                    "Full system documentation",
                    "auto run documentation",
                ),
                ("Zero Critical Issues", "No critical findings", "doctor"),
            ];

            for (i, (name, target, cmd)) in goals.iter().enumerate() {
                println!("{}. {} {}", i + 1, name.bold(), "🎯".cyan());
                println!("   Target: {}", target);
                println!("   Check: {}", cmd.bright_black());
                println!();
            }

            println!(
                "{} Run commands to check progress towards your goals",
                "💡".yellow()
            );
            println!();
        }

        "check" => {
            if args.len() < 2 {
                println!("{} Usage: goals check <goal-name>", "Error:".red());
                return Ok(());
            }

            let goal = args[1];
            println!(
                "\n{} {}",
                "🎯".cyan(),
                format!("Checking Goal: {}", goal).yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            match goal {
                "security" => {
                    println!("Running security verification...");
                    println!();
                    cmd_verify(ctx, &["security"])?;
                }
                "health" => {
                    println!("Running health diagnostic...");
                    println!();
                    cmd_doctor(ctx, &[])?;
                }
                "services" => {
                    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                        let enabled = services.iter().filter(|s| s.enabled).count();
                        println!("{}", "Service Optimization Goal:".green().bold());
                        println!("  Current: {} enabled services", enabled);
                        println!("  Target:  <40 enabled services");

                        if enabled < 40 {
                            println!("  Status:  {} Goal achieved!", "✓".green().bold());
                        } else {
                            println!(
                                "  Status:  {} In progress ({} to remove)",
                                "→".yellow(),
                                enabled - 40
                            );
                        }
                        println!();
                    }
                }
                _ => {
                    println!("{} Unknown goal: {}", "Error:".red(), goal);
                    println!("Use {} to see available goals", "goals list".cyan());
                }
            }
        }

        _ => {
            println!("{} Unknown subcommand: {}", "Error:".red(), subcommand);
            println!("{} goals", "Usage:".yellow());
        }
    }

    Ok(())
}

/// Shell usage analysis and habits
pub fn cmd_habits(ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Shell Usage Analysis                        ║"
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

    println!("{}", "Session Statistics:".green().bold());
    println!("{}", "─".repeat(70).cyan());
    println!();

    println!(
        "  Commands executed: {}",
        ctx.command_count.to_string().yellow()
    );

    if let Some(duration) = ctx.last_command_time {
        println!(
            "  Last command time: {} ms",
            format!("{:.2}", duration.as_secs_f64() * 1000.0).yellow()
        );
    }

    println!("  Current directory: {}", ctx.current_path.cyan());
    println!(
        "  Active aliases:    {}",
        ctx.aliases.len().to_string().yellow()
    );
    println!(
        "  Bookmarks saved:   {}",
        ctx.bookmarks.len().to_string().yellow()
    );
    println!();

    println!("{}", "Usage Patterns:".green().bold());
    println!("{}", "─".repeat(70).cyan());
    println!();

    // Analyze usage patterns
    if ctx.command_count < 5 {
        println!("{} {}", "🌱".cyan(), "Getting Started".bold());
        println!("  You're just beginning your exploration. Try these commands:");
        println!("  • {} - Learn the basics", "learn basics".cyan());
        println!("  • {} - See available commands", "help".cyan());
        println!("  • {} - Get an overview", "dashboard".cyan());
    } else if ctx.command_count < 20 {
        println!("{} {}", "🔍".cyan(), "Active Explorer".bold());
        println!("  You're actively exploring the system. Enhance your workflow:");
        println!("  • {} - Create shortcuts", "alias".cyan());
        println!("  • {} - Save favorite locations", "bookmark".cyan());
        println!("  • {} - Learn advanced features", "learn advanced".cyan());
    } else {
        println!("{} {}", "⭐".cyan(), "Power User".bold());
        println!("  Excellent engagement! Take advantage of advanced features:");
        println!("  • {} - Automate workflows", "auto run <preset>".cyan());
        println!("  • {} - Advanced searches", "search".cyan());
        println!("  • {} - Batch operations", "batch".cyan());
    }
    println!();

    println!("{}", "Efficiency Tips:".yellow().bold());
    println!("{}", "─".repeat(70).cyan());
    println!();

    let tips = vec![
        ("Use Tab completion", "Faster command entry"),
        ("Create aliases", "Shortcut frequently used commands"),
        ("Bookmark paths", "Quick navigation with 'goto'"),
        ("Use 'quick' menu", "Fast access to common actions"),
        ("Try 'context' command", "Get location-specific suggestions"),
    ];

    for (tip, benefit) in tips {
        println!(
            "  {} {} - {}",
            "💡".yellow(),
            tip.bold(),
            benefit.bright_black()
        );
    }
    println!();

    println!("{}", "Recommended Next Steps:".green().bold());
    println!("{}", "─".repeat(70).cyan());
    println!();

    if ctx.bookmarks.is_empty() {
        println!(
            "  {} Create bookmarks for frequently visited paths",
            "1.".yellow()
        );
        println!("     Command: {}", "bookmark myspot".cyan());
    }

    if ctx.aliases.len() <= 5 {
        println!(
            "  {} Set up custom aliases for your workflow",
            "2.".yellow()
        );
        println!("     Command: {}", "alias shortname 'full command'".cyan());
    }

    println!("  {} Try automation presets", "3.".yellow());
    println!("     Command: {}", "auto run full-analysis".cyan());
    println!();

    Ok(())
}

/// Team collaboration report generator
pub fn cmd_collaborate(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("\n{}", "Usage: collaborate <report-type>".red());
        println!();
        println!("{}", "Available Report Types:".yellow().bold());
        println!(
            "  {} - Handoff report for team transition",
            "handoff".green()
        );
        println!(
            "  {} - Incident report for security team",
            "incident".green()
        );
        println!("  {} - Change request documentation", "change".green());
        println!("  {} - Weekly status report", "status".green());
        println!();
        return Ok(());
    }

    let report_type = args[0];

    match report_type {
        "handoff" => {
            println!(
                "\n{} {}",
                "👥".cyan(),
                "Team Handoff Report".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "Generating team handoff documentation...".yellow());
            println!();

            println!("{}", "## System Handoff Report".green().bold());
            println!();

            // Current timestamp
            let now = chrono::Local::now();
            println!("**Generated:** {}", now.format("%Y-%m-%d %H:%M:%S"));
            println!("**Inspector:** GuestKit Interactive Shell");
            println!();

            println!("{}", "### System Overview".yellow());
            if let Ok(os_type) = ctx.guestfs.inspect_get_type(&ctx.root) {
                println!("- **OS Type:** {}", os_type);
            }
            if let Ok(distro) = ctx.guestfs.inspect_get_distro(&ctx.root) {
                println!("- **Distribution:** {}", distro);
            }
            if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
                println!("- **Architecture:** {}", arch);
            }
            println!();

            println!("{}", "### Key Information".yellow());
            if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
                println!("- **Total Packages:** {}", pkg_info.packages.len());
            }
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                println!("- **User Accounts:** {}", users.len());
            }
            if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
                let enabled = services.iter().filter(|s| s.enabled).count();
                println!(
                    "- **Services:** {} total, {} enabled",
                    services.len(),
                    enabled
                );
            }
            println!();

            println!("{}", "### Security Status".yellow());
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("- **SELinux:** {}", sec.selinux);
                println!(
                    "- **AppArmor:** {}",
                    if sec.apparmor { "enabled" } else { "disabled" }
                );
                println!(
                    "- **Auditd:** {}",
                    if sec.auditd { "enabled" } else { "disabled" }
                );
            }
            println!();

            println!("{}", "### Recommendations for Incoming Team".yellow());
            println!("1. Run `dashboard` for quick overview");
            println!("2. Run `verify all` to check system health");
            println!("3. Review `security` status");
            println!("4. Check `services` for running daemons");
            println!("5. Use `learn basics` for shell orientation");
            println!();

            println!("{}", "### Critical Files to Review".yellow());
            println!("- /etc/fstab - Filesystem mounts");
            println!("- /etc/hosts - Network configuration");
            println!("- /etc/ssh/sshd_config - SSH settings");
            println!();

            println!(
                "{} Save this report: {}",
                "💡".yellow(),
                "snapshot handoff-report.md".cyan()
            );
            println!();
        }

        "incident" => {
            println!(
                "\n{} {}",
                "🚨".cyan(),
                "Security Incident Report".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "## Security Incident Report".green().bold());
            println!();

            let now = chrono::Local::now();
            println!("**Report Date:** {}", now.format("%Y-%m-%d %H:%M:%S"));
            println!("**System:** {}", ctx.root);
            println!("**Reporter:** GuestKit Analysis Tool");
            println!();

            println!("{}", "### Incident Summary".yellow());
            println!("*[To be filled by investigator]*");
            println!();

            println!("{}", "### System State at Time of Incident".yellow());

            println!("\n**Security Configuration:**");
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                println!("- SELinux: {}", sec.selinux);
                println!(
                    "- AppArmor: {}",
                    if sec.apparmor { "Active" } else { "Inactive" }
                );
                println!(
                    "- Audit Daemon: {}",
                    if sec.auditd { "Running" } else { "Not running" }
                );
            }

            println!("\n**Active Users:**");
            if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
                let regular = users
                    .iter()
                    .filter(|u| u.uid.parse::<u32>().map(|uid| uid >= 1000).unwrap_or(false))
                    .count();
                println!("- {} regular user accounts", regular);
                println!("- Run 'users' for complete list");
            }

            println!();
            println!("{}", "### Evidence Collection".yellow());
            println!("The following data should be preserved:");
            println!(
                "1. Complete snapshot: `snapshot incident-{}.md`",
                now.format("%Y%m%d-%H%M%S")
            );
            println!("2. Security logs: `recent /var/log 100`");
            println!("3. User activity: `users`");
            println!("4. Service status: `services`");
            println!();

            println!("{}", "### Recommended Actions".yellow());
            println!("1. Run `playbook incident` for investigation steps");
            println!("2. Use `search <indicator> --content --path /var/log` for log analysis");
            println!("3. Export evidence: `batch export /tmp/incident-evidence`");
            println!("4. Generate forensics report: `report security`");
            println!();
        }

        "change" => {
            println!(
                "\n{} {}",
                "📝".cyan(),
                "Change Request Documentation".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!(
                "{}",
                "## Change Request - System Modification".green().bold()
            );
            println!();

            println!("**Date:** {}", chrono::Local::now().format("%Y-%m-%d"));
            println!("**System:** {}", ctx.root);
            println!("**Prepared by:** GuestKit Shell");
            println!();

            println!("{}", "### Current State Baseline".yellow());
            println!("*Pre-change system snapshot*");
            println!();
            println!("```");
            println!("Command: snapshot pre-change-baseline.md");
            println!("```");
            println!();

            println!("{}", "### Proposed Changes".yellow());
            println!("*[Describe changes to be made]*");
            println!();

            println!("{}", "### Risk Assessment".yellow());
            println!("**Impact Level:** *[Low/Medium/High]*");
            println!("**Affected Components:** *[List components]*");
            println!("**Rollback Plan:** *[Describe rollback procedure]*");
            println!();

            println!("{}", "### Testing Plan".yellow());
            println!("1. Pre-change verification: `verify all`");
            println!("2. Implement changes");
            println!("3. Post-change verification: `verify all`");
            println!("4. Performance check: `bench all`");
            println!("5. Health assessment: `doctor`");
            println!();

            println!("{}", "### Approval".yellow());
            println!("**Requested by:** ___________");
            println!("**Approved by:** ___________");
            println!("**Date:** ___________");
            println!();
        }

        "status" => {
            println!(
                "\n{} {}",
                "📊".cyan(),
                "Weekly Status Report".yellow().bold()
            );
            println!("{}", "═".repeat(70).cyan());
            println!();

            println!("{}", "## Weekly System Status".green().bold());
            println!();

            let now = chrono::Local::now();
            println!("**Report Period:** Week of {}", now.format("%Y-%m-%d"));
            println!();

            println!("{}", "### System Health".yellow());
            println!("Run `doctor` for comprehensive health check");
            println!();

            println!("{}", "### Security Status".yellow());
            if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
                let features = vec![
                    ("SELinux", &sec.selinux != "disabled"),
                    ("AppArmor", sec.apparmor),
                    ("Auditd", sec.auditd),
                ];

                let active = features.iter().filter(|(_, enabled)| *enabled).count();
                println!("**Security Features Active:** {}/3", active);

                for (name, enabled) in features {
                    let status = if enabled { "✓" } else { "✗" };
                    println!("  {} {}", status, name);
                }
            }
            println!();

            println!("{}", "### Activity Summary".yellow());
            println!("- Shell sessions: {}", ctx.command_count);
            println!("- Commands executed: {}", ctx.command_count);
            println!("- Bookmarks created: {}", ctx.bookmarks.len());
            println!();

            println!("{}", "### Recommendations".yellow());
            println!("1. Run monthly security audit: `auto run security-audit`");
            println!("2. Update system documentation: `auto run documentation`");
            println!("3. Review service status: `services`");
            println!();

            println!("{}", "### Next Week Goals".yellow());
            println!("Use `goals suggest` to set improvement targets");
            println!();
        }

        _ => {
            println!("{} Unknown report type: {}", "Error:".red(), report_type);
            println!("{} collaborate <report-type>", "Usage:".yellow());
            return Ok(());
        }
    }

    Ok(())
}

/// Predictive analysis for potential issues
pub fn cmd_template(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║              Command Template System                     ║"
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
        println!("{}", "Available Templates:".yellow().bold());
        println!();
        println!(
            "{} {} - Full security audit",
            "1.".cyan(),
            "security-audit".green()
        );
        println!(
            "{} {} - System health check",
            "2.".cyan(),
            "health-check".green()
        );
        println!(
            "{} {} - Compliance review",
            "3.".cyan(),
            "compliance-review".green()
        );
        println!(
            "{} {} - Performance analysis",
            "4.".cyan(),
            "performance-analysis".green()
        );
        println!("{} {} - Export all data", "5.".cyan(), "export-all".green());
        println!(
            "{} {} - Pre-migration check",
            "6.".cyan(),
            "pre-migration".green()
        );
        println!();
        println!("{} template <name>", "Usage:".yellow());
        return Ok(());
    }

    let template_name = args[0];

    match template_name {
        "security-audit" => {
            println!("{}", "🔒 Running Security Audit Template".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} Phase 1: Security Configuration", "→".cyan());
            cmd_verify(ctx, &["security"])?;
            println!();

            println!("{} Phase 2: Vulnerability Predictions", "→".cyan());
            cmd_predict(ctx, &[])?;
            println!();

            println!("{} Phase 3: Compliance Checks", "→".cyan());
            cmd_compliance(ctx, &["cis"])?;
            println!();

            println!("{} Phase 4: Security Insights", "→".cyan());
            cmd_insights(ctx, &[])?;
            println!();

            println!("{}", "✓ Security Audit Complete".green().bold());
            println!("  Review the findings above and address critical issues first.");
        }

        "health-check" => {
            println!("{}", "🏥 Running Health Check Template".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} Phase 1: System Doctor", "→".cyan());
            cmd_doctor(ctx, &[])?;
            println!();

            println!("{} Phase 2: Verification Suite", "→".cyan());
            cmd_verify(ctx, &["all"])?;
            println!();

            println!("{} Phase 3: Intelligent Insights", "→".cyan());
            cmd_insights(ctx, &[])?;
            println!();

            println!("{}", "✓ Health Check Complete".green().bold());
        }

        "compliance-review" => {
            println!("{}", "📋 Running Compliance Review Template".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} CIS Benchmark", "→".cyan());
            cmd_compliance(ctx, &["cis"])?;
            println!();

            println!("{} PCI-DSS", "→".cyan());
            cmd_compliance(ctx, &["pci-dss"])?;
            println!();

            println!("{} HIPAA", "→".cyan());
            cmd_compliance(ctx, &["hipaa"])?;
            println!();

            println!("{}", "✓ Compliance Review Complete".green().bold());
        }

        "performance-analysis" => {
            println!(
                "{}",
                "⚡ Running Performance Analysis Template".cyan().bold()
            );
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} Phase 1: Package Analysis", "→".cyan());
            cmd_chart(ctx, &["packages"])?;
            println!();

            println!("{} Phase 2: Service Analysis", "→".cyan());
            cmd_chart(ctx, &["services"])?;
            println!();

            println!("{} Phase 3: Optimization Recommendations", "→".cyan());
            cmd_optimize(ctx, &[])?;
            println!();

            println!("{}", "✓ Performance Analysis Complete".green().bold());
        }

        "export-all" => {
            println!("{}", "💾 Running Export All Template".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} Generating comprehensive snapshot...", "→".cyan());
            cmd_snapshot(ctx, &[])?;
            println!();

            println!("{}", "✓ Export Complete".green().bold());
        }

        "pre-migration" => {
            println!(
                "{}",
                "🚀 Running Pre-Migration Check Template".cyan().bold()
            );
            println!("{}", "━".repeat(60).bright_black());
            println!();

            println!("{} Phase 1: System Verification", "→".cyan());
            cmd_verify(ctx, &["all"])?;
            println!();

            println!("{} Phase 2: Production Readiness", "→".cyan());
            cmd_compare(ctx, &["production"])?;
            println!();

            println!("{} Phase 3: Issue Predictions", "→".cyan());
            cmd_predict(ctx, &[])?;
            println!();

            println!("{} Phase 4: Data Export", "→".cyan());
            cmd_snapshot(ctx, &[])?;
            println!();

            println!("{}", "✓ Pre-Migration Check Complete".green().bold());
            println!("  Address any critical issues before migration.");
        }

        _ => {
            println!("{} Unknown template: {}", "Error:".red(), template_name);
            println!(
                "{} template (without arguments) to see available templates",
                "Tip:".yellow()
            );
        }
    }

    println!();
    Ok(())
}

/// Comprehensive system scoring across multiple dimensions
pub fn cmd_migrate(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║           Migration Readiness Assessment                ║"
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

    let target = if args.is_empty() { "cloud" } else { args[0] };

    match target {
        "cloud" => {
            println!("{}", "☁️  Cloud Migration Readiness".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let mut ready = 0;
            let mut warnings = Vec::new();
            let mut blockers = Vec::new();

            // Check 1: Boot configuration
            println!("{} Checking boot configuration...", "→".cyan());
            if ctx.guestfs.exists("/etc/fstab").unwrap_or(false) {
                let grub_found = ctx.guestfs.exists("/boot/grub/grub.cfg").unwrap_or(false)
                    || ctx.guestfs.exists("/boot/grub2/grub.cfg").unwrap_or(false);
                if grub_found {
                    println!("  {} Boot configuration is valid", "✓".green());
                    ready += 1;
                } else {
                    println!("  {} Missing boot loader configuration", "✗".red());
                    blockers.push("Configure GRUB boot loader");
                }
            } else {
                println!("  {} Missing /etc/fstab", "✗".red());
                blockers.push("Create /etc/fstab");
            }

            // Check 2: Network configuration
            println!("{} Checking network configuration...", "→".cyan());
            if ctx.guestfs.exists("/etc/resolv.conf").unwrap_or(false) {
                println!("  {} DNS configuration exists", "✓".green());
                ready += 1;
            } else {
                println!("  {} Missing DNS configuration", "⚠".yellow());
                warnings.push("Configure /etc/resolv.conf");
            }

            // Check 3: Cloud-init support
            println!("{} Checking cloud-init support...", "→".cyan());
            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let has_cloud_init = pkg_info
                .packages
                .iter()
                .any(|p| p.name.contains("cloud-init"));
            if has_cloud_init {
                println!("  {} cloud-init is installed", "✓".green());
                ready += 1;
            } else {
                println!("  {} cloud-init not found", "⚠".yellow());
                warnings.push("Install cloud-init for better cloud integration");
            }

            // Check 4: SSH server
            println!("{} Checking SSH server...", "→".cyan());
            if ctx.guestfs.exists("/etc/ssh/sshd_config").unwrap_or(false) {
                println!("  {} SSH server configured", "✓".green());
                ready += 1;
            } else {
                println!("  {} SSH server not configured", "⚠".yellow());
                warnings.push("Configure SSH server for remote access");
            }

            // Check 5: Security features
            println!("{} Checking security features...", "→".cyan());
            let sec = ctx.guestfs.inspect_security(&ctx.root)?;
            if &sec.selinux != "disabled" || sec.apparmor {
                println!("  {} MAC system is active", "✓".green());
                ready += 1;
            } else {
                println!("  {} No MAC system active", "⚠".yellow());
                warnings.push("Consider enabling SELinux or AppArmor");
            }

            println!();
            println!("{}", "═".repeat(60).bright_black());
            println!();

            println!(
                "{} Migration Readiness: {}/5 checks passed",
                "📊".cyan(),
                ready.to_string().cyan().bold()
            );
            println!();

            if !blockers.is_empty() {
                println!("{} Critical Blockers:", "🚫".red().bold());
                for (i, blocker) in blockers.iter().enumerate() {
                    println!("  {}. {}", i + 1, blocker.red());
                }
                println!();
            }

            if !warnings.is_empty() {
                println!("{} Recommendations:", "⚠".yellow().bold());
                for (i, warning) in warnings.iter().enumerate() {
                    println!("  {}. {}", i + 1, warning.yellow());
                }
                println!();
            }

            if blockers.is_empty() && ready >= 4 {
                println!(
                    "{}",
                    "✓ System is ready for cloud migration!".green().bold()
                );
                println!();
                println!("{} Next Steps:", "💡".cyan());
                println!("  1. Create final snapshot: snapshot pre-migration.md");
                println!("  2. Run template pre-migration for comprehensive check");
                println!("  3. Export system data: export system json system-config.json");
                println!("  4. Review and test boot configuration");
            } else if blockers.is_empty() {
                println!(
                    "{}",
                    "⚠ System is mostly ready but has some warnings.".yellow()
                );
                println!("  Address recommendations above for best results.");
            } else {
                println!(
                    "{}",
                    "✗ System has critical blockers preventing migration.".red()
                );
                println!("  Resolve blockers before attempting migration.");
            }
        }

        "container" => {
            println!("{}", "🐳 Container Migration Assessment".cyan().bold());
            println!("{}", "━".repeat(60).bright_black());
            println!();

            let pkg_info = ctx.guestfs.inspect_packages(&ctx.root)?;
            let pkg_count = pkg_info.packages.len();

            println!("{} Package Analysis:", "→".cyan());
            println!("  Total packages: {}", pkg_count.to_string().cyan());

            if pkg_count < 300 {
                println!(
                    "  {} Suitable for containerization (minimal footprint)",
                    "✓".green()
                );
            } else if pkg_count < 600 {
                println!(
                    "  {} Can be containerized (consider reducing packages)",
                    "⚠".yellow()
                );
            } else {
                println!(
                    "  {} Large package count (strongly consider reduction)",
                    "⚠".yellow()
                );
            }

            println!();
            println!("{} Recommendations:", "💡".yellow());
            println!("  • Identify essential packages only");
            println!("  • Create multi-stage Dockerfile");
            println!("  • Use minimal base images (Alpine, distroless)");
            println!("  • Extract application dependencies");
        }

        _ => {
            println!("{}", "Migration Targets:".yellow().bold());
            println!();
            println!(
                "{} {} - Cloud platform migration (AWS, Azure, GCP)",
                "1.".cyan(),
                "migrate cloud".green()
            );
            println!(
                "{} {} - Container migration assessment",
                "2.".cyan(),
                "migrate container".green()
            );
            println!();
            println!("{} migrate <target>", "Usage:".yellow());
        }
    }

    println!();
    Ok(())
}
