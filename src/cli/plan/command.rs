// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Plan command - manage fix plans

use super::*;
use anyhow::{Context, Result};
use clap::{Args, Subcommand};
use colored::*;
use std::fs;
use std::path::Path;

#[derive(Debug, Args)]
pub struct PlanCommand {
    #[command(subcommand)]
    pub action: PlanAction,
}

// clap Subcommand: variants are constructed once per CLI invocation from
// parsed args, never in a hot path — boxing fields to shrink the enum
// isn't worth the added indirection at every call site.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Subcommand)]
pub enum PlanAction {
    /// Preview a fix plan
    Preview {
        /// Path to plan file (YAML/JSON)
        #[arg(value_name = "PLAN_FILE")]
        plan_file: String,

        /// Show as unified diff
        #[arg(short, long)]
        diff: bool,

        /// Show summary only
        #[arg(short, long)]
        summary: bool,
    },

    /// Validate a fix plan
    Validate {
        /// Path to plan file (YAML/JSON)
        #[arg(value_name = "PLAN_FILE")]
        plan_file: String,

        /// VM disk path (overrides plan)
        #[arg(short, long)]
        vm: Option<String>,
    },

    /// Export a fix plan to different formats
    Export {
        /// Path to plan file (YAML/JSON)
        #[arg(value_name = "PLAN_FILE")]
        plan_file: String,

        /// Output file path
        #[arg(short, long, value_name = "FILE")]
        output: String,

        /// Export format
        #[arg(short, long, value_enum, default_value = "bash")]
        format: ExportFormat,
    },

    /// Apply a fix plan
    Apply {
        /// Path to plan file (YAML/JSON)
        #[arg(value_name = "PLAN_FILE")]
        plan_file: String,

        /// VM disk path (overrides plan)
        #[arg(short, long)]
        vm: Option<String>,

        /// Dry run (don't make changes)
        #[arg(short = 'n', long)]
        dry_run: bool,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,

        /// Interactive mode (prompt for each operation)
        #[arg(short, long)]
        interactive: bool,

        /// Backup directory
        #[arg(short, long)]
        backup: Option<String>,

        /// Skip the full-image qcow2/raw copy taken before apply.
        ///
        /// Use for low-risk plans (e.g. registry-only enable-RDP) where a
        /// 30–40 GiB Windows golden backup is slower than the edit. Default
        /// remains to refuse apply without a successful full-image backup.
        #[arg(long)]
        skip_backup: bool,
    },

    /// Rollback to a previous state
    Rollback {
        /// Backup directory path
        #[arg(value_name = "BACKUP_DIR")]
        backup_dir: String,

        /// VM disk path
        #[arg(short, long)]
        vm: String,

        /// Skip confirmation prompt
        #[arg(short, long)]
        yes: bool,
    },

    /// Generate a fix plan from a profile
    Generate {
        /// VM disk path
        #[arg(value_name = "VM_DISK")]
        vm_disk: String,

        /// Profile to use (security, windows-rdp, linux-ssh, windows-hostname, windows-winrm, …)
        #[arg(short, long, default_value = "security")]
        profile: String,

        /// Output plan file
        #[arg(short, long)]
        output: String,

        /// Format (yaml or json)
        #[arg(short, long, value_enum, default_value = "yaml")]
        format: PlanFileFormat,

        /// Linux user for SSH key inject (with linux-ssh + --key/--key-file)
        #[arg(long)]
        user: Option<String>,

        /// SSH public key string (with linux-ssh + --user)
        #[arg(long)]
        key: Option<String>,

        /// Path to SSH public key file (with linux-ssh + --user)
        #[arg(long)]
        key_file: Option<String>,

        /// Hostname for windows-hostname / linux-hostname profiles
        #[arg(long)]
        hostname: Option<String>,

        /// Workgroup for windows-domain-leave (default WORKGROUP)
        #[arg(long, default_value = "WORKGROUP")]
        workgroup: String,

        /// Windows TimeZoneKeyName for windows-timezone (e.g. UTC, Pacific Standard Time)
        #[arg(long)]
        timezone: Option<String>,

        /// Interface GUID for windows-static-ip / windows-dhcp / windows-dns
        #[arg(long)]
        interface_guid: Option<String>,

        /// IPv4 address for windows-static-ip
        #[arg(long)]
        ip: Option<String>,

        /// IPv4 subnet mask for windows-static-ip
        #[arg(long)]
        mask: Option<String>,

        /// IPv4 gateway for windows-static-ip
        #[arg(long)]
        gateway: Option<String>,

        /// DNS servers for windows-static-ip / windows-dns (space or comma separated)
        #[arg(long)]
        dns: Option<String>,

        /// GRUB_TIMEOUT for linux-grub profile
        #[arg(long)]
        grub_timeout: Option<u32>,

        /// Append token to GRUB_CMDLINE_LINUX for linux-grub profile
        #[arg(long)]
        grub_cmdline: Option<String>,
    },

