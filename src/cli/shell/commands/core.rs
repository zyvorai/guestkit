// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Basic shell commands: ls, cat, cd, pwd, find, grep, info, mounts, help, tree

use anyhow::Result;
use colored::Colorize;

use super::{resolve_path, ShellContext};

/// List files in current or specified directory
pub fn cmd_ls(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    let path = if args.is_empty() {
        &ctx.current_path
    } else {
        args[0]
    };

    let full_path = resolve_path(&ctx.current_path, path);

    match ctx.guestfs.ls(&full_path) {
        Ok(entries) => {
            for entry in entries {
                // Try to get file type
                let entry_path = std::path::Path::new(full_path.trim_end_matches('/'))
                    .join(&entry)
                    .to_string_lossy()
                    .to_string();
                let is_dir = ctx.guestfs.is_dir(&entry_path).unwrap_or(false);

                if is_dir {
                    println!("{}/", entry.blue().bold());
                } else {
                    println!("{}", entry);
                }
            }
            Ok(())
        }
        Err(e) => {
            // Check if it's a file (common mistake: ls on a file instead of cat)
            if ctx.guestfs.is_file(&full_path).unwrap_or(false) {
                eprintln!(
                    "{} '{}' is a file, not a directory",
                    "Error:".red(),
                    full_path
                );
                eprintln!(
                    "{} Use 'cat {}' to view the file contents",
                    "Hint:".yellow(),
                    full_path
                );
            } else {
                eprintln!("{} {}", "Error:".red(), e);
            }
            Ok(())
        }
    }
}

/// Show file contents
pub fn cmd_cat(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        eprintln!("{} cat <file>", "Usage:".yellow());
        return Ok(());
    }

    let path = resolve_path(&ctx.current_path, args[0]);

    match ctx.guestfs.read_file(&path) {
        Ok(contents) => {
            print!("{}", String::from_utf8_lossy(&contents));
            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red(), e);
            Ok(())
        }
    }
}

/// Change directory
pub fn cmd_cd(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    let path = if args.is_empty() { "/" } else { args[0] };

    let new_path = resolve_path(&ctx.current_path, path);

    // Verify directory exists
    if ctx.guestfs.is_dir(&new_path).unwrap_or(false) {
        ctx.current_path = new_path;
        Ok(())
    } else {
        eprintln!("{} Not a directory: {}", "Error:".red(), new_path);
        Ok(())
    }
}

/// Print working directory
pub fn cmd_pwd(ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!("{}", ctx.current_path);
    Ok(())
}

/// Find files matching pattern
pub fn cmd_find(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        eprintln!("{} find <pattern>", "Usage:".yellow());
        return Ok(());
    }

    let pattern = args[0];
    let search_path = if args.len() > 1 {
        resolve_path(&ctx.current_path, args[1])
    } else {
        ctx.current_path.clone()
    };

    println!(
        "{} files matching '{}' from '{}'...",
        "Searching".cyan(),
        pattern,
        search_path
    );

    // Recursive search implementation
    search_recursive(ctx, &search_path, pattern, 0)?;

    Ok(())
}

pub fn search_recursive(
    ctx: &mut ShellContext,
    path: &str,
    pattern: &str,
    depth: usize,
) -> Result<()> {
    use std::collections::HashSet;
    search_recursive_inner(ctx, path, pattern, depth, &mut HashSet::new())
}

