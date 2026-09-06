// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! AI assistant implementation using Rig

use super::tools::DiagnosticTools;
use crate::Guestfs;
use anyhow::{Context, Result};
use colored::Colorize;
use std::path::Path;

#[cfg(feature = "ai")]
use rig::{client::completion::CompletionClient, completion::CompletionModel, providers::openai};

#[cfg(feature = "ai")]
use reqwest;

const SYSTEM_PROMPT: &str = r#"You are an expert Linux system administrator and VM conversion specialist.

Your role is to diagnose VM boot failures, LVM issues, and filesystem problems.

You have access to tools to inspect the VM:
- get_block_devices: List block devices
- get_lvm_info: Get LVM configuration
- get_mounts: Show mounted filesystems
- get_fstab: Read fstab
- get_system_info: Get OS information
- get_kernel_info: Get kernel modules
- check_boot_config: Check boot configuration
- get_security_status: Check security settings
- read_file: Read a specific file (max 100KB)
- list_directory: List directory contents

When diagnosing issues:
1. First gather relevant information using tools
2. Explain what you found
3. Identify the root cause
4. Suggest specific fixes
5. Provide exact commands when possible

Be concise but thorough. Focus on actionable solutions.

IMPORTANT: Never suggest destructive commands without clear warnings.
Always explain WHAT the command does and WHY it's needed.
"#;

pub fn run_ai_assistant(image_path: &Path, query: &str) -> Result<()> {
    // Check for API key
    if std::env::var("OPENAI_API_KEY").is_err() {
        anyhow::bail!(
            "\n{} OPENAI_API_KEY environment variable not set.\n\n\
            To use AI features:\n\
            1. Get an API key from https://platform.openai.com/api-keys\n\
            2. Set the environment variable:\n   \
               export OPENAI_API_KEY='your-key-here'\n\
            3. Run the command again\n",
            "Error:".red().bold()
        );
    }

    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║     GuestKit AI Assistant - VM Diagnostics      ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════╝".cyan()
    );
    println!();
    println!("{} {}", "Query:".yellow().bold(), query);
    println!();
    println!("{} Initializing VM inspection...", "→".cyan());

    // Initialize guestfs
    let mut guestfs = Guestfs::new().context("Failed to create Guestfs handle")?;
    guestfs
        .add_drive_opts(
            image_path.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", image_path.display())
            })?,
            false,
            None,
        )
        .context("Failed to add drive")?;
    guestfs.launch().context("Failed to launch guestfs")?;

    // Inspect and mount
    let roots = guestfs.inspect_os().context("Failed to inspect OS")?;
    if roots.is_empty() {
        anyhow::bail!("No operating systems found in the image");
    }

    let root = &roots[0];
    println!("{} Detected OS: {}", "✓".green(), root.yellow());

    // Mount filesystems
    let mounts = guestfs
        .inspect_get_mountpoints(root)
        .context("Failed to get mountpoints")?;
    for (mountpoint, device) in mounts {
        let _ = guestfs.mount(&device, &mountpoint);
    }

    println!("{} VM filesystem mounted", "✓".green());
    println!();

    // Create diagnostic tools
    let mut tools = DiagnosticTools::new(guestfs, root.to_string());

    // For now, use a simple implementation without full agent framework
    // This is a MVP - full Rig agent integration can come later
    println!("{} Analyzing VM...", "🤖".bold());
    println!();

    // Gather diagnostic information based on query type
    let context = gather_diagnostic_context(&mut tools, query)?;

    println!("{} Consulting AI...", "→".cyan());
    println!();

    // Call OpenAI
    let response = call_openai_simple(query, &context)?;

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
    println!("{} Test in a non-production environment first", "💡".cyan());
    println!();

    // Cleanup
    tools.guestfs.shutdown()?;

    Ok(())
}

fn gather_diagnostic_context(tools: &mut DiagnosticTools, query: &str) -> Result<String> {
    let query_lower = query.to_lowercase();
    let mut context = String::new();

    context.push_str("=== VM Diagnostic Information ===\n\n");

    // Always include basic system info
    context.push_str("System Information:\n");
    if let Ok(info) = tools.get_system_info() {
        context.push_str(&info);
        context.push('\n');
    }

    // Conditional gathering based on query
    if query_lower.contains("lvm") || query_lower.contains("volume") || query_lower.contains("vg") {
        context.push_str("\nLVM Information:\n");
        if let Ok(lvm) = tools.get_lvm_info() {
            context.push_str(&lvm);
            context.push('\n');
        }
    }

    if query_lower.contains("mount")
        || query_lower.contains("fstab")
        || query_lower.contains("filesystem")
    {
        context.push_str("\nCurrent Mounts:\n");
        if let Ok(mounts) = tools.get_mounts() {
            context.push_str(&mounts);
            context.push('\n');
        }

        context.push_str("\nfstab Configuration:\n");
        if let Ok(fstab) = tools.get_fstab() {
            context.push_str(&fstab);
            context.push('\n');
        }
    }

    if query_lower.contains("boot")
        || query_lower.contains("kernel")
        || query_lower.contains("grub")
    {
        context.push_str("\nBoot Configuration Check:\n");
        if let Ok(boot) = tools.check_boot_config() {
            context.push_str(&boot);
            context.push('\n');
        }

        context.push_str("\nKernel Information:\n");
        if let Ok(kernel) = tools.get_kernel_info() {
            context.push_str(&kernel);
            context.push('\n');
        }
    }

    if query_lower.contains("security")
        || query_lower.contains("selinux")
        || query_lower.contains("firewall")
    {
        context.push_str("\nSecurity Status:\n");
        if let Ok(sec) = tools.get_security_status() {
            context.push_str(&sec);
            context.push('\n');
        }
    }

    // Always include block devices
    context.push_str("\nBlock Devices:\n");
    if let Ok(devices) = tools.get_block_devices() {
        context.push_str(&devices);
        context.push('\n');
    }

    Ok(context)
}

fn call_openai_simple(query: &str, context: &str) -> Result<String> {
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
        #[cfg(feature = "ai")]
        {
            use rig::completion::AssistantContent;
            match response.choice.first() {
                AssistantContent::Text(text) => Ok(text.text.clone()),
                _ => anyhow::bail!("Unexpected response type from AI"),
            }
        }

        #[cfg(not(feature = "ai"))]
        {
            anyhow::bail!("AI feature not enabled")
        }
    })
}