    /// Show plan statistics
    Stats {
        /// Path to plan file (YAML/JSON)
        #[arg(value_name = "PLAN_FILE")]
        plan_file: String,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum ExportFormat {
    Bash,
    Ansible,
    Json,
    Yaml,
}

#[derive(Debug, Clone, clap::ValueEnum)]
pub enum PlanFileFormat {
    Yaml,
    Json,
}

impl PlanCommand {
    pub fn execute(&self) -> Result<()> {
        match &self.action {
            PlanAction::Preview {
                plan_file,
                diff,
                summary,
            } => self.preview_plan(plan_file, *diff, *summary),
            PlanAction::Validate { plan_file, vm } => self.validate_plan(plan_file, vm.as_deref()),
            PlanAction::Export {
                plan_file,
                output,
                format,
            } => self.export_plan(plan_file, output, format),
            PlanAction::Apply {
                plan_file,
                vm,
                dry_run,
                yes,
                interactive,
                backup,
                skip_backup,
            } => self.apply_plan(
                plan_file,
                vm.as_deref(),
                *dry_run,
                *yes,
                *interactive,
                *skip_backup,
                backup.as_deref(),
            ),
            PlanAction::Rollback {
                backup_dir,
                vm,
                yes,
            } => self.rollback(backup_dir, vm, *yes),
            PlanAction::Generate {
                vm_disk,
                profile,
                output,
                format,
                user,
                key,
                key_file,
                hostname,
                workgroup,
                timezone,
                interface_guid,
                ip,
                mask,
                gateway,
                dns,
                grub_timeout,
                grub_cmdline,
            } => self.generate_plan(
                vm_disk,
                profile,
                output,
                format,
                user.as_deref(),
                key.as_deref(),
                key_file.as_deref(),
                hostname.as_deref(),
                workgroup,
                timezone.as_deref(),
                interface_guid.as_deref(),
                ip.as_deref(),
                mask.as_deref(),
                gateway.as_deref(),
                dns.as_deref(),
                *grub_timeout,
                grub_cmdline.as_deref(),
            ),
            PlanAction::Stats { plan_file } => self.show_stats(plan_file),
        }
    }

    fn load_plan(&self, path: &str) -> Result<FixPlan> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read plan file: {}", path))?;

        // Try YAML first, then JSON
        if path.ends_with(".yaml") || path.ends_with(".yml") {
            serde_yaml::from_str(&content)
                .with_context(|| format!("Failed to parse YAML plan: {}", path))
        } else if path.ends_with(".json") {
            serde_json::from_str(&content)
                .with_context(|| format!("Failed to parse JSON plan: {}", path))
        } else {
            // Auto-detect
            serde_yaml::from_str(&content)
                .or_else(|_| serde_json::from_str(&content))
                .with_context(|| {
                    format!("Failed to parse plan file (tried YAML and JSON): {}", path)
                })
        }
    }

    fn preview_plan(&self, plan_file: &str, diff: bool, summary: bool) -> Result<()> {
        let plan = self.load_plan(plan_file)?;

        if summary {
            PlanPreview::print_summary(&plan);
        } else if diff {
            PlanPreview::display_diff(&plan);
        } else {
            PlanPreview::display(&plan);
        }

        Ok(())
    }

    fn validate_plan(&self, plan_file: &str, vm_override: Option<&str>) -> Result<()> {
        let plan = self.load_plan(plan_file)?;
        let vm_path = vm_override.unwrap_or(&plan.vm);

        println!("{}", "Validating plan...".bold().cyan());
        println!();

        let applicator = PlanApplicator::new(vm_path.to_string(), true);
        let result = applicator.validate(&plan)?;

        if result.is_valid() {
            println!("{}", "✓ Plan is valid".green().bold());

            if !result.warnings.is_empty() {
                println!();
                println!("{}", "Warnings:".yellow().bold());
                for warning in &result.warnings {
                    println!("  ⚠️  {}", warning.yellow());
                }
            }
        } else {
            println!("{}", "✗ Plan validation failed".red().bold());
            println!();
            println!("{}", "Errors:".red().bold());
            for error in &result.errors {
                println!("  ✗ {}", error.red());
            }

            if !result.warnings.is_empty() {
                println!();
                println!("{}", "Warnings:".yellow().bold());
                for warning in &result.warnings {
                    println!("  ⚠️  {}", warning.yellow());
                }
            }

            anyhow::bail!("Plan validation failed");
        }

        Ok(())
    }

    fn export_plan(&self, plan_file: &str, output: &str, format: &ExportFormat) -> Result<()> {
        let plan = self.load_plan(plan_file)?;

        println!(
            "Exporting plan to {} format...",
            match format {
                ExportFormat::Bash => "bash",
                ExportFormat::Ansible => "ansible",
                ExportFormat::Json => "JSON",
                ExportFormat::Yaml => "YAML",
            }
            .cyan()
        );

        let content = match format {
            ExportFormat::Bash => PlanExporter::to_bash(&plan)?,
            ExportFormat::Ansible => PlanExporter::to_ansible(&plan)?,
            ExportFormat::Json => PlanExporter::to_json(&plan)?,
            ExportFormat::Yaml => PlanExporter::to_yaml(&plan)?,
        };

        fs::write(output, content)
            .with_context(|| format!("Failed to write output file: {}", output))?;

        println!("{} Exported to: {}", "✓".green(), output.bright_blue());

        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // CLI handler: args pass through from clap-parsed flags
    fn apply_plan(
        &self,
        plan_file: &str,
        vm_override: Option<&str>,
        dry_run: bool,
        yes: bool,
        interactive: bool,
        skip_backup: bool,
        _backup_dir: Option<&str>,
    ) -> Result<()> {
        let plan = self.load_plan(plan_file)?;
        let vm_path = vm_override.unwrap_or(&plan.vm);

        // Validate first
        let applicator = PlanApplicator::new(vm_path.to_string(), true);
        let validation = applicator.validate(&plan)?;

        if !validation.is_valid() {
            println!("{}", "✗ Plan validation failed".red().bold());
            for error in &validation.errors {
                println!("  ✗ {}", error.red());
            }
            anyhow::bail!("Cannot apply invalid plan");
        }

        // Show preview
        println!();
        PlanPreview::display(&plan);
        println!();

        // Confirm unless --yes or --dry-run
        if !yes && !dry_run && !interactive {
            print!("{}", "Apply this plan? [y/N] ".yellow().bold());
            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        // Apply
        let applicator = PlanApplicator::new(vm_path.to_string(), dry_run).skip_backup(skip_backup);

        if dry_run {
            println!();
            println!(
                "{}",
                "DRY RUN MODE - No changes will be made".yellow().bold()
            );
            println!();
        } else if skip_backup {
            println!("{}", "Skipping full-image backup (--skip-backup)".yellow());
        }

        let result = applicator.apply(&plan)?;

        println!();
        if result.success {
            println!("{}", "✓ Plan applied successfully".green().bold());
            println!("  Operations applied: {}", result.operations_applied);
            println!("  Operations skipped: {}", result.operations_skipped);
        } else {
            println!("{}", "✗ Plan application failed".red().bold());
            println!("  Operations applied: {}", result.operations_applied);
            println!("  Operations failed: {}", result.operations_failed);
            println!("  Message: {}", result.message);
        }

        Ok(())
    }

    fn rollback(&self, backup_dir: &str, vm: &str, yes: bool) -> Result<()> {
        if !Path::new(backup_dir).exists() {
            anyhow::bail!("Backup directory not found: {}", backup_dir);
        }

        println!("{}", "Rollback Operation".bold().red());
        println!("{}", "═".repeat(60).bright_black());
        println!("Backup: {}", backup_dir.bright_blue());
        println!("VM: {}", vm.bright_blue());
        println!("{}", "═".repeat(60).bright_black());
        println!();
        println!(
            "{}",
            "WARNING: This will restore files from backup."
                .yellow()
                .bold()
        );
        println!(
            "{}",
            "Any changes made after the backup will be lost.".yellow()
        );
        println!();

        if !yes {
            print!("{}", "Continue with rollback? [y/N] ".red().bold());
            use std::io::{self, Write};
            io::stdout().flush()?;

            let mut input = String::new();
            io::stdin().read_line(&mut input)?;

            if !input.trim().eq_ignore_ascii_case("y") {
                println!("Aborted.");
                return Ok(());
            }
        }

        let applicator = PlanApplicator::new(vm.to_string(), false);
        applicator.rollback(backup_dir)?;

        println!();
        println!("{}", "✓ Rollback completed successfully".green().bold());

        Ok(())
    }

    #[allow(clippy::too_many_arguments)] // CLI handler: args pass through from clap-parsed flags
    fn generate_plan(
        &self,
        vm_disk: &str,
        profile: &str,
        output: &str,
        format: &PlanFileFormat,
        user: Option<&str>,
        key: Option<&str>,
        key_file: Option<&str>,
        hostname: Option<&str>,
        workgroup: &str,
        timezone: Option<&str>,
        interface_guid: Option<&str>,
        ip: Option<&str>,
        mask: Option<&str>,
        gateway: Option<&str>,
        dns: Option<&str>,
        grub_timeout: Option<u32>,
        grub_cmdline: Option<&str>,
    ) -> Result<()> {
        println!(
            "Generating {} plan for {}...",
            profile.cyan(),
            vm_disk.bright_blue()
        );

        let profile_lc = profile.to_lowercase();

        // Registry-only Windows canned plans — no guestfs inspect needed.
        if matches!(
            profile_lc.as_str(),
            "windows-rdp"
                | "windows_rdp"
                | "rdp"
                | "enable-rdp"
                | "windows-winrm"
                | "windows_winrm"
                | "winrm"
                | "enable-winrm"
                | "windows-hostname"
                | "windows_hostname"
                | "hostname"
                | "set-hostname"
                | "windows-domain-leave"
                | "windows_domain_leave"
                | "domain-leave"
                | "unjoin"
                | "windows-timezone"
                | "windows_timezone"
                | "timezone"
                | "set-timezone"
                | "windows-static-ip"
                | "windows_static_ip"
                | "static-ip"
                | "windows-dhcp"
                | "windows_dhcp"
                | "dhcp"
                | "enable-dhcp"
                | "windows-dns"
                | "windows_dns"
                | "dns"
                | "set-dns"
                | "cloud-init-aws"
                | "cloud-init-ec2"
                | "cloud-init-azure"
                | "cloud-init-gcp"
                | "cloud-init-gce"
                | "cloud-init-openstack"
                | "cloud-init-nocloud"
                | "selinux-relabel"
                | "selinux_relabel"
                | "sysprep"
                | "windows-sysprep"
                | "windows_sysprep"
                | "virtio-initramfs"
                | "virtio_initramfs"
        ) {
            if !Path::new(vm_disk).exists() {
                anyhow::bail!("VM disk not found: {vm_disk}");
            }
            let generator = PlanGenerator::new(vm_disk.to_string());
            let plan = match profile_lc.as_str() {
                "windows-rdp" | "windows_rdp" | "rdp" | "enable-rdp" => {
                    generator.windows_rdp_enable_plan()
                }
                "windows-winrm" | "windows_winrm" | "winrm" | "enable-winrm" => {
                    generator.windows_winrm_enable_plan()
                }
                "windows-hostname" | "windows_hostname" | "hostname" | "set-hostname" => {
                    let name = hostname.ok_or_else(|| {
                        anyhow::anyhow!("--hostname is required for windows-hostname profile")
                    })?;
                    generator.windows_hostname_plan(name)?
                }
                "windows-domain-leave" | "windows_domain_leave" | "domain-leave" | "unjoin" => {
                    generator.windows_domain_leave_plan(workgroup)?
                }
                "windows-timezone" | "windows_timezone" | "timezone" | "set-timezone" => {
                    let tz = timezone.ok_or_else(|| {
                        anyhow::anyhow!("--timezone is required for windows-timezone profile")
                    })?;
                    generator.windows_timezone_plan(tz)?
                }
                "windows-static-ip" | "windows_static_ip" | "static-ip" => {
                    let guid = interface_guid.ok_or_else(|| {
                        anyhow::anyhow!("--interface-guid is required for windows-static-ip")
                    })?;
                    let addr = ip
                        .ok_or_else(|| anyhow::anyhow!("--ip is required for windows-static-ip"))?;
                    let netmask = mask.ok_or_else(|| {
                        anyhow::anyhow!("--mask is required for windows-static-ip")
                    })?;
                    generator.windows_static_ip_plan(guid, addr, netmask, gateway, dns)?
                }
                "windows-dhcp" | "windows_dhcp" | "dhcp" | "enable-dhcp" => {
                    let guid = interface_guid.ok_or_else(|| {
                        anyhow::anyhow!("--interface-guid is required for windows-dhcp")
                    })?;
                    generator.windows_dhcp_plan(guid)?
                }
                "windows-dns" | "windows_dns" | "dns" | "set-dns" => {
                    let guid = interface_guid.ok_or_else(|| {
                        anyhow::anyhow!("--interface-guid is required for windows-dns")
                    })?;
                    let servers =
                        dns.ok_or_else(|| anyhow::anyhow!("--dns is required for windows-dns"))?;
                    generator.windows_dns_plan(guid, servers)?
                }
                "cloud-init-aws" | "cloud-init-ec2" => {
                    crate::cli::plan::cloud_init::cloud_init_plan(
                        crate::cli::plan::cloud_init::CloudInitOpts {
                            vm: vm_disk,
                            ds: crate::cli::plan::cloud_init::Datasource::Ec2,
                            user_data: None,
                            meta_data: None,
                            disable_network: false,
                            instance_id: None,
                        },
                    )
                }
                "cloud-init-azure" => crate::cli::plan::cloud_init::cloud_init_plan(
                    crate::cli::plan::cloud_init::CloudInitOpts {
                        vm: vm_disk,
                        ds: crate::cli::plan::cloud_init::Datasource::Azure,
                        user_data: None,
                        meta_data: None,
                        disable_network: false,
                        instance_id: None,
                    },
                ),
                "cloud-init-gcp" | "cloud-init-gce" => {
                    crate::cli::plan::cloud_init::cloud_init_plan(
                        crate::cli::plan::cloud_init::CloudInitOpts {
                            vm: vm_disk,
                            ds: crate::cli::plan::cloud_init::Datasource::Gce,
                            user_data: None,
                            meta_data: None,
                            disable_network: false,
                            instance_id: None,
                        },
                    )
                }
                "cloud-init-openstack" => crate::cli::plan::cloud_init::cloud_init_plan(
                    crate::cli::plan::cloud_init::CloudInitOpts {
                        vm: vm_disk,
                        ds: crate::cli::plan::cloud_init::Datasource::OpenStack,
                        user_data: None,
                        meta_data: None,
                        disable_network: false,
                        instance_id: None,
                    },
                ),
                "cloud-init-nocloud" => crate::cli::plan::cloud_init::cloud_init_plan(
                    crate::cli::plan::cloud_init::CloudInitOpts {
                        vm: vm_disk,
                        ds: crate::cli::plan::cloud_init::Datasource::NoCloud,
                        user_data: None,
                        meta_data: None,
                        disable_network: false,
                        instance_id: hostname,
                    },
                ),
                "selinux-relabel" | "selinux_relabel" => {
                    crate::cli::plan::cutover_prep::selinux_relabel_plan(vm_disk)
                }
                "sysprep" | "windows-sysprep" | "windows_sysprep" => {
                    crate::cli::plan::cutover_prep::windows_sysprep_plan(vm_disk, hostname, true)
                }
                "virtio-initramfs" | "virtio_initramfs" => {
                    crate::cli::plan::linux_boot::virtio_initramfs_plan(vm_disk, true)
                }
                _ => unreachable!(),
            };
            Self::write_plan_file(output, format, &plan)?;
            Self::print_generate_summary(output, &plan, vm_disk, true);
            return Ok(());
        }

        // Open VM with Guestfs
        let mut g = crate::guestfs::Guestfs::new()
            .map_err(|e| anyhow::anyhow!("Failed to create Guestfs handle: {}", e))?;
        g.add_drive_ro(vm_disk)
            .map_err(|e| anyhow::anyhow!("Failed to add drive: {}", e))?;
        g.launch()
            .map_err(|e| anyhow::anyhow!("Failed to launch Guestfs: {}", e))?;

        // Inspect OS
        let roots = g
            .inspect_os()
            .map_err(|e| anyhow::anyhow!("Failed to inspect OS: {}", e))?;
        if roots.is_empty() {
            anyhow::bail!("No operating system found in {}", vm_disk);
        }

        let root = &roots[0];

        // Mount filesystems
        if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
            let mut mounts: Vec<_> = mountpoints.into_iter().collect();
            mounts.sort_by_key(|(mount, _)| mount.len());
            for (mount, device) in &mounts {
                let _ = g.mount(device, mount);
            }
        }

        let generator = PlanGenerator::new(vm_disk.to_string());

        let pubkey = match (key, key_file) {
            (Some(k), None) => Some(k.to_string()),
            (None, Some(path)) => Some(
                fs::read_to_string(path)
                    .with_context(|| format!("Failed to read key file: {path}"))?
                    .trim()
                    .to_string(),
            ),
            (Some(_), Some(_)) => {
                anyhow::bail!("Use either --key or --key-file, not both")
            }
            (None, None) => None,
        };

        // linux-ssh / linux-hostname build inspect-based enable plans (not finding→op).
        let plan = if matches!(
            profile_lc.as_str(),
            "linux-ssh" | "linux_ssh" | "enable-ssh"
        ) {
            generator.linux_ssh_enable_plan(&mut g, user, pubkey.as_deref())?
        } else if matches!(
            profile_lc.as_str(),
            "linux-hostname" | "linux_hostname" | "set-linux-hostname"
        ) {
            let name = hostname.ok_or_else(|| {
                anyhow::anyhow!("--hostname is required for linux-hostname profile")
            })?;
            generator.linux_hostname_plan(&mut g, name)?
        } else if matches!(
            profile_lc.as_str(),
            "linux-grub" | "linux_grub" | "grub-defaults" | "grub"
        ) {
            generator.linux_grub_defaults_plan(&mut g, grub_timeout, grub_cmdline)?
        } else {
            let inspection_profile = crate::cli::profiles::get_profile(profile)
                .ok_or_else(|| anyhow::anyhow!("Unknown profile: {}", profile))?;

            let report = inspection_profile
                .inspect(&mut g, root)
                .map_err(|e| anyhow::anyhow!("Profile inspection failed: {}", e))?;

            generator.from_security_profile(&report)?
        };

        Self::write_plan_file(output, format, &plan)?;
        let _ = g.shutdown();
        let skip_backup_hint = matches!(
            profile_lc.as_str(),
            "linux-ssh"
                | "linux_ssh"
                | "enable-ssh"
                | "linux-hostname"
                | "linux_hostname"
                | "set-linux-hostname"
                | "linux-grub"
                | "linux_grub"
                | "grub-defaults"
                | "grub"
        );
        Self::print_generate_summary(output, &plan, vm_disk, skip_backup_hint);

        Ok(())
    }

    fn write_plan_file(output: &str, format: &PlanFileFormat, plan: &FixPlan) -> Result<()> {
        let content = match format {
            PlanFileFormat::Yaml => {
                serde_yaml::to_string(plan).with_context(|| "Failed to serialize plan to YAML")?
            }
            PlanFileFormat::Json => serde_json::to_string_pretty(plan)
                .with_context(|| "Failed to serialize plan to JSON")?,
        };
        fs::write(output, &content)
            .with_context(|| format!("Failed to write plan to: {}", output))?;
        Ok(())
    }

    fn print_generate_summary(output: &str, plan: &FixPlan, vm_disk: &str, skip_backup_hint: bool) {
        println!("{} Plan generated: {}", "✓".green(), output.bright_blue());
        println!("  Operations: {}", plan.operations.len());
        println!("  Overall risk: {}", plan.overall_risk);
        if skip_backup_hint {
            println!(
                "  Apply with: guestkit plan apply {} --vm {} --yes --skip-backup",
                output.bright_blue(),
                vm_disk.bright_blue()
            );
        }
    }

    fn show_stats(&self, plan_file: &str) -> Result<()> {
        let plan = self.load_plan(plan_file)?;

        println!();
        println!("{}", "Plan Statistics".bold().cyan());
        println!("{}", "═".repeat(60).bright_black());
        println!();
        println!("File: {}", plan_file.bright_blue());
        println!("VM: {}", plan.vm);
        println!("Profile: {}", plan.profile);
        println!("Generated: {}", plan.generated);
        println!(
            "Risk: {}",
            match plan.overall_risk.as_str() {
                "critical" => plan.overall_risk.red().bold(),
                "high" => plan.overall_risk.bright_red(),
                "medium" => plan.overall_risk.yellow(),
                "low" => plan.overall_risk.green(),
                _ => plan.overall_risk.normal(),
            }
        );
        println!("Duration: {}", plan.estimated_duration);
        println!();
        println!("{}", "Operations:".bold());
        println!("  Total: {}", plan.operations.len());
        println!("  Critical: {}", plan.count_by_priority(Priority::Critical));
        println!("  High: {}", plan.count_by_priority(Priority::High));
        println!("  Medium: {}", plan.count_by_priority(Priority::Medium));
        println!("  Low: {}", plan.count_by_priority(Priority::Low));
        println!("  Info: {}", plan.count_by_priority(Priority::Info));
        println!();
        println!(
            "Reversible: {}",
            if plan.metadata.reversible {
                "Yes".green()
            } else {
                "No".red()
            }
        );
        println!(
            "Review Required: {}",
            if plan.metadata.review_required {
                "Yes".yellow()
            } else {
                "No".green()
            }
        );
        println!();

        if !plan.post_apply.is_empty() {
            println!("{}", "Post-Apply Actions:".bold());
            for (i, action) in plan.post_apply.iter().enumerate() {
                match action {
                    PostApplyAction::ServiceRestart { services } => {
                        println!("  {}. Restart services: {}", i + 1, services.join(", "));
                    }
                    PostApplyAction::Validation { command, .. } => {
                        println!("  {}. Validate: {}", i + 1, command);
                    }
                    PostApplyAction::Message { message } => {
                        println!("  {}. {}", i + 1, message);
                    }
                    PostApplyAction::RebootRequired { reason } => {
                        println!("  {}. Reboot required: {}", i + 1, reason);
                    }
                }
            }
            println!();
        }

        Ok(())
    }
}