fn search_recursive_inner(
    ctx: &mut ShellContext,
    path: &str,
    pattern: &str,
    depth: usize,
    visited: &mut std::collections::HashSet<String>,
) -> Result<()> {
    if depth > 50 {
        return Ok(()); // Limit recursion depth to prevent stack overflow
    }

    // Guard against symlink loops by tracking visited paths
    if !visited.insert(path.to_string()) {
        return Ok(());
    }

    if let Ok(entries) = ctx.guestfs.ls(path) {
        for entry in entries {
            let full_path = std::path::Path::new(path.trim_end_matches('/'))
                .join(&entry)
                .to_string_lossy()
                .to_string();
            if entry.contains(pattern) {
                println!("{}", full_path.green());
            }

            if ctx.guestfs.is_dir(&full_path).unwrap_or(false) && entry != "." && entry != ".." {
                if let Err(e) = search_recursive_inner(ctx, &full_path, pattern, depth + 1, visited)
                {
                    log::warn!("Search error in {}: {}", full_path, e);
                }
            }
        }
    }

    Ok(())
}

/// Search file contents
pub fn cmd_grep(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.len() < 2 {
        eprintln!("{} grep <pattern> <file>", "Usage:".yellow());
        return Ok(());
    }

    let pattern = args[0];
    let path = resolve_path(&ctx.current_path, args[1]);

    match ctx.guestfs.read_file(&path) {
        Ok(contents) => {
            let text = String::from_utf8_lossy(&contents);
            for (i, line) in text.lines().enumerate() {
                if line.contains(pattern) {
                    println!(
                        "{}:{}",
                        format!("{}", i + 1).cyan(),
                        line.replace(pattern, &pattern.red().to_string())
                    );
                }
            }
            Ok(())
        }
        Err(e) => {
            eprintln!("{} {}", "Error:".red(), e);
            Ok(())
        }
    }
}

/// Show system information
pub fn cmd_info(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!("\n{}", "=== System Information ===".cyan().bold());

    if let Ok(os) = ctx.guestfs.inspect_get_type(&ctx.root) {
        println!("{} {}", "OS Type:".yellow(), os);
    }

    if let Ok(distro) = ctx.guestfs.inspect_get_distro(&ctx.root) {
        println!("{} {}", "Distribution:".yellow(), distro);
    }

    if let Ok(version) = ctx.guestfs.inspect_get_major_version(&ctx.root) {
        println!("{} {}", "Major Version:".yellow(), version);
    }

    if let Ok(hostname) = ctx.guestfs.inspect_get_hostname(&ctx.root) {
        println!("{} {}", "Hostname:".yellow(), hostname);
    }

    if let Ok(arch) = ctx.guestfs.inspect_get_arch(&ctx.root) {
        println!("{} {}", "Architecture:".yellow(), arch);
    }

    println!();
    Ok(())
}

/// Show mounted filesystems
pub fn cmd_mounts(ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    println!("\n{}", "=== Mounted Filesystems ===".cyan().bold());

    if let Ok(mounts) = ctx.guestfs.mounts() {
        for mount in mounts {
            println!("{}", mount.green());
        }
    }

    println!();
    Ok(())
}

