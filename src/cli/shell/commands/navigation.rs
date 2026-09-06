// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Navigation and bookmarks: alias, unalias, bookmark, goto, stats, recent, history_enhanced

use anyhow::Result;
use colored::Colorize;

use super::{format_bytes, resolve_path, ShellContext};

/// Manage aliases
pub fn cmd_alias(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        // List all aliases
        println!("{}", "Current Aliases:".yellow().bold());
        let mut aliases: Vec<_> = ctx.aliases.iter().collect();
        aliases.sort_by_key(|(k, _)| *k);

        for (name, command) in aliases {
            println!("  {} = {}", name.cyan(), command.green());
        }
        println!();
        println!("{}", "Usage: alias <name> <command>".yellow());
        return Ok(());
    }

    if args.len() < 2 {
        println!("{}", "Usage: alias <name> <command>".red());
        println!("{}", "Example: alias ll ls -l".yellow());
        return Ok(());
    }

    let name = args[0].to_string();
    let command = args[1..].join(" ");

    ctx.add_alias(name.clone(), command.clone());
    println!(
        "{} Alias added: {} = {}",
        "✓".green(),
        name.cyan(),
        command.green()
    );

    Ok(())
}

/// Remove an alias
pub fn cmd_unalias(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "Usage: unalias <name>".red());
        return Ok(());
    }

    let name = args[0];
    if ctx.aliases.remove(name).is_some() {
        println!("{} Alias removed: {}", "✓".green(), name.cyan());
    } else {
        println!("{} Alias not found: {}", "⚠".yellow(), name);
    }

    Ok(())
}

/// Manage bookmarks
pub fn cmd_bookmark(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        // List all bookmarks
        println!("{}", "Current Bookmarks:".yellow().bold());
        let mut bookmarks: Vec<_> = ctx.bookmarks.iter().collect();
        bookmarks.sort_by_key(|(k, _)| *k);

        for (name, path) in bookmarks {
            println!("  {} → {}", name.cyan(), path.blue());
        }

        if ctx.bookmarks.is_empty() {
            println!("  {}", "No bookmarks set".yellow());
        }

        println!();
        println!("{}", "Usage:".yellow());
        println!(
            "  {} - Add bookmark for current path",
            "bookmark <name>".green()
        );
        println!(
            "  {} - Add bookmark for specific path",
            "bookmark <name> <path>".green()
        );
        println!("  {} - Jump to bookmark", "goto <name>".green());
        return Ok(());
    }

    let name = args[0].to_string();
    let path = if args.len() > 1 {
        args[1].to_string()
    } else {
        ctx.current_path.clone()
    };

    ctx.add_bookmark(name.clone(), path.clone());
    println!(
        "{} Bookmark added: {} → {}",
        "✓".green(),
        name.cyan(),
        path.blue()
    );

    Ok(())
}

/// Jump to a bookmark
pub fn cmd_goto(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        println!("{}", "Usage: goto <bookmark>".red());
        println!();
        cmd_bookmark(ctx, &[])?; // Show available bookmarks
        return Ok(());
    }

    let name = args[0];
    if let Some(path) = ctx.get_bookmark(name) {
        let path = path.clone(); // Clone to avoid borrow conflict

        // Verify path exists
        if ctx.guestfs.is_dir(&path).unwrap_or(false) {
            ctx.current_path = path.clone();
            println!("{} Jumped to: {}", "→".cyan(), path.blue());
        } else {
            println!("{} Bookmark path no longer exists: {}", "⚠".yellow(), path);
        }
    } else {
        println!("{} Bookmark not found: {}", "⚠".yellow(), name);
        println!();
        cmd_bookmark(ctx, &[])?; // Show available bookmarks
    }

    Ok(())
}

/// Show shell statistics
pub fn cmd_stats(ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!("{}", "Shell Statistics:".yellow().bold());
    println!("  OS: {}", ctx.get_os_info().cyan());
    println!("  Current Path: {}", ctx.current_path.blue());
    println!(
        "  Commands Executed: {}",
        ctx.command_count.to_string().green()
    );

    if let Some(duration) = ctx.last_command_time {
        println!(
            "  Last Command Time: {}",
            format!("{:.2}ms", duration.as_secs_f64() * 1000.0).cyan()
        );
    }

    println!("  Aliases: {}", ctx.aliases.len().to_string().cyan());
    println!("  Bookmarks: {}", ctx.bookmarks.len().to_string().cyan());

    Ok(())
}

/// Show recently modified files
pub fn cmd_recent(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    let path = if args.is_empty() {
        ctx.current_path.clone()
    } else {
        resolve_path(&ctx.current_path, args[0])
    };

    let limit = if args.len() > 1 {
        args[1].parse::<usize>().unwrap_or(20)
    } else {
        20
    };

    println!(
        "{} Finding recently modified files in: {}",
        "→".cyan(),
        path.yellow()
    );
    println!();

    // This is a simplified version - in a real impl, we'd walk the tree and sort by mtime
    if let Ok(entries) = ctx.guestfs.ls(&path) {
        let mut files_with_time = Vec::new();

        for entry in entries.iter().take(limit) {
            let full_path = format!("{}/{}", path.trim_end_matches('/'), entry);
            if let Ok(stat) = ctx.guestfs.stat(&full_path) {
                files_with_time.push((entry.clone(), stat.mtime, stat.size));
            }
        }

        // Sort by modification time (descending)
        files_with_time.sort_by_key(|b| std::cmp::Reverse(b.1));

        println!("{}", "Recently Modified Files:".yellow().bold());
        println!("{}", "─".repeat(80).cyan());

        for (name, mtime, size) in files_with_time.iter().take(limit) {
            use chrono::{DateTime, Utc};
            let dt = DateTime::<Utc>::from_timestamp(*mtime, 0).unwrap_or_else(Utc::now);
            let time_str = dt.format("%Y-%m-%d %H:%M:%S").to_string();

            let size_str = format_bytes(*size as u64);
            println!(
                "  {} {} {} {}",
                time_str.bright_black(),
                name.green(),
                "-".bright_black(),
                size_str.yellow()
            );
        }
        println!();
    }

    Ok(())
}

/// Show command history with analysis
pub fn cmd_history_enhanced(ctx: &ShellContext, _args: &[&str]) -> Result<()> {
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════╗"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "║          Command History Analysis               ║"
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

    println!("{}", "Session Statistics:".yellow().bold());
    println!(
        "  Commands executed: {}",
        ctx.command_count.to_string().green().bold()
    );

    if let Some(duration) = ctx.last_command_time {
        println!(
            "  Last command time: {}",
            format!("{:.2}ms", duration.as_secs_f64() * 1000.0).cyan()
        );
    }

    println!(
        "  Aliases defined: {}",
        ctx.aliases.len().to_string().cyan()
    );
    println!(
        "  Bookmarks saved: {}",
        ctx.bookmarks.len().to_string().cyan()
    );
    println!();

    println!("{}", "Most Useful Commands:".yellow().bold());
    println!("  {} - Quick system overview", "dashboard".green());
    println!("  {} - Export for analysis", "snapshot".green());
    println!("  {} - Fast shortcuts", "quick".green());
    println!("  {} - Search anything", "search".green());
    println!("  {} - Multiple operations", "batch".green());
    println!();

    println!("{} Use 'history' to see full command list", "Tip:".yellow());
    println!("{} Type 'cheat' for command reference", "Tip:".yellow());
    println!();

    Ok(())
}
