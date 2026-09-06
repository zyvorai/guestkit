// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! REPL (Read-Eval-Print Loop) for interactive shell

use anyhow::{Context, Result};
use colored::Colorize;
use rustyline::error::ReadlineError;
use rustyline::DefaultEditor;
use std::path::Path;

use super::commands::{self, ShellContext};
use crate::Guestfs;

/// Run interactive shell
pub fn run_interactive_shell<P: AsRef<Path>>(image_path: P) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        format!(
            "║          GuestKit Interactive Shell v{:<19} ║",
            crate::VERSION
        )
        .cyan()
        .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
    println!("{} Loading VM image...", "→".cyan());

    // Initialize guestfs
    let mut guestfs = Guestfs::new().context("Failed to create Guestfs handle")?;
    guestfs
        .add_drive_opts(
            image_path
                .as_ref()
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Disk image path contains invalid UTF-8"))?,
            false,
            None,
        )
        .context("Failed to add drive")?;
    guestfs.launch().context("Failed to launch guestfs")?;

    // Inspect and mount
    let roots = guestfs.inspect_os().context("Failed to inspect OS")?;
    if roots.is_empty() {
        anyhow::bail!("No operating systems found");
    }

    let root = &roots[0];
    println!("{} Detected OS: {}", "→".cyan(), root.yellow());

    let mounts_map = guestfs
        .inspect_get_mountpoints(root)
        .context("Failed to get mountpoints")?;

    // Sort by mountpoint path length (shortest first) to ensure parents are mounted before children
    let mut mounts: Vec<_> = mounts_map.into_iter().collect();
    mounts.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (mountpoint, device) in mounts {
        if let Err(e) = guestfs.mount(&device, &mountpoint) {
            eprintln!("{} Failed to mount {}: {}", "⚠".yellow(), mountpoint, e);
        }
    }

    println!("{} VM filesystem mounted successfully", "✓".green());
    println!();
    println!("{} for available commands", "Type 'help'".yellow().bold());
    println!("{} to exit", "Type 'exit' or press Ctrl+D".yellow().bold());
    println!();

    // Create shell context
    let mut ctx = ShellContext::new(guestfs, root.to_string());

    // Get OS information for context
    let os_product = ctx
        .guestfs
        .inspect_get_product_name(root)
        .unwrap_or_else(|_| "Unknown OS".to_string());
    ctx.set_os_info(os_product);

    // Create readline editor with history
    let mut rl = DefaultEditor::new()?;

    // Load history if exists
    let history_path = dirs::home_dir()
        .map(|p| p.join(".guestkit_history"))
        .unwrap_or_else(|| std::path::PathBuf::from(".guestkit_history"));

    if let Err(e) = rl.load_history(&history_path) {
        // ReadlineError::Io with NotFound is expected on first run
        if !matches!(&e, ReadlineError::Io(io_err) if io_err.kind() == std::io::ErrorKind::NotFound)
        {
            eprintln!("{} Failed to load history: {}", "Warning:".yellow(), e);
        }
    }

    // REPL loop
    loop {
        // Enhanced prompt showing OS and path
        let prompt = format!(
            "[{}] {}{}> ",
            ctx.get_os_info().cyan(),
            ctx.current_path.yellow(),
            "".clear()
        );

        match rl.readline(&prompt) {
            Ok(line) => {
                let line = line.trim();

                if line.is_empty() {
                    continue;
                }

                // Add to history
                let _ = rl.add_history_entry(line);

                // Parse command - use owned strings to avoid lifetime issues
                let mut line_owned = line.to_string();

                // Check for alias expansion first
                let parts: Vec<&str> = line_owned.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let cmd_str = parts[0];

                // Expand alias if exists
                if let Some(alias_expansion) = ctx.get_alias(cmd_str).cloned() {
                    // Replace command with alias and keep arguments
                    let alias_parts: Vec<String> = alias_expansion
                        .split_whitespace()
                        .map(|s| s.to_string())
                        .collect();
                    if !alias_parts.is_empty() {
                        let mut expanded_parts = alias_parts;
                        for arg in &parts[1..] {
                            expanded_parts.push(arg.to_string());
                        }
                        line_owned = expanded_parts.join(" ");
                        println!("{} {}", "→".cyan(), line_owned.yellow());
                    }
                }

                // Re-parse the (possibly expanded) line
                let parts: Vec<&str> = line_owned.split_whitespace().collect();
                if parts.is_empty() {
                    continue;
                }

                let cmd = parts[0];
                let args = &parts[1..];

                // Start timing (without borrowing ctx mutably)
                let start = std::time::Instant::now();

                // Execute command
                let result = match cmd {
                    "exit" | "quit" | "q" => {
                        println!("{} Shutting down...", "→".cyan());
                        break;
                    }
                    "help" | "?" => commands::cmd_help(&ctx, args),
                    "clear" | "cls" => {
                        print!("\x1B[2J\x1B[1;1H");
                        Ok(())
                    }
                    "history" => {
                        for (i, entry) in rl.history().iter().enumerate() {
                            println!("{:4}  {}", i + 1, entry);
                        }
                        Ok(())
                    }
                    "ls" => commands::cmd_ls(&mut ctx, args),
                    "cat" => commands::cmd_cat(&mut ctx, args),
                    "cd" => commands::cmd_cd(&mut ctx, args),
                    "pwd" => commands::cmd_pwd(&ctx, args),
                    "find" => commands::cmd_find(&mut ctx, args),
                    "grep" => commands::cmd_grep(&mut ctx, args),
                    "info" => commands::cmd_info(&mut ctx, args),
                    "mounts" => commands::cmd_mounts(&mut ctx, args),
                    "packages" => {
                        cmd_packages(&mut ctx, args);
                        Ok(())
                    }
                    "services" => {
                        cmd_services(&mut ctx, args);
                        Ok(())
                    }
                    "users" => {
                        cmd_users(&mut ctx);
                        Ok(())
                    }
                    "network" => {
                        cmd_network(&mut ctx);
                        Ok(())
                    }
                    "security" => {
                        cmd_security(&mut ctx);
                        Ok(())
                    }
                    "health" => {
                        cmd_health(&mut ctx);
                        Ok(())
                    }
                    "risks" => {
                        cmd_risks(&mut ctx);
                        Ok(())
                    }
                    "ai" => {
                        let _ = commands::cmd_ai(&mut ctx, args);
                        Ok(())
                    }
                    "alias" => commands::cmd_alias(&mut ctx, args),
                    "unalias" => commands::cmd_unalias(&mut ctx, args),
                    "bookmark" | "bm" => commands::cmd_bookmark(&mut ctx, args),
                    "goto" => commands::cmd_goto(&mut ctx, args),
                    "stats" => commands::cmd_stats(&ctx, args),
                    "dashboard" | "dash" => commands::cmd_dashboard(&mut ctx, args),
                    "export" => commands::cmd_export(&mut ctx, args),
                    "tree" => commands::cmd_tree(&mut ctx, args),
                    "summary" | "sum" => commands::cmd_summary(&mut ctx, args),
                    "tips" | "tip" => commands::cmd_tips(&ctx, args),
                    "snapshot" | "snap" => commands::cmd_snapshot(&mut ctx, args),
                    "diff" => commands::cmd_diff(&mut ctx, args),
                    "recent" => commands::cmd_recent(&mut ctx, args),
                    "quick" => commands::cmd_quick(&mut ctx, args),
                    "cheat" => commands::cmd_cheat(&ctx, args),
                    "search" => commands::cmd_search(&mut ctx, args),
                    "watch" => commands::cmd_watch(&mut ctx, args),
                    "batch" => commands::cmd_batch(&mut ctx, args),
                    "pin" => commands::cmd_pin(&mut ctx, args),
                    "h" => commands::cmd_history_enhanced(&ctx, args),
                    "wizard" | "wiz" => commands::cmd_wizard(&mut ctx, args),
                    "scan" => commands::cmd_scan(&mut ctx, args),
                    "compare" | "cmp" => commands::cmd_compare(&mut ctx, args),
                    "profile" => commands::cmd_profile(&mut ctx, args),
                    "recommend" | "rec" => commands::cmd_recommend(&mut ctx, args),
                    "discover" | "disco" => commands::cmd_discover(&mut ctx, args),
                    "explore" | "ex" => commands::cmd_explore(&mut ctx, args),
                    "report" => commands::cmd_report(&mut ctx, args),
                    "auto" => commands::cmd_auto(&mut ctx, args),
                    "menu" => commands::cmd_menu(&mut ctx, args),
                    "timeline" => commands::cmd_timeline(&mut ctx, args),
                    "bench" => commands::cmd_bench(&mut ctx, args),
                    "presets" => commands::cmd_presets(&ctx, args),
                    "context" => commands::cmd_context(&mut ctx, args),
                    "learn" => commands::cmd_learn(&ctx, args),
                    "focus" => commands::cmd_focus(&mut ctx, args),
                    "playbook" => commands::cmd_playbook(&mut ctx, args),
                    "inspect" => commands::cmd_inspect(&mut ctx, args),
                    "story" => commands::cmd_story(&mut ctx, args),
                    "advisor" => commands::cmd_advisor(&ctx, args),
                    "verify" => commands::cmd_verify(&mut ctx, args),
                    "optimize" => commands::cmd_optimize(&ctx, args),
                    "roadmap" => commands::cmd_roadmap(&mut ctx, args),
                    "insights" => commands::cmd_insights(&mut ctx, args),
                    "doctor" => commands::cmd_doctor(&mut ctx, args),
                    "goals" => commands::cmd_goals(&mut ctx, args),
                    "habits" => commands::cmd_habits(&ctx, args),
                    "collaborate" => commands::cmd_collaborate(&mut ctx, args),
                    "predict" => commands::cmd_predict(&mut ctx, args),
                    "chart" => commands::cmd_chart(&mut ctx, args),
                    "compliance" => commands::cmd_compliance(&mut ctx, args),
                    "template" => commands::cmd_template(&mut ctx, args),
                    "score" => commands::cmd_score(&mut ctx, args),
                    "query" => commands::cmd_query(&mut ctx, args),
                    "monitor" => commands::cmd_monitor(&mut ctx, args),
                    "migrate" => commands::cmd_migrate(&mut ctx, args),
                    "troubleshoot" => commands::cmd_troubleshoot(&mut ctx, args),
                    "depends" => commands::cmd_depends(&mut ctx, args),
                    "validate" => commands::cmd_validate(&mut ctx, args),
                    "forensics" => commands::cmd_forensics(&mut ctx, args),
                    "audit" => commands::cmd_audit(&mut ctx, args),
                    "baseline" => commands::cmd_baseline(&mut ctx, args),
                    _ => {
                        eprintln!(
                            "{} Unknown command: {}. Type 'help' for available commands.",
                            "Error:".red(),
                            cmd
                        );
                        Ok(())
                    }
                };

                // End timing and show duration if command took >100ms
                ctx.end_timing(start);
                if let Some(duration) = ctx.last_command_time {
                    if duration.as_millis() > 100 {
                        println!(
                            "{} Command completed in {}",
                            "⏱".cyan(),
                            format!("{:.2}ms", duration.as_secs_f64() * 1000.0).yellow()
                        );
                    }
                }

                // Check for errors
                if let Err(e) = result {
                    eprintln!("{} {}", "Error:".red(), e);
                }
            }
            Err(ReadlineError::Interrupted) => {
                println!("{} Use 'exit' to quit", "^C".yellow());
            }
            Err(ReadlineError::Eof) => {
                println!();
                println!("{} Shutting down...", "→".cyan());
                break;
            }
            Err(err) => {
                eprintln!("{} {}", "Error:".red(), err);
                break;
            }
        }
    }

    // Save history
    if let Err(e) = rl.save_history(&history_path) {
        eprintln!("{} Failed to save history: {}", "Warning:".yellow(), e);
    }

    // Shutdown
    ctx.guestfs.shutdown()?;
    println!("{} Goodbye!", "✓".green());

    Ok(())
}