/// Show help
pub fn cmd_help(_ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!("\n{}", "=== GuestKit Interactive Shell ===".cyan().bold());
    println!("\n{}", "File System Commands:".yellow().bold());
    println!("  {}  - List directory contents", "ls [path]".green());
    println!("  {}  - Show file contents", "cat <file>".green());
    println!("  {}  - Change directory", "cd <path>".green());
    println!("  {}     - Print working directory", "pwd".green());
    println!(
        "  {}  - Find files by name",
        "find <pattern> [path]".green()
    );
    println!("  {} - Search in file", "grep <pattern> <file>".green());

    println!("\n{}", "System Commands:".yellow().bold());
    println!("  {}    - Show system information", "info".green());
    println!("  {}  - Show mounted filesystems", "mounts".green());
    println!(
        "  {} - Search installed packages",
        "packages <pattern>".green()
    );
    println!("  {} - List system services", "services [pattern]".green());
    println!("  {}   - List user accounts", "users".green());
    println!("  {} - Show network configuration", "network".green());

    println!("\n{}", "Analysis Commands:".yellow().bold());
    println!("  {} - Show security status", "security".green());
    println!("  {}  - Show system health score", "health".green());
    println!("  {}   - Show security risks", "risks".green());

    println!("\n{}", "Interactive Navigation:".yellow().bold());
    println!(
        "  {} - Interactive file explorer (TUI)",
        "explore, ex [path]".green()
    );
    println!("           Visual file browser with preview & actions");
    println!("           Use ↑↓/j/k to navigate, Enter to open, h for help");

    println!("\n{}", "Overview & Visualization:".yellow().bold());
    println!(
        "  {} - Beautiful system dashboard",
        "dashboard, dash".green()
    );
    println!("  {} - Quick system summary", "summary, sum".green());
    println!(
        "  {} - Visualize directory tree",
        "tree [path] [depth]".green()
    );
    println!("  {}     - Random helpful tip", "tips, tip".green());

    println!("\n{}", "Data Export & Reporting:".yellow().bold());
    println!(
        "  {} - Export data in various formats",
        "export <type> <format> [file]".green()
    );
    println!("           Types: packages, users, services, system");
    println!("           Formats: json, csv, md, txt");
    println!("           Example: export packages json packages.json");
    println!(
        "  {} - Generate comprehensive snapshot report",
        "snapshot, snap [file]".green()
    );
    println!("           Creates detailed Markdown report");
    println!("           Example: snapshot system-report.md");
    println!(
        "  {}      - Compare and analyze",
        "diff <type> <filter>".green()
    );
    println!("           Example: diff package kernel");

    #[cfg(feature = "ai")]
    {
        println!("\n{}", "AI Assistant:".yellow().bold());
        println!(
            "  {}    - Ask AI for help (requires OPENAI_API_KEY)",
            "ai <query>".green()
        );
        println!("           Example: ai why won't this boot?");
    }

    println!("\n{}", "Intelligence & Discovery:".yellow().bold());
    println!(
        "  {} - Smart recommendations engine",
        "recommend, rec".green()
    );
    println!(
        "  {} - System profiling & detection",
        "profile <type>".green()
    );
    println!("           Types: create, quick, detect, show");
    println!(
        "  {} - Automatic system discovery",
        "discover, disco <type>".green()
    );
    println!("           Types: files, apps, network, all");
    println!(
        "  {}  - Formatted report generator",
        "report <type>".green()
    );
    println!("           Types: executive, technical, security, compliance");

    println!("\n{}", "Guided Workflows:".yellow().bold());
    println!(
        "  {} - Interactive task wizards",
        "wizard, wiz <type>".green()
    );
    println!("           Types: security, health, packages, config, export");
    println!("  {}    - System scanners", "scan <type>".green());
    println!("           Types: security, issues, vulns, all");
    println!(
        "  {} - File/directory comparison",
        "compare, cmp <type>".green()
    );
    println!("           Types: files, dirs");

    println!("\n{}", "Advanced Features:".yellow().bold());
    println!(
        "  {} - Smart search with filters",
        "search <pattern> [options]".green()
    );
    println!("           Options: --path, --type, --content");
    println!("  {}   - Batch operations", "batch <operation>".green());
    println!("           Operations: cat, find, export");
    println!("  {}   - Watch files/directories", "watch <path>".green());
    println!("  {}     - Pin favorite commands", "pin [command]".green());

    println!("\n{}", "Quick Commands:".yellow().bold());
    println!("  {}   - Quick actions menu", "quick".green());
    println!("  {}   - Command cheat sheet", "cheat".green());
    println!(
        "  {}  - Recently modified files",
        "recent [path] [limit]".green()
    );
    println!("  {}       - Enhanced history analysis", "h".green());

    println!("\n{}", "Automation & Utilities:".yellow().bold());
    println!("  {}    - Automation presets", "auto <preset>".green());
    println!("           Presets: security-audit, full-analysis, health-check, export-all, documentation");
    println!("  {}    - Interactive command menu", "menu".green());
    println!("  {} - Session timeline visualization", "timeline".green());
    println!("  {}   - Performance benchmarking", "bench <type>".green());
    println!("           Types: files, list, packages, all");
    println!("  {} - Role-based command presets", "presets".green());

    println!("\n{}", "Learning & Guidance:".yellow().bold());
    println!("  {} - Interactive tutorials", "learn <tutorial>".green());
    println!("           Tutorials: basics, navigation, security, export, advanced, automation");
    println!("  {} - Context-aware suggestions", "context".green());
    println!("  {} - Focus on specific aspects", "focus <aspect>".green());
    println!("           Aspects: security, performance, network, storage, users");
    println!("  {} - Operational playbooks", "playbook <name>".green());
    println!("           Playbooks: incident, hardening, audit, forensics, migration, recovery");
    println!(
        "  {} - Deep component inspection",
        "inspect <component>".green()
    );
    println!("           Components: boot, logging, packages, services, kernel");

    println!("\n{}", "Planning & Strategy:".yellow().bold());
    println!(
        "  {} - Narrative system explanations",
        "story <topic>".green()
    );
    println!("           Topics: system, security, config, timeline");
    println!(
        "  {} - Interactive advisor Q&A",
        "advisor <question>".green()
    );
    println!("           Questions: secure, performance, troubleshoot, backup, monitoring, upgrade, compliance, migration");
    println!(
        "  {} - System verification checks",
        "verify <check>".green()
    );
    println!("           Checks: integrity, security, boot, network, all");
    println!("  {} - Optimization recommendations", "optimize".green());
    println!("  {} - Improvement roadmaps", "roadmap <timeframe>".green());
    println!("           Timeframes: 30-day, 90-day, annual");

    println!("\n{}", "Intelligence & Analytics:".yellow().bold());
    println!("  {} - AI-like intelligent insights", "insights".green());
    println!("  {}  - Comprehensive health diagnostic", "doctor".green());
    println!(
        "  {}   - Goal setting and tracking",
        "goals <command>".green()
    );
    println!("           Commands: suggest, list, check");
    println!("  {}  - Shell usage analysis", "habits".green());
    println!(
        "  {} - Team collaboration reports",
        "collaborate <type>".green()
    );
    println!("           Types: handoff, incident, change, status");

    println!(
        "\n{}",
        "Advanced Analytics & Visualization:".yellow().bold()
    );
    println!("  {}  - Predictive issue analysis", "predict".green());
    println!("  {}   - Data visualization charts", "chart <type>".green());
    println!("           Types: packages, users, services, storage, security");
    println!(
        "  {} - Compliance checking",
        "compliance <standard>".green()
    );
    println!("           Standards: cis, pci-dss, hipaa, gdpr, soc2");

    println!("\n{}", "Automation & Operations:".yellow().bold());
    println!("  {} - Command template system", "template <name>".green());
    println!("           Templates: security-audit, health-check, compliance-review");
    println!("           performance-analysis, export-all, pre-migration");
    println!("  {}   - Comprehensive system scoring", "score".green());
    println!(
        "  {}   - SQL-like query interface",
        "query <statement>".green()
    );
    println!(
        "  {} - System monitoring & alerts",
        "monitor <type>".green()
    );
    println!("           Types: security, health, changes, alerts");
    println!(
        "  {} - Migration readiness assessment",
        "migrate <target>".green()
    );
    println!("           Targets: cloud, container");

    println!("\n{}", "Diagnostics & Remediation:".yellow().bold());
    println!(
        "  {} - Intelligent troubleshooting",
        "troubleshoot <category>".green()
    );
    println!("           Categories: boot, network, services, performance, security, auto");
    println!(
        "  {} - Package dependency analysis",
        "depends <command>".green()
    );
    println!("           Commands: search, analyze, dev, libs");
    println!(
        "  {} - Configuration validation",
        "validate <target>".green()
    );
    println!("           Targets: all, config");

    println!("\n{}", "Security & Forensics:".yellow().bold());
    println!(
        "  {} - Digital forensics workflows",
        "forensics <workflow>".green()
    );
    println!("           Workflows: collect, timeline, suspicious, activity, integrity, memory");
    println!(
        "  {} - Security audit trail analysis",
        "audit <type>".green()
    );
    println!("           Types: auth, users, config, packages, sudo, full");
    println!(
        "  {} - Security baseline management",
        "baseline <command>".green()
    );
    println!("           Commands: create, show, drift, cis, export");

    println!("\n{}", "Shell Commands:".yellow().bold());
    println!("  {}    - Show this help", "help".green());
    println!("  {}   - Clear screen", "clear".green());
    println!("  {}   - Show command history", "history".green());
    println!("  {}    - Show shell statistics", "stats".green());
    println!("  {}    - Exit shell", "exit, quit, q".green());

    println!("\n{}", "Aliases & Bookmarks:".yellow().bold());
    println!("  {} - List all aliases", "alias".green());
    println!("  {} - Create an alias", "alias <name> <command>".green());
    println!("  {} - Remove an alias", "unalias <name>".green());
    println!("  {} - List bookmarks", "bookmark".green());
    println!("  {} - Bookmark current path", "bookmark <name>".green());
    println!(
        "  {} - Bookmark specific path",
        "bookmark <name> <path>".green()
    );
    println!("  {} - Jump to bookmark", "goto <name>".green());

    println!("\n{}", "Default Aliases:".yellow().bold());
    println!("  {} - Same as: ls -l", "ll".cyan());
    println!("  {} - Same as: ls -a", "la".cyan());
    println!("  {} - Same as: cd ..  ", "..".cyan());
    println!("  {}  - Same as: cd /   ", "~".cyan());
    println!("  {}  - Same as: quit  ", "q".cyan());

    println!("\n{}", "Tips:".yellow().bold());
    println!("  • Use {} for command completion", "Tab".cyan());
    println!("  • Use {} for command history", "↑/↓ arrows".cyan());
    println!("  • Paths are relative to current directory");
    println!("  • Commands taking >100ms show execution time");
    println!("  • Prompt shows: {}", "[OS] /current/path>".yellow());
    println!();

    Ok(())
}

