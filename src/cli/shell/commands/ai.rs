// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! AI-related command implementations for interactive shell

use super::ShellContext;
use anyhow::Result;
use colored::Colorize;

#[cfg(feature = "ai")]
use crate::Guestfs;
#[cfg(feature = "ai")]
use reqwest;

#[cfg(feature = "ai")]
use rig::{
    client::completion::CompletionClient,
    completion::{AssistantContent, CompletionModel},
    providers::openai,
};

#[cfg(feature = "ai")]
pub fn cmd_ai(ctx: &mut ShellContext, args: &[&str]) -> Result<()> {
    if args.is_empty() {
        eprintln!("{} ai <query>", "Usage:".yellow());
        eprintln!("Example: ai why won't this boot?");
        return Ok(());
    }

    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        eprintln!(
            "\n{} {}",
            "⚠".yellow().bold(),
            "OPENAI_API_KEY environment variable not set.".yellow()
        );
        eprintln!("\nTo use AI features:");
        eprintln!("  1. Get an API key from https://platform.openai.com/api-keys");
        eprintln!("  2. Set the environment variable:");
        eprintln!("     export OPENAI_API_KEY='your-key-here'");
        eprintln!();
        return Ok(());
    }

    let query = args.join(" ");

    println!("\n{} {}", "🤖".bold(), "Analyzing VM...".cyan());
    println!();

    // Gather diagnostic context based on query
    let context = gather_diagnostic_context(&mut ctx.guestfs, &ctx.root, &query)?;

    println!("{} {}", "→".cyan(), "Consulting AI...".cyan());
    println!();

    // Call OpenAI
    let response = call_openai_simple(&query, &context)?;

    // Display response
    println!("{}", "═".repeat(70).cyan());
    println!("{}", "AI Analysis".yellow().bold());
    println!("{}", "═".repeat(70).cyan());
    println!();
    println!("{}", response);
    println!();
    println!("{}", "═".repeat(70).cyan());
    println!();

    println!(
        "{} Review suggestions carefully before applying",
        "⚠".yellow().bold()
    );
    println!();

    Ok(())
}

#[cfg(not(feature = "ai"))]
pub fn cmd_ai(_ctx: &mut ShellContext, _args: &[&str]) -> Result<()> {
    eprintln!("\n{} AI features not enabled.", "Error:".red().bold());
    eprintln!("Rebuild with: cargo build --features ai");
    eprintln!();
    Ok(())
}

#[cfg(feature = "ai")]
fn gather_diagnostic_context(guestfs: &mut Guestfs, root: &str, query: &str) -> Result<String> {
    use serde_json::json;

    let query_lower = query.to_lowercase();
    let mut context = String::new();

    context.push_str("=== VM Diagnostic Information ===\n\n");

    // Always include basic system info
    context.push_str("System Information:\n");
    let info = json!({
        "os_type": guestfs.inspect_get_type(root).ok(),
        "distro": guestfs.inspect_get_distro(root).ok(),
        "version": {
            "major": guestfs.inspect_get_major_version(root).ok(),
            "minor": guestfs.inspect_get_minor_version(root).ok(),
        },
        "hostname": guestfs.inspect_get_hostname(root).ok(),
        "architecture": guestfs.inspect_get_arch(root).ok(),
    });
    context.push_str(&serde_json::to_string_pretty(&info).unwrap_or_default());
    context.push('\n');

    // Conditional gathering based on query
    if query_lower.contains("lvm") || query_lower.contains("volume") || query_lower.contains("vg") {
        context.push_str("\nLVM Information:\n");
        if let Ok(lvm) = guestfs.inspect_lvm(root) {
            context.push_str(&serde_json::to_string_pretty(&lvm).unwrap_or_default());
            context.push('\n');
        }
    }

    if query_lower.contains("mount")
        || query_lower.contains("fstab")
        || query_lower.contains("filesystem")
    {
        context.push_str("\nCurrent Mounts:\n");
        if let Ok(mounts) = guestfs.mounts() {
            context.push_str(&mounts.join("\n"));
            context.push('\n');
        }

        context.push_str("\nfstab Configuration:\n");
        if let Ok(fstab) = guestfs.inspect_fstab(root) {
            context.push_str(&serde_json::to_string_pretty(&fstab).unwrap_or_default());
            context.push('\n');
        }
    }

    if query_lower.contains("boot")
        || query_lower.contains("kernel")
        || query_lower.contains("grub")
    {
        context.push_str("\nBoot Configuration:\n");
        if guestfs.is_dir("/boot").unwrap_or(false) {
            context.push_str("Boot directory accessible\n");
        }
    }

    if query_lower.contains("security")
        || query_lower.contains("selinux")
        || query_lower.contains("firewall")
    {
        context.push_str("\nSecurity Status:\n");
        if let Ok(sec) = guestfs.inspect_security(root) {
            context.push_str(&serde_json::to_string_pretty(&sec).unwrap_or_default());
            context.push('\n');
        }
    }

    // Always include block devices
    context.push_str("\nBlock Devices:\n");
    if let Ok(devices) = guestfs.list_devices() {
        for device in devices {
            let size = guestfs.blockdev_getsize64(&device).unwrap_or(0);
            context.push_str(&format!("{}: {} MB\n", device, size / 1024 / 1024));
        }
    }

    Ok(context)
}

#[cfg(feature = "ai")]
fn call_openai_simple(query: &str, context: &str) -> Result<String> {
    use anyhow::Context;

    const SYSTEM_PROMPT: &str = r#"You are an expert Linux system administrator and VM conversion specialist.

Your role is to diagnose VM boot failures, LVM issues, and filesystem problems.

When diagnosing issues:
1. Explain what you found
2. Identify the root cause
3. Suggest specific fixes
4. Provide exact commands when possible

Be concise but thorough. Focus on actionable solutions.

IMPORTANT: Never suggest destructive commands without clear warnings.
Always explain WHAT the command does and WHY it's needed.
"#;

    // Get API key from environment
    let api_key =
        std::env::var("OPENAI_API_KEY").context("OPENAI_API_KEY environment variable not set")?;

    // Use tokio runtime for async call
    let runtime = tokio::runtime::Runtime::new()?;

    runtime.block_on(async {
        let full_prompt = format!(
            "{}\n\nUser Query: {}\n\n{}\n\nProvide a clear diagnosis and solution:",
            SYSTEM_PROMPT, query, context
        );

        // Create OpenAI client and call completion API using GPT-4o
        let response = openai::Client::<reqwest::Client>::new(&api_key)
            .context("Failed to create OpenAI client")?
            .completions_api()
            .completion_model(openai::GPT_4O)
            .completion_request(&full_prompt)
            .send()
            .await
            .context("Failed to get AI response")?;

        // Extract text from first choice
        match response.choice.first() {
            AssistantContent::Text(text) => Ok(text.text.clone()),
            _ => anyhow::bail!("Unexpected response type from AI"),
        }
    })
}