// Enhanced command implementations with analysis

fn cmd_packages(ctx: &mut ShellContext, args: &[&str]) {
    println!("{} Analyzing packages...", "→".cyan());

    if let Ok(pkg_info) = ctx.guestfs.inspect_packages(&ctx.root) {
        let packages = &pkg_info.packages;
        let filtered: Vec<_> = if args.is_empty() {
            packages.iter().take(20).collect()
        } else {
            let pattern = args[0].to_lowercase();
            packages
                .iter()
                .filter(|p| p.name.to_lowercase().contains(&pattern))
                .collect()
        };

        println!(
            "\n{} ({} shown, {} total)",
            "Installed Packages".yellow().bold(),
            filtered.len(),
            packages.len()
        );
        println!("{}", "─".repeat(60).cyan());

        for pkg in filtered {
            println!(
                "{:30} {}",
                pkg.name.green(),
                pkg.version.to_string().bright_black()
            );
        }

        if args.is_empty() && packages.len() > 20 {
            println!("\n{} Use 'packages <pattern>' to search", "Tip:".yellow());
        }
        println!();
    }
}

fn cmd_services(ctx: &mut ShellContext, args: &[&str]) {
    println!("{} Analyzing services...", "→".cyan());

    if let Ok(services) = ctx.guestfs.inspect_systemd_services(&ctx.root) {
        let filtered: Vec<_> = if args.is_empty() {
            services.iter().collect()
        } else {
            let pattern = args[0].to_lowercase();
            services
                .iter()
                .filter(|s| s.name.to_lowercase().contains(&pattern))
                .collect()
        };

        println!(
            "\n{} ({} shown)",
            "System Services".yellow().bold(),
            filtered.len()
        );
        println!("{}", "─".repeat(60).cyan());

        for svc in filtered {
            let status = if svc.enabled {
                "enabled".green()
            } else {
                "disabled".bright_black()
            };
            println!("{:40} {}", svc.name, status);
        }
        println!();
    }
}