/// Show directory tree
pub fn cmd_tree(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    let path = if args.is_empty() {
        ctx.current_path.clone()
    } else {
        resolve_path(&ctx.current_path, args[0])
    };

    let max_depth = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(3)
    } else {
        3
    };

    println!("\n{} {}", "Tree view of:".yellow().bold(), path.cyan());
    println!();

    print_tree(ctx, &path, "", 0, max_depth)?;
    println!();

    Ok(())
}

pub fn print_tree(
    ctx: &mut ShellContext,
    path: &str,
    prefix: &str,
    depth: usize,
    max_depth: usize,
) -> Result<()> {
    if depth >= max_depth {
        return Ok(());
    }

    let entries = match ctx.guestfs.ls(path) {
        Ok(e) => e,
        Err(_) => return Ok(()),
    };

    for (i, entry) in entries.iter().enumerate() {
        let is_last = i == entries.len() - 1;
        let connector = if is_last { "└── " } else { "├── " };
        let new_prefix = if is_last { "    " } else { "│   " };

        let full_path = format!("{}/{}", path, entry);
        let is_dir = ctx.guestfs.is_dir(&full_path).unwrap_or(false);

        let display_name = if is_dir {
            format!("{}/", entry).cyan().bold()
        } else {
            entry.normal()
        };

        println!("{}{}{}", prefix, connector, display_name);

        if is_dir {
            let new_prefix_full = format!("{}{}", prefix, new_prefix);
            let _ = print_tree(ctx, &full_path, &new_prefix_full, depth + 1, max_depth);
        }
    }

    Ok(())
}