fn cmd_users(ctx: &mut ShellContext) {
    println!("{} Analyzing user accounts...", "→".cyan());

    if let Ok(users) = ctx.guestfs.inspect_users(&ctx.root) {
        println!(
            "\n{} ({} total)",
            "User Accounts".yellow().bold(),
            users.len()
        );
        println!("{}", "─".repeat(80).cyan());
        println!(
            "{:20} {:10} {:10} {:30}",
            "Username".bold(),
            "UID".bold(),
            "GID".bold(),
            "Home".bold()
        );
        println!("{}", "─".repeat(80).cyan());

        for user in users {
            let uid_color = if user.uid == "0" {
                user.uid.red()
            } else {
                user.uid.normal()
            };

            println!(
                "{:20} {:10} {:10} {}",
                user.username.green(),
                uid_color,
                user.gid.bright_black(),
                user.home.bright_black()
            );
        }
        println!();
    }
}

fn cmd_network(ctx: &mut ShellContext) {
    println!("{} Analyzing network configuration...", "→".cyan());

    if let Ok(interfaces) = ctx.guestfs.inspect_network(&ctx.root) {
        println!(
            "\n{} ({} total)",
            "Network Interfaces".yellow().bold(),
            interfaces.len()
        );
        println!("{}", "─".repeat(70).cyan());

        for iface in interfaces {
            println!("\n{}", iface.name.green().bold());
            // Note: NetworkInterface struct may have different fields
            // Adjust based on actual struct definition
        }

        if let Ok(dns) = ctx.guestfs.inspect_dns(&ctx.root) {
            if !dns.is_empty() {
                println!("\n{}", "DNS Servers".yellow().bold());
                for server in dns {
                    println!("  • {}", server.cyan());
                }
            }
        }
        println!();
    }
}

fn cmd_security(ctx: &mut ShellContext) {
    println!("{} Analyzing security configuration...", "→".cyan());

    if let Ok(sec) = ctx.guestfs.inspect_security(&ctx.root) {
        println!("\n{}", "Security Features".yellow().bold());
        println!("{}", "─".repeat(60).cyan());

        let selinux_status = if &sec.selinux != "disabled" {
            sec.selinux.green()
        } else {
            sec.selinux.red()
        };
        println!("  SELinux:  {}", selinux_status);

        let apparmor = if sec.apparmor {
            "enabled".green()
        } else {
            "disabled".red()
        };
        println!("  AppArmor: {}", apparmor);

        let fail2ban = if sec.fail2ban {
            "installed".green()
        } else {
            "not installed".bright_black()
        };
        println!("  fail2ban: {}", fail2ban);

        let aide = if sec.aide {
            "installed".green()
        } else {
            "not installed".bright_black()
        };
        println!("  AIDE:     {}", aide);

        let auditd = if sec.auditd {
            "enabled".green()
        } else {
            "disabled".red()
        };
        println!("  auditd:   {}", auditd);

        if let Ok(fw) = ctx.guestfs.inspect_firewall(&ctx.root) {
            let fw_status = if fw.enabled {
                format!("{} ({})", "enabled".green(), fw.firewall_type)
            } else {
                format!("{} ({})", "disabled".red(), fw.firewall_type)
            };
            println!("  Firewall: {}", fw_status);
        }

        println!();
    }
}

fn cmd_health(_ctx: &mut ShellContext) {
    println!("\n{}", "System Health Score".yellow().bold());
    println!("{}", "─".repeat(60).cyan());
    println!(
        "\n{} This command requires full TUI mode.",
        "Note:".yellow()
    );
    println!(
        "{} Run '{}' for complete health analysis",
        "Tip:".cyan(),
        crate::cli::invocation::example("tui <image>")
    );
    println!();
}

fn cmd_risks(_ctx: &mut ShellContext) {
    println!("\n{}", "Security Risks".yellow().bold());
    println!("{}", "─".repeat(60).cyan());
    println!(
        "\n{} This command requires full TUI mode.",
        "Note:".yellow()
    );
    println!(
        "{} Run '{}' to view security issues",
        "Tip:".cyan(),
        crate::cli::invocation::example("tui <image>")
    );
    println!();
}
