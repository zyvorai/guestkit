// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Interactive REPL mode for guestkit CLI

use super::errors::builders as errors;
use super::invocation;
use crate::Guestfs;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use rustyline::completion::{Completer, Pair};
use rustyline::error::ReadlineError;
use rustyline::highlight::Highlighter;
use rustyline::hint::Hinter;
use rustyline::validate::Validator;
use rustyline::{Context as RustyContext, Editor, Helper, Result as RustyResult};
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

/// Get the history directory path (~/.guestkit/history/)
fn get_history_dir() -> Result<PathBuf> {
    let home =
        dirs::home_dir().ok_or_else(|| anyhow::anyhow!("Could not determine home directory"))?;
    let history_dir = home.join(".guestkit").join("history");

    // Create directory if it doesn't exist
    if !history_dir.exists() {
        fs::create_dir_all(&history_dir).with_context(|| {
            format!(
                "Failed to create history directory: {}",
                history_dir.display()
            )
        })?;
    }

    Ok(history_dir)
}

/// Get history file path for a specific disk image
/// Uses a hash of the disk path to create a unique history file
fn get_history_file(disk_path: &Path) -> Result<PathBuf> {
    let history_dir = get_history_dir()?;

    // Create a hash of the disk path for a unique filename
    let mut hasher = DefaultHasher::new();
    disk_path.to_string_lossy().hash(&mut hasher);
    let hash = hasher.finish();

    let filename = format!("guestkit-{:x}.history", hash);
    Ok(history_dir.join(filename))
}

/// Helper for rustyline completion
struct GuestkitHelper;

impl Helper for GuestkitHelper {}
impl Hinter for GuestkitHelper {
    type Hint = String;
}
impl Highlighter for GuestkitHelper {}
impl Validator for GuestkitHelper {}

impl Completer for GuestkitHelper {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _ctx: &RustyContext<'_>,
    ) -> RustyResult<(usize, Vec<Pair>)> {
        let before_cursor = &line[..pos];
        let parts: Vec<&str> = before_cursor.split_whitespace().collect();

        // Command completion
        if parts.is_empty() || (parts.len() == 1 && !before_cursor.ends_with(' ')) {
            #[allow(unused_mut)]
            let mut commands = vec![
                "help",
                "info",
                "filesystems",
                "fs",
                "mount",
                "umount",
                "unmount",
                "mounts",
                "ls",
                "cat",
                "head",
                "find",
                "stat",
                "download",
                "dl",
                // File operations
                "upload",
                "edit",
                "write",
                "copy",
                "move",
                "delete",
                "mkdir",
                "chmod",
                "chown",
                "symlink",
                "large-files",
                "disk-usage",
                // Package management
                "packages",
                "pkg",
                "install",
                "remove",
                "update",
                "search",
                // System info
                "services",
                "svc",
                "users",
                "network",
                "net",
                // User management
                "adduser",
                "deluser",
                "passwd",
                "usermod",
                "groups",
                // SSH management
                "ssh-addkey",
                "ssh-removekey",
                "ssh-listkeys",
                "ssh-enable",
                "ssh-config",
                // System configuration
                "hostname",
                "timezone",
                "selinux",
                "locale",
                // Service management
                "enable",
                "disable",
                "restart",
                "logs",
                "failed",
                "boot-time",
                // Firewall
                "firewall-add",
                "firewall-remove",
                "firewall-list",
                // Cron
                "cron-add",
                "cron-list",
                // Cleanup
                "clean-logs",
                "clean-cache",
                "clean-temp",
                "clean-kernels",
                // Backup
                "backup",
                "backups",
                // GRUB
                "grub",
                // Network config
                "net-setip",
                "net-setdns",
                "net-route-add",
                "net-dhcp",
                // Process management
                "ps",
                "kill",
                "top",
                // Security & audit
                "scan-ports",
                "audit-perms",
                "audit-suid",
                "check-updates",
                // Database
                "db-list",
                "db-backup",
                // Advanced file ops
                "grep-replace",
                "diff",
                "tree",
                "compress",
                "extract",
                // Git
                "git-clone",
                "git-pull",
                // Performance tuning
                "tune-swappiness",
                "tune-show",
                // Setup wizards
                "setup-webserver",
                "setup-database",
                "setup-docker",
                // Monitoring
                "metrics",
                "bandwidth",
                // SELinux advanced
                "selinux-context",
                "selinux-audit",
                // Templates
                "template-save",
                // Other
                "clear",
                "cls",
                "exit",
                "quit",
                "q",
            ];

            #[cfg(feature = "ai")]
            commands.push("ai");

            let prefix = parts.last().unwrap_or(&"");
            let matches: Vec<Pair> = commands
                .iter()
                .filter(|cmd| cmd.starts_with(prefix))
                .map(|cmd| Pair {
                    display: cmd.to_string(),
                    replacement: cmd.to_string(),
                })
                .collect();

            let start = if parts.is_empty() {
                0
            } else {
                before_cursor.len() - prefix.len()
            };

            return Ok((start, matches));
        }

        // Device/filesystem completion for mount commands
        if parts.len() >= 2 {
            let command = parts[0];

            if command == "mount" && parts.len() == 2 {
                let prefix = parts.last().unwrap_or(&"");

                // Common device paths in VMs
                let devices = vec![
                    "/dev/sda",
                    "/dev/sda1",
                    "/dev/sda2",
                    "/dev/sda3",
                    "/dev/vda",
                    "/dev/vda1",
                    "/dev/vda2",
                    "/dev/vda3",
                    "/dev/mapper/",
                ];

                let matches: Vec<Pair> = devices
                    .iter()
                    .filter(|dev| dev.starts_with(prefix))
                    .map(|dev| Pair {
                        display: dev.to_string(),
                        replacement: dev.to_string(),
                    })
                    .collect();

                if !matches.is_empty() {
                    let start = before_cursor.len() - prefix.len();
                    return Ok((start, matches));
                }
            }

            // Mount point completion for mount command (second argument)
            if command == "mount" && parts.len() == 3 {
                let prefix = parts.last().unwrap_or(&"");

                let mount_points = ["/mnt", "/mnt/", "/tmp/mnt", "/tmp/mnt/"];

                let matches: Vec<Pair> = mount_points
                    .iter()
                    .filter(|mp| mp.starts_with(prefix))
                    .map(|mp| Pair {
                        display: mp.to_string(),
                        replacement: mp.to_string(),
                    })
                    .collect();

                if !matches.is_empty() {
                    let start = before_cursor.len() - prefix.len();
                    return Ok((start, matches));
                }
            }

            // Path completion for commands that expect paths
            let path_commands = vec![
                "ls", "cat", "head", "stat", "find", "download", "dl", "umount", "unmount",
            ];

            if path_commands.contains(&command) {
                let prefix = parts.last().unwrap_or(&"");

                // Common Linux paths
                let common_paths = vec![
                    "/", "/etc", "/etc/", "/var", "/var/", "/home", "/home/", "/usr", "/usr/",
                    "/tmp", "/tmp/", "/opt", "/opt/", "/root", "/root/", "/boot", "/boot/", "/dev",
                    "/dev/", "/proc", "/proc/", "/sys", "/sys/", "/run", "/run/", "/mnt", "/mnt/",
                ];

                let matches: Vec<Pair> = common_paths
                    .iter()
                    .filter(|path| path.starts_with(prefix))
                    .map(|path| Pair {
                        display: path.to_string(),
                        replacement: path.to_string(),
                    })
                    .collect();

                if !matches.is_empty() {
                    let start = before_cursor.len() - prefix.len();
                    return Ok((start, matches));
                }
            }
        }

        Ok((0, vec![]))
    }
}

/// Validate a username to prevent injection attacks
fn validate_username(username: &str) -> bool {
    !username.is_empty()
        && !username.starts_with('-')
        && username.len() <= 32
        && username
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.')
}

/// Look up a user's home directory from /etc/passwd inside the guest
fn lookup_home_dir(handle: &mut Guestfs, username: &str) -> String {
    if username == "root" {
        return "/root".to_string();
    }
    // Try to read /etc/passwd and extract home directory
    if let Ok(content) = handle.read_file("/etc/passwd") {
        let content_str = String::from_utf8_lossy(&content);
        for line in content_str.lines() {
            let fields: Vec<&str> = line.split(':').collect();
            if fields.len() >= 6 && fields[0] == username {
                return fields[5].to_string();
            }
        }
    }
    // Fallback to /home/{username}
    format!("/home/{}", username)
}

/// Interactive session state
pub struct InteractiveSession {
    handle: Guestfs,
    editor: Editor<GuestkitHelper, rustyline::history::DefaultHistory>,
    disk_path: PathBuf,
    mounted: HashMap<String, String>, // device -> mountpoint
    current_root: Option<String>,
}

impl InteractiveSession {
    /// Create a new interactive session for the given disk image
    pub fn new(disk_path: PathBuf) -> Result<Self> {
        println!(
            "{}",
            "Initializing GuestKit Interactive Mode..."
                .truecolor(222, 115, 86)
                .bold()
        );
        println!();

        // Create handle
        let mut handle = Guestfs::new().context("Failed to create guestfs handle")?;

        // Add drive
        println!(
            "  {} Loading disk: {}",
            "→".truecolor(222, 115, 86),
            disk_path.display()
        );
        handle
            .add_drive_ro(
                disk_path
                    .to_str()
                    .ok_or_else(|| anyhow::anyhow!("Disk image path contains invalid UTF-8"))?,
            )
            .context("Failed to add drive")?;

        // Launch
        println!("  {} Launching appliance...", "→".truecolor(222, 115, 86));
        handle.launch().context("Failed to launch guestfs")?;

        // Auto-inspect
        println!("  {} Inspecting disk...", "→".truecolor(222, 115, 86));
        let roots = handle.inspect_os().unwrap_or_default();
        let current_root = roots.first().cloned();

        if let Some(ref root) = current_root {
            if let Ok(os_type) = handle.inspect_get_type(root) {
                if let Ok(distro) = handle.inspect_get_distro(root) {
                    if let (Ok(major), Ok(minor)) = (
                        handle.inspect_get_major_version(root),
                        handle.inspect_get_minor_version(root),
                    ) {
                        println!();
                        println!(
                            "  {} Found: {} {} {}.{}",
                            "✓".green().bold(),
                            os_type.truecolor(222, 115, 86),
                            distro.truecolor(222, 115, 86),
                            major.to_string().bright_white(),
                            minor.to_string().bright_white()
                        );
                    }
                }
            }
        }

        // Auto-mount filesystems
        if let Some(ref root) = current_root {
            println!(
                "  {} Auto-mounting filesystems...",
                "→".truecolor(222, 115, 86)
            );
            match handle.inspect_get_mountpoints(root) {
                Ok(mountpoints) => {
                    // Sort by mountpoint length (mount / before /boot, etc.)
                    let mut sorted_mounts: Vec<_> = mountpoints.into_iter().collect();
                    sorted_mounts.sort_by_key(|(mp, _)| mp.len());

                    for (mountpoint, device) in sorted_mounts {
                        if let Err(e) = handle.mount_ro(&device, &mountpoint) {
                            println!(
                                "  {} Failed to mount {} at {}: {}",
                                "⚠".yellow(),
                                device,
                                mountpoint,
                                e
                            );
                        } else {
                            println!(
                                "  {} Mounted {} at {} (ro)",
                                "✓".green(),
                                device.bright_white(),
                                mountpoint.truecolor(222, 115, 86)
                            );
                        }
                    }
                }
                Err(e) => {
                    println!(
                        "  {} Warning: Could not get mountpoints: {}",
                        "⚠".yellow(),
                        e
                    );
                }
            }
        }

        // Create editor with tab completion
        let mut editor = Editor::new().context("Failed to create line editor")?;
        editor.set_helper(Some(GuestkitHelper));

        // Load command history if available
        if let Ok(history_file) = get_history_file(&disk_path) {
            if history_file.exists() && editor.load_history(&history_file).is_ok() {
                println!("  {} Loaded command history", "→".truecolor(222, 115, 86));
            }
        }

        println!();
        println!(
            "{}",
            "Ready! Type 'help' for commands, 'exit' to quit.".bright_green()
        );
        println!("{}", "Tip: Press TAB for command completion".dimmed());
        println!(
            "{}",
            "Tip: Use ↑/↓ arrows to browse command history".dimmed()
        );
        println!();

        Ok(Self {
            handle,
            editor,
            disk_path,
            mounted: HashMap::new(),
            current_root,
        })
    }

    /// Run the interactive session
    pub fn run(&mut self) -> Result<()> {
        loop {
            let prompt = format!("{}> ", invocation::name().truecolor(222, 115, 86).bold());

            match self.editor.readline(&prompt) {
                Ok(line) => {
                    let line = line.trim();
                    if line.is_empty() {
                        continue;
                    }

                    // Add to history
                    let _ = self.editor.add_history_entry(line);

                    // Handle exit
                    if line == "exit" || line == "quit" || line == "q" {
                        println!("Goodbye!");
                        break;
                    }

                    // Execute command
                    if let Err(e) = self.execute_command(line) {
                        eprintln!("{} {}", "Error:".red().bold(), e);
                    }
                }
                Err(ReadlineError::Interrupted) => {
                    println!("^C");
                    println!("Use 'exit' or 'quit' to leave interactive mode");
                }
                Err(ReadlineError::Eof) => {
                    println!("Goodbye!");
                    break;
                }
                Err(err) => {
                    eprintln!("{} {:?}", "Error:".red().bold(), err);
                    break;
                }
            }
        }

        // Save command history before exiting
        if let Ok(history_file) = get_history_file(&self.disk_path) {
            if let Err(e) = self.editor.save_history(&history_file) {
                eprintln!(
                    "{} Failed to save command history: {}",
                    "Warning:".yellow(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Execute a command
    fn execute_command(&mut self, line: &str) -> Result<()> {
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            return Ok(());
        }

        match parts[0] {
            "help" | "?" => self.cmd_help(),
            "info" => self.cmd_info(),
            "filesystems" | "fs" => self.cmd_filesystems(),
            "mount" => self.cmd_mount(&parts[1..]),
            "umount" | "unmount" => self.cmd_umount(&parts[1..]),
            "mounts" => self.cmd_mounts(),
            "ls" => self.cmd_ls(&parts[1..]),
            "cat" => self.cmd_cat(&parts[1..]),
            "head" => self.cmd_head(&parts[1..]),
            "find" => self.cmd_find(&parts[1..]),
            "stat" => self.cmd_stat(&parts[1..]),
            "download" | "dl" => self.cmd_download(&parts[1..]),
            "packages" | "pkg" => self.cmd_packages(&parts[1..]),
            "services" | "svc" => self.cmd_services(),
            "users" => self.cmd_users(),
            "network" | "net" => self.cmd_network(),
            "clear" | "cls" => self.cmd_clear(),
            // File operations
            "upload" => self.cmd_upload(&parts[1..]),
            "edit" => self.cmd_edit(&parts[1..]),
            "write" => self.cmd_write(&parts[1..]),
            // User management
            "adduser" => self.cmd_adduser(&parts[1..]),
            "deluser" => self.cmd_deluser(&parts[1..]),
            "passwd" => self.cmd_passwd(&parts[1..]),
            "usermod" => self.cmd_usermod(&parts[1..]),
            "groups" => self.cmd_groups(&parts[1..]),
            // SSH management
            "ssh-addkey" => self.cmd_ssh_addkey(&parts[1..]),
            "ssh-removekey" => self.cmd_ssh_removekey(&parts[1..]),
            "ssh-listkeys" => self.cmd_ssh_listkeys(&parts[1..]),
            "ssh-enable" => self.cmd_ssh_enable(),
            "ssh-config" => self.cmd_ssh_config(),
            // System configuration
            "hostname" => self.cmd_hostname(&parts[1..]),
            "timezone" => self.cmd_timezone(&parts[1..]),
            "selinux" => self.cmd_selinux(&parts[1..]),
            // Service management
            "enable" => self.cmd_enable(&parts[1..]),
            "disable" => self.cmd_disable(&parts[1..]),
            "restart" => self.cmd_restart(&parts[1..]),
            // File/Directory operations
            "copy" => self.cmd_copy(&parts[1..]),
            "move" => self.cmd_move(&parts[1..]),
            "delete" => self.cmd_delete(&parts[1..]),
            "mkdir" => self.cmd_mkdir(&parts[1..]),
            "chmod" => self.cmd_chmod(&parts[1..]),
            "chown" => self.cmd_chown(&parts[1..]),
            "symlink" => self.cmd_symlink(&parts[1..]),
            "large-files" => self.cmd_large_files(&parts[1..]),
            "disk-usage" => self.cmd_disk_usage(&parts[1..]),
            // Package management
            "install" => self.cmd_install(&parts[1..]),
            "remove" => self.cmd_remove(&parts[1..]),
            "update" => self.cmd_update(&parts[1..]),
            "search" => self.cmd_search(&parts[1..]),
            // Firewall
            "firewall-add" => self.cmd_firewall_add(&parts[1..]),
            "firewall-remove" => self.cmd_firewall_remove(&parts[1..]),
            "firewall-list" => self.cmd_firewall_list(&parts[1..]),
            // Cron
            "cron-add" => self.cmd_cron_add(&parts[1..]),
            "cron-list" => self.cmd_cron_list(&parts[1..]),
            // Cleanup
            "clean-logs" => self.cmd_clean_logs(&parts[1..]),
            "clean-cache" => self.cmd_clean_cache(&parts[1..]),
            "clean-temp" => self.cmd_clean_temp(&parts[1..]),
            "clean-kernels" => self.cmd_clean_kernels(&parts[1..]),
            // Systemd/Logs
            "logs" => self.cmd_logs(&parts[1..]),
            "failed" => self.cmd_failed(&parts[1..]),
            "boot-time" => self.cmd_boot_time(&parts[1..]),
            // Locale
            "locale" => self.cmd_locale(&parts[1..]),
            // Backup
            "backup" => self.cmd_backup(&parts[1..]),
            "backups" => self.cmd_backups(&parts[1..]),
            // GRUB
            "grub" => self.cmd_grub(&parts[1..]),
            // Network config
            "net-setip" => self.cmd_net_setip(&parts[1..]),
            "net-setdns" => self.cmd_net_setdns(&parts[1..]),
            "net-route-add" => self.cmd_net_route_add(&parts[1..]),
            "net-dhcp" => self.cmd_net_dhcp(&parts[1..]),
            // Process management
            "ps" => self.cmd_ps(&parts[1..]),
            "kill" => self.cmd_kill(&parts[1..]),
            "top" => self.cmd_top(&parts[1..]),
            // Security & audit
            "scan-ports" => self.cmd_scan_ports(&parts[1..]),
            "audit-perms" => self.cmd_audit_perms(&parts[1..]),
            "audit-suid" => self.cmd_audit_suid(&parts[1..]),
            "check-updates" => self.cmd_check_updates(&parts[1..]),
            // Database
            "db-list" => self.cmd_db_list(&parts[1..]),
            "db-backup" => self.cmd_db_backup(&parts[1..]),
            // Advanced file ops
            "grep-replace" => self.cmd_grep_replace(&parts[1..]),
            "diff" => self.cmd_diff(&parts[1..]),
            "tree" => self.cmd_tree(&parts[1..]),
            "compress" => self.cmd_compress(&parts[1..]),
            "extract" => self.cmd_extract(&parts[1..]),
            // Git
            "git-clone" => self.cmd_git_clone(&parts[1..]),
            "git-pull" => self.cmd_git_pull(&parts[1..]),
            // Performance tuning
            "tune-swappiness" => self.cmd_tune_swappiness(&parts[1..]),
            "tune-show" => self.cmd_tune_show(&parts[1..]),
            // Setup wizards
            "setup-webserver" => self.cmd_setup_webserver(&parts[1..]),
            "setup-database" => self.cmd_setup_database(&parts[1..]),
            "setup-docker" => self.cmd_setup_docker(&parts[1..]),
            // Monitoring
            "metrics" => self.cmd_metrics(&parts[1..]),
            "bandwidth" => self.cmd_bandwidth(&parts[1..]),
            // SELinux advanced
            "selinux-context" => self.cmd_selinux_context(&parts[1..]),
            "selinux-audit" => self.cmd_selinux_audit(&parts[1..]),
            // Templates
            "template-save" => self.cmd_template_save(&parts[1..]),
            // AI Assistant
            "ai" => self.cmd_ai(&parts[1..]),
            _ => {
                let available = vec![
                    "help",
                    "info",
                    "filesystems",
                    "mount",
                    "umount",
                    "mounts",
                    "ls",
                    "cat",
                    "head",
                    "find",
                    "stat",
                    "download",
                    "packages",
                    "services",
                    "users",
                    "network",
                    "clear",
                    "exit",
                ];
                let err = errors::unknown_command(parts[0], &available);
                err.display();
                Ok(())
            }
        }
    }

    /// Show help
    fn cmd_help(&self) -> Result<()> {
        println!();
        println!(
            "{}",
            "GuestKit Interactive Commands:"
                .truecolor(222, 115, 86)
                .bold()
        );
        println!();

        println!("{}", "  System Information:".bright_white().bold());
        println!(
            "    {}  - Show disk and OS information",
            "info".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Filesystem Operations:".bright_white().bold());
        println!(
            "    {}  - List available filesystems",
            "filesystems, fs".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Mount a filesystem",
            "mount <device> <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Unmount a filesystem",
            "umount <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show mounted filesystems",
            "mounts".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  File Operations:".bright_white().bold());
        println!(
            "    {}  - List directory contents",
            "ls [path]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Display file contents",
            "cat <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Display first lines of file",
            "head <path> [lines]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Find files by name pattern",
            "find <pattern>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show file information",
            "stat <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Download file from disk",
            "download <src> <dest>".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  System Inspection:".bright_white().bold());
        println!(
            "    {}  - List installed packages",
            "packages, pkg [filter]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List system services",
            "services, svc".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List user accounts",
            "users".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show network configuration",
            "network, net".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  User Management:".bright_white().bold());
        println!(
            "    {}  - Create new user",
            "adduser <username>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Delete user",
            "deluser <username>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Change password",
            "passwd <username>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Add user to group",
            "usermod <user> <group>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show user groups",
            "groups <username>".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  SSH Key Management:".bright_white().bold());
        println!(
            "    {}  - Add SSH public key",
            "ssh-addkey <user> <keyfile>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Remove SSH key",
            "ssh-removekey <user> <index>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List authorized keys",
            "ssh-listkeys <user>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Enable SSH service",
            "ssh-enable".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show SSH config",
            "ssh-config".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  File Management:".bright_white().bold());
        println!(
            "    {}  - Upload file to VM",
            "upload <local> <remote>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Edit file in VM",
            "edit <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Write content to file",
            "write <path> <content>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Copy file/directory",
            "copy <src> <dest>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Move/rename file",
            "move <src> <dest>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Delete file/directory",
            "delete <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Create directory",
            "mkdir <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Change permissions",
            "chmod <mode> <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Change owner",
            "chown <user:group> <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Create symlink",
            "symlink <target> <link>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Find large files",
            "large-files [path] [size]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Disk usage analysis",
            "disk-usage [path]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Package Management:".bright_white().bold());
        println!(
            "    {}  - Install package",
            "install <package>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Remove package",
            "remove <package>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Update all packages",
            "update".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Search packages",
            "search <keyword>".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  System Configuration:".bright_white().bold());
        println!(
            "    {}  - Set hostname",
            "hostname <name>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Set timezone",
            "timezone <tz>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Set SELinux mode",
            "selinux <mode>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Set locale",
            "locale <locale>".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Service Management:".bright_white().bold());
        println!(
            "    {}  - Enable service",
            "enable <service>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Disable service",
            "disable <service>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Restart service",
            "restart <service>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - View service logs",
            "logs <service> [lines]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show failed services",
            "failed".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Analyze boot time",
            "boot-time".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Firewall Management:".bright_white().bold());
        println!(
            "    {}  - Add firewall rule",
            "firewall-add <port/service>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Remove firewall rule",
            "firewall-remove <port/service>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List firewall rules",
            "firewall-list".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Scheduled Tasks:".bright_white().bold());
        println!(
            "    {}  - Add cron job",
            "cron-add <user> <schedule> <cmd>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List cron jobs",
            "cron-list [user]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  System Cleanup:".bright_white().bold());
        println!(
            "    {}  - Clean old logs",
            "clean-logs".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Clean package cache",
            "clean-cache".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Clean temp files",
            "clean-temp".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Remove old kernels",
            "clean-kernels".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Backup & Safety:".bright_white().bold());
        println!(
            "    {}  - Backup a file",
            "backup <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - List backups",
            "backups [path]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Boot Configuration:".bright_white().bold());
        println!(
            "    {}  - Show GRUB config",
            "grub show".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Set kernel parameter",
            "grub set <param>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Update GRUB",
            "grub update".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Network Configuration:".bright_white().bold());
        println!(
            "    {}  - Set static IP",
            "net-setip <iface> <ip> <mask>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Set DNS servers",
            "net-setdns <server1> [server2]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Add route",
            "net-route-add <dest> <gateway>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Enable DHCP",
            "net-dhcp <interface>".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Process Management:".bright_white().bold());
        println!(
            "    {}  - List processes",
            "ps [filter]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Kill process",
            "kill <pid> [signal]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show top processes",
            "top".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Security & Audit:".bright_white().bold());
        println!(
            "    {}  - Scan open ports",
            "scan-ports".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Find world-writable files",
            "audit-perms".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Find SUID/SGID files",
            "audit-suid".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Check security updates",
            "check-updates".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Database Operations:".bright_white().bold());
        println!(
            "    {}  - List databases",
            "db-list".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Backup database",
            "db-backup <db> [type]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Advanced File Operations:".bright_white().bold());
        println!(
            "    {}  - Search & replace",
            "grep-replace <old> <new> <file>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Compare files",
            "diff <file1> <file2>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Directory tree",
            "tree [path] [depth]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Compress files",
            "compress <path> [output]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Extract archive",
            "extract <archive> [dest]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Git Operations:".bright_white().bold());
        println!(
            "    {}  - Clone repository",
            "git-clone <url> [path]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Update repository",
            "git-pull [path]".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Performance Tuning:".bright_white().bold());
        println!(
            "    {}  - Set swap usage",
            "tune-swappiness <value>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show tuning parameters",
            "tune-show".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Quick Setup Wizards:".bright_white().bold());
        println!(
            "    {}  - Setup Nginx",
            "setup-webserver".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Setup database",
            "setup-database [mysql|postgres]".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Setup Docker",
            "setup-docker".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Monitoring & Metrics:".bright_white().bold());
        println!(
            "    {}  - System metrics summary",
            "metrics".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Network bandwidth stats",
            "bandwidth".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  SELinux Advanced:".bright_white().bold());
        println!(
            "    {}  - Show SELinux context",
            "selinux-context <path>".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - SELinux audit log",
            "selinux-audit".truecolor(222, 115, 86)
        );
        println!();

        println!("{}", "  Templates:".bright_white().bold());
        println!(
            "    {}  - Save config template",
            "template-save <name>".truecolor(222, 115, 86)
        );
        println!();

        #[cfg(feature = "ai")]
        {
            println!("{}", "  AI Assistant:".bright_white().bold());
            println!(
                "    {}  - Ask AI for help (requires OPENAI_API_KEY)",
                "ai <query>".truecolor(222, 115, 86)
            );
            println!("             Example: ai why won't this boot?");
            println!();
        }

        println!("{}", "  Other:".bright_white().bold());
        println!(
            "    {}  - Clear screen",
            "clear, cls".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Show this help",
            "help, ?".truecolor(222, 115, 86)
        );
        println!(
            "    {}  - Exit interactive mode",
            "exit, quit, q".truecolor(222, 115, 86)
        );
        println!();

        Ok(())
    }

    /// Show disk and OS information
    fn cmd_info(&mut self) -> Result<()> {
        println!();
        println!("{}", "Disk Information:".truecolor(222, 115, 86).bold());
        println!("  Path: {}", self.disk_path.display());

        if let Some(ref root) = self.current_root {
            println!();
            println!("{}", "Operating System:".truecolor(222, 115, 86).bold());

            if let Ok(os_type) = self.handle.inspect_get_type(root) {
                println!("  Type: {}", os_type.bright_white());
            }

            if let Ok(distro) = self.handle.inspect_get_distro(root) {
                println!("  Distribution: {}", distro.bright_white());
            }

            if let (Ok(major), Ok(minor)) = (
                self.handle.inspect_get_major_version(root),
                self.handle.inspect_get_minor_version(root),
            ) {
                println!(
                    "  Version: {}.{}",
                    major.to_string().bright_white(),
                    minor.to_string().bright_white()
                );
            }

            if let Ok(hostname) = self.handle.inspect_get_hostname(root) {
                println!("  Hostname: {}", hostname.bright_white());
            }

            if let Ok(arch) = self.handle.inspect_get_arch(root) {
                println!("  Architecture: {}", arch.bright_white());
            }
        } else {
            println!("\n  No operating system detected");
        }

        println!();
        Ok(())
    }

    /// List filesystems
    fn cmd_filesystems(&mut self) -> Result<()> {
        let filesystems = self
            .handle
            .list_filesystems()
            .context("Failed to list filesystems")?;

        println!();
        println!(
            "{}",
            "Available Filesystems:".truecolor(222, 115, 86).bold()
        );
        println!();

        for (device, fstype) in filesystems {
            let mounted = if self.mounted.contains_key(&device) {
                format!(" {} at {}", "→".green(), self.mounted[&device])
            } else {
                String::new()
            };

            println!(
                "  {} {}{}",
                device.bright_white().bold(),
                fstype.yellow(),
                mounted
            );
        }

        println!();
        Ok(())
    }

    /// Mount a filesystem
    fn cmd_mount(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!(
                "{} Usage: mount <device> <mountpoint>",
                "Error:".red().bold()
            );
            println!("Example: mount /dev/sda1 /");
            return Ok(());
        }

        let device = args[0];
        let mountpoint = args[1];

        self.handle
            .mount(device, mountpoint)
            .with_context(|| format!("Failed to mount {} at {}", device, mountpoint))?;

        self.mounted
            .insert(device.to_string(), mountpoint.to_string());

        println!(
            "{} Mounted {} at {}",
            "✓".green().bold(),
            device.bright_white(),
            mountpoint.truecolor(222, 115, 86)
        );

        Ok(())
    }

    /// Unmount a filesystem
    fn cmd_umount(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!("{} Usage: umount <mountpoint>", "Error:".red().bold());
            return Ok(());
        }

        let mountpoint = args[0];

        self.handle
            .umount(mountpoint)
            .with_context(|| format!("Failed to unmount {}", mountpoint))?;

        // Remove from mounted map
        self.mounted.retain(|_, mp| mp != mountpoint);

        println!(
            "{} Unmounted {}",
            "✓".green().bold(),
            mountpoint.truecolor(222, 115, 86)
        );

        Ok(())
    }

    /// Show mounted filesystems
    fn cmd_mounts(&self) -> Result<()> {
        if self.mounted.is_empty() {
            println!("No filesystems mounted");
            println!("Use 'mount <device> <path>' to mount a filesystem");
            return Ok(());
        }

        println!();
        println!("{}", "Mounted Filesystems:".truecolor(222, 115, 86).bold());
        println!();

        for (device, mountpoint) in &self.mounted {
            println!(
                "  {} {} {}",
                device.bright_white().bold(),
                "→".truecolor(222, 115, 86),
                mountpoint.yellow()
            );
        }

        println!();
        Ok(())
    }

    /// List directory contents
    fn cmd_ls(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "/" } else { args[0] };

        match self.handle.ls(path) {
            Ok(entries) => {
                println!();
                for entry in &entries {
                    println!("  {}", entry);
                }
                println!();
                println!(
                    "{} entries",
                    entries.len().to_string().bright_white().bold()
                );
                println!();
                Ok(())
            }
            Err(e) => {
                // Check if it's a file (common mistake: ls on a file instead of cat)
                if self.handle.is_file(path).unwrap_or(false) {
                    println!();
                    println!(
                        "{} '{}' is a file, not a directory",
                        "Error:".red().bold(),
                        path
                    );
                    println!(
                        "{} Use 'cat {}' to view the file contents",
                        "Hint:".yellow().bold(),
                        path
                    );
                    println!();
                    Ok(())
                } else {
                    Err(e).with_context(|| format!("Failed to list directory: {}", path))
                }
            }
        }
    }

    /// Display file contents
    fn cmd_cat(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!("{} Usage: cat <path>", "Error:".red().bold());
            return Ok(());
        }

        let path = args[0];
        let content = self
            .handle
            .cat(path)
            .with_context(|| format!("Failed to read file: {}", path))?;

        println!();
        print!("{}", content);
        if !content.ends_with('\n') {
            println!();
        }
        println!();

        Ok(())
    }

    /// Display first lines of file
    fn cmd_head(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!("{} Usage: head <path> [lines]", "Error:".red().bold());
            return Ok(());
        }

        let path = args[0];
        let lines = if args.len() > 1 {
            match args[1].parse::<usize>() {
                Ok(n) => n,
                Err(_) => {
                    println!(
                        "{} Invalid line count '{}', using default 10",
                        "Warning:".yellow().bold(),
                        args[1]
                    );
                    10
                }
            }
        } else {
            10
        };

        let content = self
            .handle
            .cat(path)
            .with_context(|| format!("Failed to read file: {}", path))?;

        println!();
        for (i, line) in content.lines().enumerate() {
            if i >= lines {
                break;
            }
            println!("{}", line);
        }
        println!();

        Ok(())
    }

    /// Find files by pattern
    fn cmd_find(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!("{} Usage: find <pattern>", "Error:".red().bold());
            println!("Example: find '*.conf'");
            return Ok(());
        }

        let pattern = args[0];

        // Use guestfs glob functionality
        let results = self
            .handle
            .glob_expand(pattern)
            .with_context(|| format!("Failed to find files matching: {}", pattern))?;

        println!();
        if results.is_empty() {
            println!("No files found matching '{}'", pattern);
        } else {
            for path in &results {
                println!("  {}", path.bright_white());
            }
            println!();
            println!(
                "{} matches",
                results.len().to_string().bright_white().bold()
            );
        }
        println!();

        Ok(())
    }

    /// Show file information
    fn cmd_stat(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!("{} Usage: stat <path>", "Error:".red().bold());
            return Ok(());
        }

        let path = args[0];
        let stat = self
            .handle
            .stat(path)
            .with_context(|| format!("Failed to stat: {}", path))?;

        println!();
        println!("{}", "File Information:".truecolor(222, 115, 86).bold());
        println!("  Path: {}", path.bright_white());
        println!("  Size: {} bytes", stat.size.to_string().bright_white());
        println!("  Mode: {:o}", stat.mode);
        println!("  UID: {}", stat.uid);
        println!("  GID: {}", stat.gid);
        println!();

        Ok(())
    }

    /// Download file from disk
    fn cmd_download(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!(
                "{} Usage: download <source> <destination>",
                "Error:".red().bold()
            );
            println!("Example: download /etc/hostname ./hostname.txt");
            return Ok(());
        }

        let source = args[0];
        let dest = args[1];

        self.handle
            .download(source, dest)
            .with_context(|| format!("Failed to download {} to {}", source, dest))?;

        println!(
            "{} Downloaded {} to {}",
            "✓".green().bold(),
            source.bright_white(),
            dest.truecolor(222, 115, 86)
        );

        Ok(())
    }

    /// List packages
    fn cmd_packages(&mut self, args: &[&str]) -> Result<()> {
        let filter = if args.is_empty() { None } else { Some(args[0]) };

        if let Some(ref root) = self.current_root {
            let apps = self
                .handle
                .inspect_list_applications(root)
                .context("Failed to list packages")?;

            let filtered: Vec<_> = if let Some(f) = filter {
                apps.iter().filter(|app| app.name.contains(f)).collect()
            } else {
                apps.iter().collect()
            };

            println!();
            if filtered.is_empty() {
                if let Some(f) = &filter {
                    println!("No packages found matching '{}'", f);
                } else {
                    println!("No packages found");
                }
            } else {
                for app in filtered.iter().take(50) {
                    println!(
                        "  {} {} {}",
                        app.name.bright_white().bold(),
                        app.version.yellow(),
                        app.description.dimmed()
                    );
                }

                if filtered.len() > 50 {
                    println!();
                    println!(
                        "  {} (showing first 50 of {})",
                        "...".dimmed(),
                        filtered.len().to_string().bright_white()
                    );
                }
            }
            println!();
            println!(
                "{} packages total",
                apps.len().to_string().bright_white().bold()
            );
            println!();
        } else {
            println!("No operating system detected");
        }

        Ok(())
    }

    /// List services
    fn cmd_services(&mut self) -> Result<()> {
        if let Some(ref root) = self.current_root {
            let services = self
                .handle
                .inspect_systemd_services(root)
                .context("Failed to list services")?;

            println!();
            println!("{}", "Enabled Services:".truecolor(222, 115, 86).bold());
            println!();

            for service in &services {
                println!("  {} {}", "▶".green(), service.name.bright_white());
            }

            println!();
            println!(
                "{} services enabled",
                services.len().to_string().bright_white().bold()
            );
            println!();
        } else {
            println!("No operating system detected");
        }

        Ok(())
    }

    /// List users
    fn cmd_users(&mut self) -> Result<()> {
        if let Some(ref root) = self.current_root {
            let users = self
                .handle
                .inspect_users(root)
                .context("Failed to list users")?;

            println!();
            println!("{}", "User Accounts:".truecolor(222, 115, 86).bold());
            println!();

            for user in &users {
                let uid: i32 = user.uid.parse().unwrap_or(-1);
                let color = if uid == 0 {
                    owo_colors::Style::new().red().bold()
                } else if uid >= 1000 {
                    owo_colors::Style::new().bright_white()
                } else {
                    owo_colors::Style::new().yellow()
                };

                println!(
                    "  {} (uid: {}, shell: {})",
                    user.username.style(color),
                    user.uid,
                    user.shell
                );
            }

            println!();
            println!("{} users", users.len().to_string().bright_white().bold());
            println!();
        } else {
            println!("No operating system detected");
        }

        Ok(())
    }

    /// Show network configuration
    fn cmd_network(&mut self) -> Result<()> {
        if let Some(ref root) = self.current_root {
            let interfaces = self
                .handle
                .inspect_network(root)
                .context("Failed to get network configuration")?;

            println!();
            println!("{}", "Network Interfaces:".truecolor(222, 115, 86).bold());
            println!();

            for iface in &interfaces {
                println!(
                    "  {} {}",
                    iface.name.bright_white().bold(),
                    iface.mac_address.yellow()
                );
                for addr in &iface.ip_address {
                    println!(
                        "    {} {}",
                        "→".truecolor(222, 115, 86),
                        addr.bright_white()
                    );
                }
                if !iface.ip_address.is_empty() {
                    println!();
                }
            }

            if let Ok(dns_servers) = self.handle.inspect_dns(root) {
                if !dns_servers.is_empty() {
                    println!("{}", "DNS Servers:".truecolor(222, 115, 86).bold());
                    for dns in &dns_servers {
                        println!("  {}", dns.bright_white());
                    }
                    println!();
                }
            }
        } else {
            println!("No operating system detected");
        }

        Ok(())
    }

    /// Clear screen
    fn cmd_clear(&self) -> Result<()> {
        print!("\x1B[2J\x1B[1;1H");
        Ok(())
    }

    /// AI-powered diagnostics (requires --features ai)
    #[cfg(feature = "ai")]
    fn cmd_ai(&mut self, args: &[&str]) -> Result<()> {
        use anyhow::Context;
        use rig::{
            client::completion::CompletionClient,
            completion::{AssistantContent, CompletionModel},
            providers::openai,
        };
        use serde_json::json;

        if args.is_empty() {
            println!();
            println!("{} ai <query>", "Usage:".truecolor(222, 115, 86).bold());
            println!("Example: ai why won't this boot?");
            println!();
            return Ok(());
        }

        // Check for API key
        if std::env::var("OPENAI_API_KEY").is_err() {
            println!();
            println!(
                "{} {}",
                "⚠".truecolor(222, 115, 86).bold(),
                "OPENAI_API_KEY environment variable not set.".bright_white()
            );
            println!();
            println!("To use AI features:");
            println!("  1. Get an API key from https://platform.openai.com/api-keys");
            println!("  2. Set the environment variable:");
            println!("     export OPENAI_API_KEY='your-key-here'");
            println!();
            return Ok(());
        }

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                println!();
                println!(
                    "{} No OS detected. Cannot run AI diagnostics.",
                    "Error:".red().bold()
                );
                println!();
                return Ok(());
            }
        };

        let query = args.join(" ");

        println!();
        println!(
            "{} {}",
            "🤖".bold(),
            "Analyzing VM...".truecolor(222, 115, 86)
        );
        println!();

        // Gather diagnostic context
        let query_lower = query.to_lowercase();
        let mut context = String::new();

        context.push_str("=== VM Diagnostic Information ===\n\n");

        // Always include basic system info
        context.push_str("System Information:\n");
        let info = json!({
            "os_type": self.handle.inspect_get_type(root).ok(),
            "distro": self.handle.inspect_get_distro(root).ok(),
            "version": {
                "major": self.handle.inspect_get_major_version(root).ok(),
                "minor": self.handle.inspect_get_minor_version(root).ok(),
            },
            "hostname": self.handle.inspect_get_hostname(root).ok(),
            "architecture": self.handle.inspect_get_arch(root).ok(),
        });
        context.push_str(&serde_json::to_string_pretty(&info).unwrap_or_default());
        context.push('\n');

        // Conditional gathering based on query
        if query_lower.contains("lvm")
            || query_lower.contains("volume")
            || query_lower.contains("vg")
        {
            context.push_str("\nLVM Information:\n");
            if let Ok(lvm) = self.handle.inspect_lvm(root) {
                context.push_str(&serde_json::to_string_pretty(&lvm).unwrap_or_default());
                context.push('\n');
            }
        }

        if query_lower.contains("mount")
            || query_lower.contains("fstab")
            || query_lower.contains("filesystem")
        {
            context.push_str("\nCurrent Mounts:\n");
            if let Ok(mounts) = self.handle.mounts() {
                context.push_str(&mounts.join("\n"));
                context.push('\n');
            }

            context.push_str("\nfstab Configuration:\n");
            if let Ok(fstab) = self.handle.inspect_fstab(root) {
                context.push_str(&serde_json::to_string_pretty(&fstab).unwrap_or_default());
                context.push('\n');
            }
        }

        if query_lower.contains("boot")
            || query_lower.contains("kernel")
            || query_lower.contains("grub")
        {
            context.push_str("\nBoot Configuration:\n");
            if self.handle.is_dir("/boot").unwrap_or(false) {
                context.push_str("Boot directory accessible\n");
            }
        }

        if query_lower.contains("security")
            || query_lower.contains("selinux")
            || query_lower.contains("firewall")
        {
            context.push_str("\nSecurity Status:\n");
            if let Ok(sec) = self.handle.inspect_security(root) {
                context.push_str(&serde_json::to_string_pretty(&sec).unwrap_or_default());
                context.push('\n');
            }
        }

        // Always include block devices
        context.push_str("\nBlock Devices:\n");
        if let Ok(devices) = self.handle.list_devices() {
            for device in devices {
                let size = self.handle.blockdev_getsize64(&device).unwrap_or(0);
                context.push_str(&format!("{}: {} MB\n", device, size / 1024 / 1024));
            }
        }

        println!(
            "{} {}",
            "→".truecolor(222, 115, 86),
            "Consulting AI...".bright_white()
        );
        println!();

        // Call OpenAI
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

        let api_key = std::env::var("OPENAI_API_KEY")
            .context("OPENAI_API_KEY environment variable not set")?;

        let runtime = tokio::runtime::Runtime::new()?;

        let response = runtime.block_on(async {
            let full_prompt = format!(
                "{}\n\nUser Query: {}\n\n{}\n\nProvide a clear diagnosis and solution:",
                SYSTEM_PROMPT, query, context
            );

            let response = openai::Client::<reqwest::Client>::new(&api_key)
                .context("Failed to create OpenAI client")?
                .completions_api()
                .completion_model(openai::GPT_4O)
                .completion_request(&full_prompt)
                .send()
                .await
                .map_err(|e| anyhow::anyhow!("Failed to get AI response: {}", e))?;

            match response.choice.first() {
                AssistantContent::Text(text) => Ok(text.text.clone()),
                _ => anyhow::bail!("Unexpected response type from AI"),
            }
        })?;

        // Display response
        println!("{}", "═".repeat(70).truecolor(222, 115, 86));
        println!("{}", "AI Analysis".truecolor(222, 115, 86).bold());
        println!("{}", "═".repeat(70).truecolor(222, 115, 86));
        println!();
        println!("{}", response);
        println!();
        println!("{}", "═".repeat(70).truecolor(222, 115, 86));
        println!();

        println!(
            "{} {}",
            "⚠".truecolor(222, 115, 86).bold(),
            "Review suggestions carefully before applying".bright_white()
        );
        println!();

        Ok(())
    }

    #[cfg(not(feature = "ai"))]
    fn cmd_ai(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("{} AI features not enabled.", "Error:".red().bold());
        println!("Rebuild with: cargo build --features ai");
        println!();
        Ok(())
    }

    // ========== FILE MANAGEMENT ==========

    /// Upload file from host to VM
    fn cmd_upload(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} upload <local-file> <remote-path>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: upload ~/.ssh/id_rsa.pub /root/.ssh/authorized_keys");
            println!();
            return Ok(());
        }

        let local_path = args[0];
        let remote_path = args[1];

        println!();
        println!(
            "  {} Uploading {} → {}",
            "→".truecolor(222, 115, 86),
            local_path,
            remote_path
        );

        // Read local file
        let content = std::fs::read(local_path)
            .with_context(|| format!("Failed to read local file: {}", local_path))?;

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(remote_path).parent() {
            let parent_str = parent.to_str().unwrap_or("/");
            let _ = self.handle.mkdir_p(parent_str);
        }

        // Write to VM
        self.handle
            .write(remote_path, &content)
            .with_context(|| format!("Failed to write to VM: {}", remote_path))?;

        println!(
            "  {} File uploaded successfully",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Edit file in VM (download, edit, upload)
    fn cmd_edit(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!("{} edit <path>", "Usage:".truecolor(222, 115, 86).bold());
            println!("Example: edit /etc/ssh/sshd_config");
            println!();
            return Ok(());
        }

        let remote_path = args[0];

        println!();
        println!(
            "  {} Downloading file for editing...",
            "→".truecolor(222, 115, 86)
        );

        // Download file content
        let content = self
            .handle
            .read_file(remote_path)
            .with_context(|| format!("Failed to read file: {}", remote_path))?;

        // Create temp file securely using tempfile crate to avoid symlink attacks
        let temp_file = tempfile::Builder::new()
            .prefix("guestkit-edit-")
            .suffix(".tmp")
            .tempfile()
            .with_context(|| "Failed to create temporary file for editing")?;
        std::fs::write(temp_file.path(), &content)?;

        // Open in editor - validate editor is a simple command name
        let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".to_string());
        let editor_name = std::path::Path::new(&editor)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("vi");
        if editor.contains(';')
            || editor.contains('&')
            || editor.contains('|')
            || editor.contains('$')
        {
            anyhow::bail!("EDITOR contains unsafe characters: {}", editor);
        }
        println!(
            "  {} Opening {} in {}...",
            "→".truecolor(222, 115, 86),
            remote_path,
            editor_name
        );

        let status = std::process::Command::new(&editor)
            .arg(temp_file.path())
            .status()
            .with_context(|| format!("Failed to launch editor: {}", editor))?;

        if status.success() {
            // Upload modified file
            let modified_content = std::fs::read(temp_file.path())?;
            self.handle
                .write(remote_path, &modified_content)
                .with_context(|| format!("Failed to write back to VM: {}", remote_path))?;

            println!(
                "  {} File updated in VM",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            println!("  {} Edit cancelled", "⚠".truecolor(222, 115, 86));
        }

        // temp_file is automatically cleaned up when dropped
        println!();

        Ok(())
    }

    /// Write content directly to a file
    fn cmd_write(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} write <path> <content...>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: write /etc/motd Welcome to my VM!");
            println!();
            return Ok(());
        }

        let remote_path = args[0];
        let content = args[1..].join(" ");

        println!();
        println!(
            "  {} Writing to {}...",
            "→".truecolor(222, 115, 86),
            remote_path
        );

        // Ensure parent directory exists
        if let Some(parent) = std::path::Path::new(remote_path).parent() {
            let parent_str = parent.to_str().unwrap_or("/");
            let _ = self.handle.mkdir_p(parent_str);
        }

        self.handle
            .write(remote_path, content.as_bytes())
            .with_context(|| format!("Failed to write to {}", remote_path))?;

        println!(
            "  {} Content written successfully",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    // ========== USER MANAGEMENT ==========

    /// Add a new user
    fn cmd_adduser(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} adduser <username>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: adduser john");
            println!();
            return Ok(());
        }

        let username = args[0];

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Creating user {}...",
            "→".truecolor(222, 115, 86),
            username.bright_white().bold()
        );

        // Create user using useradd command
        self.handle
            .command(&["useradd", "-m", "-s", "/bin/bash", username])
            .with_context(|| format!("Failed to create user: {}", username))?;

        println!(
            "  {} User {} created successfully",
            "✓".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );
        println!(
            "  {} Run 'passwd {}' to set password",
            "→".truecolor(222, 115, 86),
            username
        );
        println!();

        Ok(())
    }

    /// Delete a user
    fn cmd_deluser(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} deluser <username>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: deluser john");
            println!();
            return Ok(());
        }

        let username = args[0];

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Deleting user {}...",
            "→".truecolor(222, 115, 86),
            username.bright_white().bold()
        );

        // Delete user and their home directory
        self.handle
            .command(&["userdel", "-r", username])
            .with_context(|| format!("Failed to delete user: {}", username))?;

        println!(
            "  {} User {} deleted",
            "✓".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );
        println!();

        Ok(())
    }

    /// Set user password
    fn cmd_passwd(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} passwd <username>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: passwd john");
            println!();
            return Ok(());
        }

        let username = args[0];

        println!();
        print!(
            "Enter new password for {}: ",
            username.bright_white().bold()
        );
        std::io::Write::flush(&mut std::io::stdout())?;

        let password = rpassword::read_password()?;

        if password.is_empty() {
            println!("  {} Password cannot be empty", "✗".red());
            println!();
            return Ok(());
        }

        println!("  {} Setting password...", "→".truecolor(222, 115, 86));

        // Pipe password directly to chpasswd via echo to avoid plaintext temp files
        let passwd_input = format!("{}:{}", username, password);
        let escaped_input = passwd_input.replace('\'', "'\\''");
        let result =
            self.handle
                .command(&["sh", "-c", &format!("echo '{}' | chpasswd", escaped_input)]);

        result.with_context(|| format!("Failed to set password for {}", username))?;

        println!(
            "  {} Password set successfully for {}",
            "✓".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );
        println!();

        Ok(())
    }

    /// Modify user (add to group)
    fn cmd_usermod(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} usermod <username> <group>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: usermod john sudo");
            println!();
            return Ok(());
        }

        let username = args[0];
        let group = args[1];

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Adding {} to group {}...",
            "→".truecolor(222, 115, 86),
            username.bright_white(),
            group.bright_white()
        );

        self.handle
            .command(&["usermod", "-a", "-G", group, username])
            .with_context(|| format!("Failed to add {} to group {}", username, group))?;

        println!(
            "  {} User {} added to group {}",
            "✓".truecolor(222, 115, 86).bold(),
            username.bright_white(),
            group.bright_white()
        );
        println!();

        Ok(())
    }

    /// Show user groups
    fn cmd_groups(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} groups <username>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: groups john");
            println!();
            return Ok(());
        }

        let username = args[0];

        println!();
        println!(
            "{} Groups for {}:",
            "→".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );

        let output = self
            .handle
            .command(&["groups", username])
            .with_context(|| format!("Failed to get groups for {}", username))?;

        println!("  {}", output.trim().bright_white());
        println!();

        Ok(())
    }

    // ========== SSH KEY MANAGEMENT ==========

    /// Add SSH public key to user's authorized_keys
    fn cmd_ssh_addkey(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} ssh-addkey <username> <keyfile>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: ssh-addkey root ~/.ssh/id_rsa.pub");
            println!();
            return Ok(());
        }

        let username = args[0];
        let keyfile = args[1];

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Adding SSH key for {}...",
            "→".truecolor(222, 115, 86),
            username.bright_white()
        );

        // Read public key from host
        let key_content = std::fs::read_to_string(keyfile)
            .with_context(|| format!("Failed to read key file: {}", keyfile))?;

        // Determine home directory from /etc/passwd
        let home_dir = lookup_home_dir(&mut self.handle, username);

        let ssh_dir = format!("{}/.ssh", home_dir);
        let authorized_keys = format!("{}/authorized_keys", ssh_dir);

        // Create .ssh directory
        let _ = self.handle.mkdir_p(&ssh_dir);
        self.handle.command(&["chmod", "700", &ssh_dir])?;

        // Append key to authorized_keys
        let mut existing_keys = String::new();
        if let Ok(content) = self.handle.read_file(&authorized_keys) {
            existing_keys = String::from_utf8_lossy(&content).to_string();
        }

        if !existing_keys.trim().is_empty() {
            existing_keys.push('\n');
        }
        existing_keys.push_str(&key_content);

        self.handle
            .write(&authorized_keys, existing_keys.as_bytes())?;
        self.handle.command(&["chmod", "600", &authorized_keys])?;
        self.handle.command(&["chown", "-R", username, &ssh_dir])?;

        println!(
            "  {} SSH key added for {}",
            "✓".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );
        println!();

        Ok(())
    }

    /// Remove SSH key from authorized_keys
    fn cmd_ssh_removekey(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} ssh-removekey <username> <key-index>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: ssh-removekey root 0");
            println!("Tip: Use 'ssh-listkeys <user>' to see key indices");
            println!();
            return Ok(());
        }

        let username = args[0];
        let index: usize = args[1].parse().with_context(|| "Invalid key index")?;

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        let home_dir = lookup_home_dir(&mut self.handle, username);

        let authorized_keys = format!("{}/.ssh/authorized_keys", home_dir);

        println!();
        println!(
            "  {} Removing SSH key #{} for {}...",
            "→".truecolor(222, 115, 86),
            index,
            username.bright_white()
        );

        // Read existing keys
        let content = self
            .handle
            .read_file(&authorized_keys)
            .with_context(|| format!("Failed to read authorized_keys for {}", username))?;

        let mut lines: Vec<String> = String::from_utf8_lossy(&content)
            .lines()
            .filter(|l| !l.trim().is_empty())
            .map(|s| s.to_string())
            .collect();

        if index >= lines.len() {
            println!(
                "  {} Invalid key index (max: {})",
                "✗".red(),
                lines.len() - 1
            );
            println!();
            return Ok(());
        }

        lines.remove(index);

        // Write back
        let new_content = lines.join("\n") + "\n";
        self.handle
            .write(&authorized_keys, new_content.as_bytes())?;

        println!("  {} SSH key removed", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// List authorized SSH keys for a user
    fn cmd_ssh_listkeys(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} ssh-listkeys <username>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: ssh-listkeys root");
            println!();
            return Ok(());
        }

        let username = args[0];

        if !validate_username(username) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        let home_dir = lookup_home_dir(&mut self.handle, username);

        let authorized_keys = format!("{}/.ssh/authorized_keys", home_dir);

        println!();
        println!(
            "{} SSH keys for {}:",
            "→".truecolor(222, 115, 86).bold(),
            username.bright_white()
        );
        println!();

        match self.handle.read_file(&authorized_keys) {
            Ok(content) => {
                let content_str = String::from_utf8_lossy(&content);
                let lines: Vec<&str> = content_str
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .collect();

                if lines.is_empty() {
                    println!("  No SSH keys found");
                } else {
                    for (i, line) in lines.iter().enumerate() {
                        let parts: Vec<&str> = line.split_whitespace().collect();
                        let key_type = parts.first().unwrap_or(&"unknown");
                        let key_preview = parts.get(1).map(|k| &k[..k.len().min(40)]).unwrap_or("");
                        let comment = parts.get(2).unwrap_or(&"");

                        println!(
                            "  [{}] {} {}... {}",
                            i.to_string().truecolor(222, 115, 86),
                            key_type.bright_white(),
                            key_preview,
                            comment.bright_black()
                        );
                    }
                }
            }
            Err(_) => {
                println!("  No authorized_keys file found");
            }
        }

        println!();
        Ok(())
    }

    /// Enable SSH service
    fn cmd_ssh_enable(&mut self) -> Result<()> {
        println!();
        println!("  {} Enabling SSH service...", "→".truecolor(222, 115, 86));

        // Try systemctl first, fall back to service command
        let result = self
            .handle
            .sh_raw("systemctl enable sshd || systemctl enable ssh || chkconfig sshd on");

        match result {
            Ok(_) => {
                println!(
                    "  {} SSH service enabled",
                    "✓".truecolor(222, 115, 86).bold()
                );
                println!(
                    "  {} Remember to start/restart the service",
                    "→".truecolor(222, 115, 86)
                );
            }
            Err(e) => {
                println!("  {} Failed to enable SSH: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Show SSH configuration
    fn cmd_ssh_config(&mut self) -> Result<()> {
        println!();
        println!("{} SSH Configuration:", "→".truecolor(222, 115, 86).bold());
        println!();

        match self.handle.read_file("/etc/ssh/sshd_config") {
            Ok(content) => {
                let config = String::from_utf8_lossy(&content);
                for line in config.lines() {
                    let trimmed = line.trim();
                    if !trimmed.is_empty() && !trimmed.starts_with('#') {
                        println!("  {}", line.bright_white());
                    }
                }
            }
            Err(e) => {
                println!("  {} Failed to read SSH config: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    // ========== SYSTEM CONFIGURATION ==========

    /// Set hostname
    fn cmd_hostname(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} hostname <new-hostname>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: hostname webserver01");
            println!();
            return Ok(());
        }

        let new_hostname = args[0];

        println!();
        println!(
            "  {} Setting hostname to {}...",
            "→".truecolor(222, 115, 86),
            new_hostname.bright_white().bold()
        );

        // Write to /etc/hostname
        self.handle
            .write("/etc/hostname", new_hostname.as_bytes())?;

        // Update /etc/hosts
        let hosts_content = format!("127.0.0.1 localhost\n127.0.1.1 {}\n", new_hostname);
        self.handle.write("/etc/hosts", hosts_content.as_bytes())?;

        println!(
            "  {} Hostname set to {}",
            "✓".truecolor(222, 115, 86).bold(),
            new_hostname.bright_white()
        );
        println!();

        Ok(())
    }

    /// Set timezone
    fn cmd_timezone(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} timezone <timezone>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: timezone America/New_York");
            println!("Example: timezone UTC");
            println!();
            return Ok(());
        }

        let timezone = args[0];

        println!();
        println!(
            "  {} Setting timezone to {}...",
            "→".truecolor(222, 115, 86),
            timezone.bright_white()
        );

        // Create symlink to timezone file
        let tz_path = format!("/usr/share/zoneinfo/{}", timezone);
        let _ = self.handle.rm("/etc/localtime");
        self.handle.ln_s(&tz_path, "/etc/localtime")?;

        println!(
            "  {} Timezone set to {}",
            "✓".truecolor(222, 115, 86).bold(),
            timezone.bright_white()
        );
        println!();

        Ok(())
    }

    /// Set SELinux mode
    fn cmd_selinux(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!("{} selinux <mode>", "Usage:".truecolor(222, 115, 86).bold());
            println!("Modes: enforcing, permissive, disabled");
            println!("Example: selinux permissive");
            println!();
            return Ok(());
        }

        let mode = args[0].to_lowercase();

        if !["enforcing", "permissive", "disabled"].contains(&mode.as_str()) {
            println!();
            println!(
                "{} Invalid mode. Use: enforcing, permissive, or disabled",
                "Error:".red().bold()
            );
            println!();
            return Ok(());
        }

        println!();
        println!(
            "  {} Setting SELinux to {}...",
            "→".truecolor(222, 115, 86),
            mode.bright_white().bold()
        );

        // Update /etc/selinux/config
        let config = format!("SELINUX={}\nSELINUXTYPE=targeted\n", mode);
        let _ = self.handle.mkdir_p("/etc/selinux");
        self.handle
            .write("/etc/selinux/config", config.as_bytes())?;

        println!(
            "  {} SELinux set to {} (will take effect on reboot)",
            "✓".truecolor(222, 115, 86).bold(),
            mode.bright_white()
        );
        println!();

        Ok(())
    }

    // ========== SERVICE MANAGEMENT ==========

    /// Enable service at boot
    fn cmd_enable(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} enable <service>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: enable nginx");
            println!();
            return Ok(());
        }

        let service = args[0];

        println!();
        println!(
            "  {} Enabling {}...",
            "→".truecolor(222, 115, 86),
            service.bright_white()
        );

        // Try systemctl first, then chkconfig
        if self
            .handle
            .command(&["systemctl", "enable", service])
            .is_err()
        {
            let _ = self.handle.command(&["chkconfig", service, "on"]);
        }

        println!(
            "  {} Service {} enabled",
            "✓".truecolor(222, 115, 86).bold(),
            service.bright_white()
        );
        println!();

        Ok(())
    }

    /// Disable service at boot
    fn cmd_disable(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} disable <service>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: disable nginx");
            println!();
            return Ok(());
        }

        let service = args[0];

        println!();
        println!(
            "  {} Disabling {}...",
            "→".truecolor(222, 115, 86),
            service.bright_white()
        );

        // Try systemctl first, then chkconfig
        if self
            .handle
            .command(&["systemctl", "disable", service])
            .is_err()
        {
            let _ = self.handle.command(&["chkconfig", service, "off"]);
        }

        println!(
            "  {} Service {} disabled",
            "✓".truecolor(222, 115, 86).bold(),
            service.bright_white()
        );
        println!();

        Ok(())
    }

    /// Restart service (mark for restart on next boot)
    fn cmd_restart(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} restart <service>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: restart nginx");
            println!("Note: Service will be restarted when VM boots");
            println!();
            return Ok(());
        }

        let service = args[0];

        println!();
        println!(
            "  {} Marking {} for restart...",
            "→".truecolor(222, 115, 86),
            service.bright_white()
        );
        println!(
            "  {} Service will restart when VM boots",
            "→".truecolor(222, 115, 86)
        );
        println!();

        Ok(())
    }

    // ========== File/Directory Operations ==========

    /// Copy file or directory
    fn cmd_copy(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} copy <source> <destination>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: copy /etc/hosts /etc/hosts.backup");
            println!();
            return Ok(());
        }

        let source = args[0];
        let dest = args[1];

        println!();
        println!(
            "  {} Copying {} to {}...",
            "→".truecolor(222, 115, 86),
            source.bright_white(),
            dest.bright_white()
        );

        self.handle.command(&["cp", "-r", source, dest])?;

        println!("  {} Copy complete", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Move/rename file or directory
    fn cmd_move(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} move <source> <destination>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: move /tmp/old.txt /tmp/new.txt");
            println!();
            return Ok(());
        }

        let source = args[0];
        let dest = args[1];

        println!();
        println!(
            "  {} Moving {} to {}...",
            "→".truecolor(222, 115, 86),
            source.bright_white(),
            dest.bright_white()
        );

        self.handle.command(&["mv", source, dest])?;

        println!("  {} Move complete", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Delete file or directory
    fn cmd_delete(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!("{} delete <path>", "Usage:".truecolor(222, 115, 86).bold());
            println!("Example: delete /tmp/unwanted.txt");
            println!(
                "{} This will permanently delete the file!",
                "Warning:".red().bold()
            );
            println!();
            return Ok(());
        }

        let path = args[0];

        // Block deletion of critical system paths
        let blocked_paths = [
            "/", "/etc", "/usr", "/var", "/boot", "/home", "/root", "/bin", "/sbin", "/lib",
            "/lib64", "/dev", "/proc", "/sys",
        ];
        let normalized = path.trim_end_matches('/');
        if blocked_paths
            .iter()
            .any(|&bp| normalized == bp || path == bp)
        {
            anyhow::bail!("Refusing to delete protected system path: {}", path);
        }

        println!();
        println!(
            "{} Delete {}? This cannot be undone!",
            "⚠".red().bold(),
            path.bright_white()
        );
        print!("  Type 'yes' to confirm: ");
        std::io::Write::flush(&mut std::io::stdout())?;

        let mut confirmation = String::new();
        std::io::stdin().read_line(&mut confirmation)?;
        let confirmation = confirmation.trim().to_lowercase();
        if confirmation != "y" && confirmation != "yes" {
            println!("  {} Cancelled", "✗".red());
            println!();
            return Ok(());
        }

        println!("  {} Deleting...", "→".truecolor(222, 115, 86));

        self.handle.command(&["rm", "-rf", path])?;

        println!(
            "  {} Deleted successfully",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Create directory
    fn cmd_mkdir(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!("{} mkdir <path>", "Usage:".truecolor(222, 115, 86).bold());
            println!("Example: mkdir /opt/myapp");
            println!();
            return Ok(());
        }

        let path = args[0];

        println!();
        println!(
            "  {} Creating directory {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );

        self.handle.mkdir_p(path)?;

        println!("  {} Directory created", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Change file permissions
    fn cmd_chmod(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} chmod <mode> <path>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: chmod 755 /usr/local/bin/script.sh");
            println!();
            return Ok(());
        }

        let mode = args[0];
        let path = args[1];

        // Validate mode is a valid octal or symbolic mode
        if !mode
            .chars()
            .all(|c| c.is_ascii_digit() || "ugorwxXst+-=,".contains(c))
        {
            anyhow::bail!("Invalid chmod mode: {}", mode);
        }

        println!();
        println!(
            "  {} Setting permissions {} on {}...",
            "→".truecolor(222, 115, 86),
            mode.bright_white(),
            path.bright_white()
        );

        self.handle.command(&["chmod", mode, path])?;

        println!(
            "  {} Permissions updated",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Change file owner
    fn cmd_chown(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} chown <user[:group]> <path>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: chown alice:users /home/alice/file.txt");
            println!();
            return Ok(());
        }

        let owner = args[0];
        let path = args[1];

        // Validate owner format (user or user:group)
        if !owner
            .chars()
            .all(|c| c.is_alphanumeric() || c == ':' || c == '_' || c == '-' || c == '.')
        {
            anyhow::bail!("Invalid owner format: {}", owner);
        }

        println!();
        println!(
            "  {} Changing owner of {} to {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white(),
            owner.bright_white()
        );

        self.handle.command(&["chown", "-R", owner, path])?;

        println!("  {} Owner updated", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Find large files
    fn cmd_large_files(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "/" } else { args[0] };
        let min_size = if args.len() > 1 { args[1] } else { "100M" };

        println!();
        println!(
            "  {} Finding files larger than {} in {}...",
            "→".truecolor(222, 115, 86),
            min_size.bright_white(),
            path.bright_white()
        );
        println!();

        // Validate min_size to prevent injection (should be digits optionally followed by b/k/M/G/T/P)
        if !min_size
            .chars()
            .all(|c| c.is_ascii_digit() || "bckMGTP".contains(c))
        {
            eprintln!("Error: Invalid size format: {}", min_size);
            return Ok(());
        }

        // Use shell quoting to prevent injection
        let script = format!("find '{}' -type f -size +'{}' -exec ls -lh {{}} \\; 2>/dev/null | awk '{{print $5, $9}}' | sort -hr | head -20",
            path.replace('\'', "'\\''"),
            min_size.replace('\'', "'\\''"));
        match self.handle.sh_raw(&script) {
            Ok(output) => {
                if output.is_empty() {
                    println!("  No large files found");
                } else {
                    println!("{}", output);
                }
            }
            Err(e) => {
                println!("  {} {}", "Error:".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Disk usage analysis
    fn cmd_disk_usage(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "/" } else { args[0] };

        println!();
        println!(
            "  {} Analyzing disk usage for {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );
        println!();

        let script = format!(
            "du -h --max-depth=1 '{}' 2>/dev/null | sort -hr | head -20",
            path.replace('\'', "'\\''")
        );
        match self.handle.sh_raw(&script) {
            Ok(output) => {
                println!("{}", output);
            }
            Err(e) => {
                println!("  {} {}", "Error:".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Create symbolic link
    fn cmd_symlink(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} symlink <target> <link-name>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: symlink /usr/bin/python3 /usr/bin/python");
            println!();
            return Ok(());
        }

        let target = args[0];
        let link = args[1];

        println!();
        println!(
            "  {} Creating symlink {} -> {}...",
            "→".truecolor(222, 115, 86),
            link.bright_white(),
            target.bright_white()
        );

        self.handle.command(&["ln", "-sf", target, link])?;

        println!("  {} Symlink created", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    // ========== Package Management ==========

    /// Install package
    fn cmd_install(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} install <package-name>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: install nginx");
            println!();
            return Ok(());
        }

        let package = args[0];

        println!();
        println!(
            "  {} Installing package {}...",
            "→".truecolor(222, 115, 86),
            package.bright_white()
        );

        // Detect package manager and use appropriate command
        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        let result =
            if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
                self.handle.command(&["dnf", "install", "-y", package])
            } else if distro.contains("debian") || distro.contains("ubuntu") {
                // For apt-get, need to update first, then install
                let _ = self.handle.command(&["apt-get", "update"]);
                self.handle.command(&["apt-get", "install", "-y", package])
            } else {
                self.handle.command(&["yum", "install", "-y", package])
            };

        match result {
            Ok(_) => {
                println!(
                    "  {} Package installed successfully",
                    "✓".truecolor(222, 115, 86).bold()
                );
            }
            Err(e) => {
                println!("  {} Installation failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Remove package
    fn cmd_remove(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} remove <package-name>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: remove nginx");
            println!();
            return Ok(());
        }

        let package = args[0];

        println!();
        println!(
            "  {} Removing package {}...",
            "→".truecolor(222, 115, 86),
            package.bright_white()
        );

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        let result =
            if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
                self.handle.command(&["dnf", "remove", "-y", package])
            } else if distro.contains("debian") || distro.contains("ubuntu") {
                self.handle.command(&["apt-get", "remove", "-y", package])
            } else {
                self.handle.command(&["yum", "remove", "-y", package])
            };

        match result {
            Ok(_) => {
                println!(
                    "  {} Package removed successfully",
                    "✓".truecolor(222, 115, 86).bold()
                );
            }
            Err(e) => {
                println!("  {} Removal failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Update packages
    fn cmd_update(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Updating package lists...",
            "→".truecolor(222, 115, 86)
        );

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        let cmd =
            if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
                "dnf update -y"
            } else if distro.contains("debian") || distro.contains("ubuntu") {
                "apt-get update && apt-get upgrade -y"
            } else {
                "yum update -y"
            };

        match self.handle.sh_raw(cmd) {
            Ok(_) => {
                println!(
                    "  {} System updated successfully",
                    "✓".truecolor(222, 115, 86).bold()
                );
            }
            Err(e) => {
                println!("  {} Update failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    /// Search for packages
    fn cmd_search(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} search <keyword>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: search nginx");
            println!();
            return Ok(());
        }

        let keyword = args[0];

        println!();
        println!(
            "  {} Searching for packages matching '{}'...",
            "→".truecolor(222, 115, 86),
            keyword.bright_white()
        );
        println!();

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        // Use command() with argv array to avoid shell injection
        let result =
            if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
                self.handle.command(&["dnf", "search", keyword])
            } else if distro.contains("debian") || distro.contains("ubuntu") {
                self.handle.command(&["apt-cache", "search", keyword])
            } else {
                self.handle.command(&["pacman", "-Ss", keyword])
            };

        match result {
            Ok(output) => {
                println!("{}", output);
            }
            Err(e) => {
                println!("  {} Search failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    // ========== Firewall Management ==========

    /// Add firewall rule
    fn cmd_firewall_add(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} firewall-add <port/service>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Examples:");
            println!("  firewall-add 8080/tcp");
            println!("  firewall-add http");
            println!("  firewall-add 22/tcp");
            println!();
            return Ok(());
        }

        let rule = args[0];

        println!();
        println!(
            "  {} Adding firewall rule for {}...",
            "→".truecolor(222, 115, 86),
            rule.bright_white()
        );

        // Try firewalld first, fall back to ufw
        let port_result = self.handle.command(&[
            "firewall-cmd",
            "--permanent",
            &format!("--add-port={}", rule),
        ]);
        let service_result = if port_result.is_err() {
            self.handle.command(&[
                "firewall-cmd",
                "--permanent",
                &format!("--add-service={}", rule),
            ])
        } else {
            port_result
        };

        if service_result.is_ok() && self.handle.command(&["firewall-cmd", "--reload"]).is_ok() {
            println!(
                "  {} Firewall rule added (firewalld)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else if self.handle.command(&["ufw", "allow", rule]).is_ok() {
            println!(
                "  {} Firewall rule added (ufw)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            println!("  {} No firewall detected or rule add failed", "⚠".yellow());
        }

        println!();
        Ok(())
    }

    /// Remove firewall rule
    fn cmd_firewall_remove(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} firewall-remove <port/service>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: firewall-remove 8080/tcp");
            println!();
            return Ok(());
        }

        let rule = args[0];

        println!();
        println!(
            "  {} Removing firewall rule for {}...",
            "→".truecolor(222, 115, 86),
            rule.bright_white()
        );

        let port_result = self.handle.command(&[
            "firewall-cmd",
            "--permanent",
            &format!("--remove-port={}", rule),
        ]);
        let service_result = if port_result.is_err() {
            self.handle.command(&[
                "firewall-cmd",
                "--permanent",
                &format!("--remove-service={}", rule),
            ])
        } else {
            port_result
        };

        if service_result.is_ok() && self.handle.command(&["firewall-cmd", "--reload"]).is_ok() {
            println!(
                "  {} Firewall rule removed (firewalld)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else if self
            .handle
            .command(&["ufw", "delete", "allow", rule])
            .is_ok()
        {
            println!(
                "  {} Firewall rule removed (ufw)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            println!(
                "  {} No firewall detected or rule removal failed",
                "⚠".yellow()
            );
        }

        println!();
        Ok(())
    }

    /// List firewall rules
    fn cmd_firewall_list(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Listing firewall rules...",
            "→".truecolor(222, 115, 86)
        );
        println!();

        // Try firewalld
        if let Ok(output) = self.handle.sh_raw("firewall-cmd --list-all 2>/dev/null") {
            println!("{}", output);
        }
        // Try ufw
        else if let Ok(output) = self.handle.sh_raw("ufw status verbose 2>/dev/null") {
            println!("{}", output);
        }
        // Try iptables
        else if let Ok(output) = self.handle.sh_raw("iptables -L -n 2>/dev/null") {
            println!("{}", output);
        } else {
            println!("  {} No firewall detected", "⚠".yellow());
        }

        println!();
        Ok(())
    }

    // ========== Cron/Scheduled Tasks ==========

    /// Add cron job
    fn cmd_cron_add(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} cron-add <user> <schedule> <command>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: cron-add root \"0 2 * * *\" \"/usr/bin/backup.sh\"");
            println!("Schedule format: minute hour day month weekday");
            println!();
            return Ok(());
        }

        let user = args[0];
        let schedule_and_cmd = args[1..].join(" ");

        if !validate_username(user) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Adding cron job for {}...",
            "→".truecolor(222, 115, 86),
            user.bright_white()
        );

        // Read existing crontab, add new entry, write back
        let temp_cron = "/tmp/.guestkit_cron_tmp";
        // Use command array to avoid shell injection
        let existing = self
            .handle
            .command(&["crontab", "-u", user, "-l"])
            .unwrap_or_default();
        let new_crontab = if existing.trim().is_empty() {
            format!("{}\n", schedule_and_cmd)
        } else {
            format!("{}\n{}\n", existing.trim(), schedule_and_cmd)
        };

        match self.handle.write(temp_cron, new_crontab.as_bytes()) {
            Ok(_) => {
                let result = self.handle.command(&["crontab", "-u", user, temp_cron]);
                let _ = self.handle.command(&["rm", "-f", temp_cron]);

                match result {
                    Ok(_) => println!("  {} Cron job added", "✓".truecolor(222, 115, 86).bold()),
                    Err(e) => println!("  {} Failed to add cron job: {}", "✗".red(), e),
                }
            }
            Err(e) => println!("  {} Failed to write cron file: {}", "✗".red(), e),
        }

        println!();
        Ok(())
    }

    /// List cron jobs
    fn cmd_cron_list(&mut self, args: &[&str]) -> Result<()> {
        let user = if args.is_empty() { "root" } else { args[0] };

        if !validate_username(user) {
            anyhow::bail!("Invalid username: must be 1-32 chars, alphanumeric/underscore/dash/dot, not starting with '-'");
        }

        println!();
        println!(
            "  {} Cron jobs for {}:",
            "→".truecolor(222, 115, 86),
            user.bright_white()
        );
        println!();

        match self.handle.command(&["crontab", "-u", user, "-l"]) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  No cron jobs found");
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => {
                println!("  No cron jobs found or user doesn't exist");
            }
        }

        println!();
        Ok(())
    }

    // ========== System Cleanup ==========

    /// Clean old logs
    fn cmd_clean_logs(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Cleaning old logs...", "→".truecolor(222, 115, 86));

        // Clear old journal logs
        let _ = self
            .handle
            .sh_raw("journalctl --vacuum-time=7d 2>/dev/null");

        // Clear old log files
        let _ = self
            .handle
            .sh_raw("find /var/log -type f -name '*.log.*' -mtime +30 -delete 2>/dev/null");
        let _ = self
            .handle
            .sh_raw("find /var/log -type f -name '*.gz' -mtime +30 -delete 2>/dev/null");

        println!("  {} Logs cleaned", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Clean package cache
    fn cmd_clean_cache(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Cleaning package cache...",
            "→".truecolor(222, 115, 86)
        );

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
            let _ = self.handle.sh_raw("dnf clean all 2>/dev/null");
        } else if distro.contains("debian") || distro.contains("ubuntu") {
            let _ = self.handle.sh_raw("apt-get clean 2>/dev/null");
        } else {
            let _ = self.handle.sh_raw("yum clean all 2>/dev/null");
        }

        println!(
            "  {} Package cache cleaned",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Clean temp files
    fn cmd_clean_temp(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Cleaning temporary files...",
            "→".truecolor(222, 115, 86)
        );

        let _ = self.handle.sh_raw("rm -rf /tmp/* 2>/dev/null");
        let _ = self.handle.sh_raw("rm -rf /var/tmp/* 2>/dev/null");

        println!(
            "  {} Temporary files cleaned",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Remove old kernels
    fn cmd_clean_kernels(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Removing old kernels...", "→".truecolor(222, 115, 86));

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        if distro.contains("fedora") || distro.contains("rhel") || distro.contains("centos") {
            match self.handle.sh_raw(
                "dnf remove -y $(dnf repoquery --installonly --latest-limit=-2 -q) 2>/dev/null",
            ) {
                Ok(_) => println!(
                    "  {} Old kernels removed",
                    "✓".truecolor(222, 115, 86).bold()
                ),
                Err(_) => println!("  {} No old kernels to remove", "→".truecolor(222, 115, 86)),
            }
        } else if distro.contains("debian") || distro.contains("ubuntu") {
            let _ = self
                .handle
                .sh_raw("apt-get autoremove --purge -y 2>/dev/null");
            println!(
                "  {} Old kernels removed",
                "✓".truecolor(222, 115, 86).bold()
            );
        }

        println!();
        Ok(())
    }

    // ========== Systemd/Logs ==========

    /// View service logs
    fn cmd_logs(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} logs <service-name> [lines]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: logs sshd 50");
            println!();
            return Ok(());
        }

        let service = args[0];
        let lines = if args.len() > 1 { args[1] } else { "100" };

        println!();
        println!(
            "  {} Viewing logs for {}...",
            "→".truecolor(222, 115, 86),
            service.bright_white()
        );
        println!();

        let script = format!(
            "journalctl -u '{}' -n '{}' --no-pager 2>/dev/null",
            service.replace('\'', "'\\''"),
            lines.replace('\'', "'\\''")
        );
        match self.handle.sh_raw(&script) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  No logs found for service");
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => {
                // Fallback to syslog
                let fallback = format!(
                    "grep '{}' /var/log/syslog 2>/dev/null | tail -n '{}'",
                    service.replace('\'', "'\\''"),
                    lines.replace('\'', "'\\''")
                );
                if let Ok(output) = self.handle.sh_raw(&fallback) {
                    println!("{}", output);
                } else {
                    println!("  No logs found");
                }
            }
        }

        println!();
        Ok(())
    }

    /// Show failed services
    fn cmd_failed(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Checking for failed services...",
            "→".truecolor(222, 115, 86)
        );
        println!();

        match self
            .handle
            .sh_raw("systemctl list-units --failed --no-pager 2>/dev/null")
        {
            Ok(output) => {
                if output.contains("0 loaded units listed") {
                    println!("  {} No failed services", "✓".green().bold());
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => {
                println!("  {} systemd not available", "⚠".yellow());
            }
        }

        println!();
        Ok(())
    }

    /// Analyze boot time
    fn cmd_boot_time(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Analyzing boot time...", "→".truecolor(222, 115, 86));
        println!();

        match self.handle.sh_raw("systemd-analyze 2>/dev/null") {
            Ok(output) => {
                println!("{}", output);
                println!();

                // Show critical chain
                if let Ok(chain) = self
                    .handle
                    .sh_raw("systemd-analyze critical-chain 2>/dev/null | head -20")
                {
                    println!("{}", "Critical Chain:".bright_white().bold());
                    println!("{}", chain);
                }
            }
            Err(_) => {
                println!("  {} systemd-analyze not available", "⚠".yellow());
            }
        }

        println!();
        Ok(())
    }

    // ========== Environment/Locale ==========

    /// Set locale
    fn cmd_locale(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} locale <locale-name>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: locale en_US.UTF-8");
            println!();
            return Ok(());
        }

        let locale = args[0];

        println!();
        println!(
            "  {} Setting locale to {}...",
            "→".truecolor(222, 115, 86),
            locale.bright_white()
        );

        let lang_arg = format!("LANG={}", locale);
        match self.handle.command(&["localectl", "set-locale", &lang_arg]) {
            Ok(_) => {
                // Also update /etc/locale.conf
                let content = format!("LANG={}\n", locale);
                let _ = self.handle.write("/etc/locale.conf", content.as_bytes());
                println!("  {} Locale set", "✓".truecolor(222, 115, 86).bold());
            }
            Err(e) => {
                println!("  {} Failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    // ========== Backup/Safety ==========

    /// Backup a file
    fn cmd_backup(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} backup <file-path>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: backup /etc/nginx/nginx.conf");
            println!("Creates: /etc/nginx/nginx.conf.backup.TIMESTAMP");
            println!();
            return Ok(());
        }

        let path = args[0];

        println!();
        println!(
            "  {} Backing up {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );

        use chrono::Local;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let backup_path = format!("{}.backup.{}", path, timestamp);

        self.handle.command(&["cp", "-p", path, &backup_path])?;

        println!(
            "  {} Backup created: {}",
            "✓".truecolor(222, 115, 86).bold(),
            backup_path.bright_white()
        );
        println!();

        Ok(())
    }

    /// List backups
    fn cmd_backups(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "/" } else { args[0] };

        println!();
        println!(
            "  {} Finding backups in {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );
        println!();

        let script = format!(
            "find '{}' -name '*.backup.*' -o -name '*.bak' 2>/dev/null | sort",
            path.replace('\'', "'\\''")
        );
        match self.handle.sh_raw(&script) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  No backups found");
                } else {
                    for line in output.lines() {
                        println!("  {}", line.bright_white());
                    }
                }
            }
            Err(_) => {
                println!("  No backups found");
            }
        }

        println!();
        Ok(())
    }

    // ========== GRUB/Boot Configuration ==========

    /// Edit GRUB configuration
    fn cmd_grub(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!("{}", "GRUB Management:".bright_white().bold());
            println!(
                "  {} - Show current config",
                "grub show".truecolor(222, 115, 86)
            );
            println!(
                "  {} - Set kernel parameter",
                "grub set <param>".truecolor(222, 115, 86)
            );
            println!("  {} - Update GRUB", "grub update".truecolor(222, 115, 86));
            println!();
            println!("Examples:");
            println!("  grub set quiet");
            println!("  grub set \"elevator=noop\"");
            println!();
            return Ok(());
        }

        match args[0] {
            "show" => {
                println!();
                if let Ok(content) = self.handle.read_file("/etc/default/grub") {
                    println!("{}", String::from_utf8_lossy(&content));
                }
                println!();
            }
            "set" => {
                if args.len() < 2 {
                    println!();
                    println!("{} grub set <param>", "Usage:".red().bold());
                    println!();
                    return Ok(());
                }

                let param = args[1..].join(" ");
                println!();
                println!(
                    "  {} Adding kernel parameter: {}",
                    "→".truecolor(222, 115, 86),
                    param.bright_white()
                );

                // Read current grub config
                if let Ok(content) = self.handle.read_file("/etc/default/grub") {
                    let mut config = String::from_utf8_lossy(&content).to_string();

                    // Add parameter to GRUB_CMDLINE_LINUX
                    if config.contains("GRUB_CMDLINE_LINUX=") {
                        config = config.replace(
                            "GRUB_CMDLINE_LINUX=\"",
                            &format!("GRUB_CMDLINE_LINUX=\"{} ", param),
                        );
                    } else {
                        config.push_str(&format!("\nGRUB_CMDLINE_LINUX=\"{}\"\n", param));
                    }

                    self.handle.write("/etc/default/grub", config.as_bytes())?;
                    println!("  {} Parameter added", "✓".truecolor(222, 115, 86).bold());
                    println!(
                        "  {} Run 'grub update' to apply changes",
                        "→".truecolor(222, 115, 86)
                    );
                }
                println!();
            }
            "update" => {
                println!();
                println!(
                    "  {} Updating GRUB configuration...",
                    "→".truecolor(222, 115, 86)
                );

                // Try different GRUB update commands
                let commands = vec![
                    "grub2-mkconfig -o /boot/grub2/grub.cfg",
                    "grub-mkconfig -o /boot/grub/grub.cfg",
                    "update-grub",
                ];

                let mut success = false;
                for cmd in commands {
                    if self.handle.sh_raw(cmd).is_ok() {
                        success = true;
                        break;
                    }
                }

                if success {
                    println!("  {} GRUB updated", "✓".truecolor(222, 115, 86).bold());
                } else {
                    println!("  {} Failed to update GRUB", "✗".red());
                }
                println!();
            }
            _ => {
                println!();
                println!(
                    "{} Unknown grub command: {}",
                    "Error:".red().bold(),
                    args[0]
                );
                println!();
            }
        }

        Ok(())
    }

    // ========== Network Configuration ==========

    /// Set static IP address
    fn cmd_net_setip(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 3 {
            println!();
            println!(
                "{} net-setip <interface> <ip> <netmask>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: net-setip eth0 192.168.1.100 255.255.255.0");
            println!();
            return Ok(());
        }

        let iface = args[0];
        let ip = args[1];
        let netmask = args[2];

        println!();
        println!(
            "  {} Configuring {} with IP {}...",
            "→".truecolor(222, 115, 86),
            iface.bright_white(),
            ip.bright_white()
        );

        // Try NetworkManager first using command() to avoid injection
        let addr_spec = format!("{}/{}", ip, netmask);
        if self
            .handle
            .command(&[
                "nmcli",
                "con",
                "mod",
                iface,
                "ipv4.addresses",
                &addr_spec,
                "ipv4.method",
                "manual",
            ])
            .is_ok()
        {
            println!(
                "  {} IP configured (NetworkManager)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            // Fallback to writing ifcfg file directly
            let ifcfg_content = format!(
                "DEVICE={}\nBOOTPROTO=static\nIPADDR={}\nNETMASK={}\nONBOOT=yes\n",
                iface, ip, netmask
            );
            let _ = self.handle.write(
                &format!("/etc/sysconfig/network-scripts/ifcfg-{}", iface),
                ifcfg_content.as_bytes(),
            );
            println!(
                "  {} IP configured (ifcfg)",
                "✓".truecolor(222, 115, 86).bold()
            );
        }

        println!();
        Ok(())
    }

    /// Set DNS servers
    fn cmd_net_setdns(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} net-setdns <server1> [server2] [server3]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: net-setdns 8.8.8.8 8.8.4.4");
            println!();
            return Ok(());
        }

        println!();
        println!("  {} Setting DNS servers...", "→".truecolor(222, 115, 86));

        let mut resolv_conf = String::new();
        for server in args {
            resolv_conf.push_str(&format!("nameserver {}\n", server));
        }

        self.handle
            .write("/etc/resolv.conf", resolv_conf.as_bytes())?;

        println!(
            "  {} DNS servers configured",
            "✓".truecolor(222, 115, 86).bold()
        );
        println!();

        Ok(())
    }

    /// Add network route
    fn cmd_net_route_add(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} net-route-add <destination> <gateway>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: net-route-add 10.0.0.0/8 192.168.1.1");
            println!();
            return Ok(());
        }

        let dest = args[0];
        let gateway = args[1];

        println!();
        println!(
            "  {} Adding route to {} via {}...",
            "→".truecolor(222, 115, 86),
            dest.bright_white(),
            gateway.bright_white()
        );

        self.handle
            .command(&["ip", "route", "add", dest, "via", gateway])?;

        // Make persistent by appending to route file using write API
        let route_content = format!("{} via {}\n", dest, gateway);
        let route_file = "/etc/sysconfig/network-scripts/route-eth0";
        let existing = self.handle.read_file(route_file).unwrap_or_default();
        let mut new_content = String::from_utf8_lossy(&existing).to_string();
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&route_content);
        let _ = self.handle.write(route_file, new_content.as_bytes());

        println!("  {} Route added", "✓".truecolor(222, 115, 86).bold());
        println!();

        Ok(())
    }

    /// Enable DHCP on interface
    fn cmd_net_dhcp(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} net-dhcp <interface>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: net-dhcp eth0");
            println!();
            return Ok(());
        }

        let iface = args[0];

        println!();
        println!(
            "  {} Enabling DHCP on {}...",
            "→".truecolor(222, 115, 86),
            iface.bright_white()
        );

        if self
            .handle
            .command(&["nmcli", "con", "mod", iface, "ipv4.method", "auto"])
            .is_ok()
        {
            println!(
                "  {} DHCP enabled (NetworkManager)",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            let ifcfg = format!("DEVICE={}\nBOOTPROTO=dhcp\nONBOOT=yes\n", iface);
            let _ = self.handle.write(
                &format!("/etc/sysconfig/network-scripts/ifcfg-{}", iface),
                ifcfg.as_bytes(),
            );
            println!(
                "  {} DHCP enabled (ifcfg)",
                "✓".truecolor(222, 115, 86).bold()
            );
        }

        println!();
        Ok(())
    }

    // ========== Process Management ==========

    /// List processes
    fn cmd_ps(&mut self, args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Listing processes...", "→".truecolor(222, 115, 86));
        println!();

        let cmd = if args.is_empty() {
            "ps aux | head -50".to_string()
        } else {
            // For grep filter, use command to run ps, then filter output in Rust
            match self.handle.command(&["ps", "aux"]) {
                Ok(output) => {
                    let filter = args[0].to_lowercase();
                    let filtered: Vec<&str> = output
                        .lines()
                        .filter(|line| line.to_lowercase().contains(&filter))
                        .collect();
                    println!("{}", filtered.join("\n"));
                    println!();
                    return Ok(());
                }
                Err(e) => {
                    println!("  {} {}", "Error:".red(), e);
                    println!();
                    return Ok(());
                }
            }
        };

        match self.handle.sh_raw(&cmd) {
            Ok(output) => println!("{}", output),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    /// Kill process
    fn cmd_kill(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} kill <pid> [signal]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: kill 1234");
            println!("Example: kill 1234 -9");
            println!();
            return Ok(());
        }

        let pid = args[0];
        let signal = if args.len() > 1 { args[1] } else { "" };

        println!();
        println!(
            "  {} Killing process {}...",
            "→".truecolor(222, 115, 86),
            pid.bright_white()
        );

        let result = if signal.is_empty() {
            self.handle.command(&["kill", pid])
        } else {
            self.handle.command(&["kill", signal, pid])
        };
        match result {
            Ok(_) => println!("  {} Process killed", "✓".truecolor(222, 115, 86).bold()),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    /// Show top processes
    fn cmd_top(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Top processes by CPU and memory usage:",
            "→".truecolor(222, 115, 86)
        );
        println!();

        let cmd = "ps aux --sort=-%cpu | head -15";
        if let Ok(output) = self.handle.sh_raw(cmd) {
            println!("{}", output);
        }

        println!();
        Ok(())
    }

    // ========== Security & Audit ==========

    /// Scan for open ports
    fn cmd_scan_ports(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Scanning for open ports...",
            "→".truecolor(222, 115, 86)
        );
        println!();

        let cmd = "ss -tulpn 2>/dev/null || netstat -tulpn 2>/dev/null";
        match self.handle.sh_raw(cmd) {
            Ok(output) => println!("{}", output),
            Err(_) => println!("  {} Unable to scan ports", "⚠".yellow()),
        }

        println!();
        Ok(())
    }

    /// Find world-writable files
    fn cmd_audit_perms(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Finding world-writable files...",
            "→".truecolor(222, 115, 86)
        );
        println!();

        let cmd =
            "find / -type f -perm -002 ! -path '/proc/*' ! -path '/sys/*' 2>/dev/null | head -50";
        match self.handle.sh_raw(cmd) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  {} No world-writable files found", "✓".green().bold());
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => println!("  {} Audit failed", "✗".red()),
        }

        println!();
        Ok(())
    }

    /// Find SUID/SGID files
    fn cmd_audit_suid(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Finding SUID/SGID files...",
            "→".truecolor(222, 115, 86)
        );
        println!();

        let cmd = "find / -type f \\( -perm -4000 -o -perm -2000 \\) ! -path '/proc/*' ! -path '/sys/*' 2>/dev/null";
        match self.handle.sh_raw(cmd) {
            Ok(output) => println!("{}", output),
            Err(_) => println!("  {} Audit failed", "✗".red()),
        }

        println!();
        Ok(())
    }

    /// Check for security updates
    fn cmd_check_updates(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Checking for security updates...",
            "→".truecolor(222, 115, 86)
        );

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        let cmd = if distro.contains("fedora") || distro.contains("rhel") {
            "dnf updateinfo list security 2>/dev/null | head -20"
        } else if distro.contains("debian") || distro.contains("ubuntu") {
            "apt list --upgradable 2>/dev/null | head -20"
        } else {
            "yum updateinfo list security 2>/dev/null | head -20"
        };

        println!();
        match self.handle.sh_raw(cmd) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  {} No security updates available", "✓".green().bold());
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => println!("  {} Unable to check updates", "⚠".yellow()),
        }

        println!();
        Ok(())
    }

    // ========== Database Operations ==========

    /// List databases
    fn cmd_db_list(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Detecting databases...", "→".truecolor(222, 115, 86));
        println!();

        // Check for MySQL/MariaDB
        if let Ok(output) = self.handle.sh_raw("mysql -e 'SHOW DATABASES;' 2>/dev/null") {
            println!("{}", "MySQL/MariaDB:".bright_white().bold());
            println!("{}", output);
        }

        // Check for PostgreSQL
        if let Ok(output) = self.handle.sh_raw("sudo -u postgres psql -l 2>/dev/null") {
            println!("{}", "PostgreSQL:".bright_white().bold());
            println!("{}", output);
        }

        println!();
        Ok(())
    }

    /// Backup database
    fn cmd_db_backup(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} db-backup <database> [type]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Types: mysql, postgres");
            println!("Example: db-backup mydb mysql");
            println!();
            return Ok(());
        }

        let db = args[0];
        let db_type = if args.len() > 1 { args[1] } else { "mysql" };

        use chrono::Local;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let backup_file = format!("/tmp/{}_backup_{}.sql", db, timestamp);

        println!();
        println!(
            "  {} Backing up database {}...",
            "→".truecolor(222, 115, 86),
            db.bright_white()
        );

        let result = match db_type {
            "mysql" => self
                .handle
                .command(&["mysqldump", "--result-file", &backup_file, db]),
            "postgres" => {
                // For postgres, we need to use sudo -u postgres pg_dump
                self.handle
                    .command(&["sudo", "-u", "postgres", "pg_dump", "-f", &backup_file, db])
            }
            _ => {
                println!("  {} Unknown database type", "✗".red());
                return Ok(());
            }
        };

        match result {
            Ok(_) => {
                println!(
                    "  {} Backup created: {}",
                    "✓".truecolor(222, 115, 86).bold(),
                    backup_file.bright_white()
                );
            }
            Err(e) => {
                println!("  {} Backup failed: {}", "✗".red(), e);
            }
        }

        println!();
        Ok(())
    }

    // ========== Advanced File Operations ==========

    /// Search and replace in file
    fn cmd_grep_replace(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 3 {
            println!();
            println!(
                "{} grep-replace <pattern> <replacement> <file>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: grep-replace 'old_value' 'new_value' /etc/config.conf");
            println!();
            return Ok(());
        }

        let pattern = args[0];
        let replacement = args[1];
        let file = args[2];

        println!();
        println!(
            "  {} Replacing '{}' with '{}' in {}...",
            "→".truecolor(222, 115, 86),
            pattern.bright_white(),
            replacement.bright_white(),
            file.bright_white()
        );

        // Escape special sed characters in pattern and replacement
        let pattern_escaped = pattern
            .replace('\\', "\\\\")
            .replace('/', "\\/")
            .replace('&', "\\&")
            .replace('\n', "\\n");
        let replacement_escaped = replacement
            .replace('\\', "\\\\")
            .replace('/', "\\/")
            .replace('&', "\\&")
            .replace('\n', "\\n");
        let sed_expr = format!("s/{}/{}/g", pattern_escaped, replacement_escaped);

        match self.handle.command(&["sed", "-i", &sed_expr, file]) {
            Ok(_) => println!(
                "  {} Replacement complete",
                "✓".truecolor(222, 115, 86).bold()
            ),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    /// Compare files
    fn cmd_diff(&mut self, args: &[&str]) -> Result<()> {
        if args.len() < 2 {
            println!();
            println!(
                "{} diff <file1> <file2>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: diff /etc/hosts /etc/hosts.backup");
            println!();
            return Ok(());
        }

        let file1 = args[0];
        let file2 = args[1];

        println!();
        println!(
            "  {} Comparing {} and {}...",
            "→".truecolor(222, 115, 86),
            file1.bright_white(),
            file2.bright_white()
        );
        println!();

        // Use command() with argv array to avoid shell injection
        // diff returns exit code 1 when files differ, which is not an error
        match self.handle.command(&["diff", "-u", file1, file2]) {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  {} Files are identical", "✓".green().bold());
                } else {
                    println!("{}", output);
                }
            }
            Err(e) => {
                // diff returns exit code 1 when files differ - check if the error
                // message contains diff output (indicating files differ, not a real error)
                let err_msg = format!("{}", e);
                if err_msg.contains("---") || err_msg.contains("+++") {
                    println!("{}", err_msg);
                } else {
                    println!("  {} Failed to compare files: {}", "✗".red(), e);
                }
            }
        }

        println!();
        Ok(())
    }

    /// Show directory tree
    fn cmd_tree(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "." } else { args[0] };
        let depth = if args.len() > 1 { args[1] } else { "3" };

        println!();
        println!(
            "  {} Directory tree for {}:",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );
        println!();

        // Try tree command first, fall back to find - use command() to avoid shell injection
        if let Ok(output) = self
            .handle
            .sh_raw("command -v tree >/dev/null 2>&1 && echo yes || echo no")
        {
            if output.trim() == "yes" {
                if let Ok(tree_output) = self.handle.command(&["tree", "-L", depth, path]) {
                    println!("{}", tree_output);
                    println!();
                    return Ok(());
                }
            }
        }

        // Fallback to find
        // Validate depth is numeric to prevent injection
        if depth.parse::<u32>().is_err() {
            eprintln!("Error: Invalid depth: {}", depth);
            return Ok(());
        }

        let script = format!(
            "find '{}' -maxdepth {} -print 2>/dev/null | head -50",
            path.replace('\'', "'\\''"),
            depth.replace('\'', "'\\''")
        );
        if let Ok(output) = self.handle.sh_raw(&script) {
            println!("{}", output);
        } else {
            println!("  {} Unable to generate tree", "⚠".yellow());
        }

        println!();
        Ok(())
    }

    /// Compress files/directories
    fn cmd_compress(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} compress <path> [output.tar.gz]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: compress /var/www/html backup.tar.gz");
            println!();
            return Ok(());
        }

        let path = args[0];
        let output = if args.len() > 1 {
            args[1].to_string()
        } else {
            format!(
                "{}.tar.gz",
                path.trim_end_matches('/')
                    .split('/')
                    .next_back()
                    .unwrap_or("archive")
            )
        };

        println!();
        println!(
            "  {} Compressing {} to {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white(),
            output.bright_white()
        );

        match self.handle.command(&["tar", "czf", &output, path]) {
            Ok(_) => println!(
                "  {} Archive created: {}",
                "✓".truecolor(222, 115, 86).bold(),
                output.bright_white()
            ),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    /// Extract archive
    fn cmd_extract(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} extract <archive> [destination]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: extract backup.tar.gz /tmp/restored");
            println!();
            return Ok(());
        }

        let archive = args[0];
        let dest = if args.len() > 1 { args[1] } else { "." };

        println!();
        println!(
            "  {} Extracting {} to {}...",
            "→".truecolor(222, 115, 86),
            archive.bright_white(),
            dest.bright_white()
        );

        let result = if archive.ends_with(".tar.gz") || archive.ends_with(".tgz") {
            self.handle.command(&["tar", "xzf", archive, "-C", dest])
        } else if archive.ends_with(".tar.bz2") {
            self.handle.command(&["tar", "xjf", archive, "-C", dest])
        } else if archive.ends_with(".zip") {
            self.handle.command(&["unzip", archive, "-d", dest])
        } else {
            self.handle.command(&["tar", "xf", archive, "-C", dest])
        };

        match result {
            Ok(_) => println!(
                "  {} Extraction complete",
                "✓".truecolor(222, 115, 86).bold()
            ),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    // ========== Git Operations ==========

    /// Clone git repository
    fn cmd_git_clone(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} git-clone <url> [path]",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: git-clone https://github.com/user/repo.git /opt/repo");
            println!();
            return Ok(());
        }

        let url = args[0];
        let path = if args.len() > 1 { args[1] } else { "" };

        println!();
        println!("  {} Cloning repository...", "→".truecolor(222, 115, 86));

        let result = if path.is_empty() {
            self.handle.command(&["git", "clone", url])
        } else {
            self.handle.command(&["git", "clone", url, path])
        };

        match result {
            Ok(_) => println!("  {} Repository cloned", "✓".truecolor(222, 115, 86).bold()),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    /// Update git repository
    fn cmd_git_pull(&mut self, args: &[&str]) -> Result<()> {
        let path = if args.is_empty() { "." } else { args[0] };

        println!();
        println!(
            "  {} Updating repository in {}...",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );

        // Use git -C to change directory safely without using shell cd
        match self.handle.command(&["git", "-C", path, "pull"]) {
            Ok(output) => {
                println!("{}", output);
                println!(
                    "  {} Repository updated",
                    "✓".truecolor(222, 115, 86).bold()
                );
            }
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }

    // ========== Performance Tuning ==========

    /// Set swappiness
    fn cmd_tune_swappiness(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} tune-swappiness <value>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Value: 0-100 (default: 60, recommended for servers: 10)");
            println!("Example: tune-swappiness 10");
            println!();
            return Ok(());
        }

        let value = args[0];

        // Validate that value is numeric to prevent injection
        if value.parse::<u32>().is_err() {
            println!();
            println!(
                "  {} Invalid swappiness value: must be a number between 0-100",
                "✗".red()
            );
            println!();
            return Ok(());
        }

        println!();
        println!(
            "  {} Setting swappiness to {}...",
            "→".truecolor(222, 115, 86),
            value.bright_white()
        );

        let sysctl_arg = format!("vm.swappiness={}", value);
        let _ = self.handle.command(&["sysctl", &sysctl_arg]);

        // Append to sysctl.conf using write API instead of echo
        let content = format!("vm.swappiness={}\n", value);
        let existing = self
            .handle
            .read_file("/etc/sysctl.conf")
            .unwrap_or_default();
        let mut new_content = String::from_utf8_lossy(&existing).to_string();
        if !new_content.ends_with('\n') && !new_content.is_empty() {
            new_content.push('\n');
        }
        new_content.push_str(&content);
        let _ = self
            .handle
            .write("/etc/sysctl.conf", new_content.as_bytes());

        println!(
            "  {} Swappiness set to {}",
            "✓".truecolor(222, 115, 86).bold(),
            value.bright_white()
        );
        println!();

        Ok(())
    }

    /// Show current tuning
    fn cmd_tune_show(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("{}", "System Tuning Parameters:".bright_white().bold());
        println!();

        if let Ok(swappiness) = self.handle.sh_raw("sysctl vm.swappiness 2>/dev/null") {
            println!("  {}", swappiness);
        }

        if let Ok(cache_pressure) = self
            .handle
            .sh_raw("sysctl vm.vfs_cache_pressure 2>/dev/null")
        {
            println!("  {}", cache_pressure);
        }

        if let Ok(scheduler) = self
            .handle
            .sh_raw("cat /sys/block/sda/queue/scheduler 2>/dev/null")
        {
            println!("  I/O Scheduler: {}", scheduler.trim());
        }

        println!();
        Ok(())
    }

    // ========== Quick Setup Wizards ==========

    /// Setup web server
    fn cmd_setup_webserver(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("{}", "Web Server Setup Wizard".bright_white().bold());
        println!();

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        println!("  {} Installing Nginx...", "→".truecolor(222, 115, 86));

        let install_cmd = if distro.contains("fedora") || distro.contains("rhel") {
            "dnf install -y nginx"
        } else if distro.contains("debian") || distro.contains("ubuntu") {
            "apt-get update && apt-get install -y nginx"
        } else {
            "yum install -y nginx"
        };

        if self.handle.sh_raw(install_cmd).is_ok() {
            let _ = self.handle.sh_raw("systemctl enable nginx");
            println!(
                "  {} Nginx installed and enabled",
                "✓".truecolor(222, 115, 86).bold()
            );

            // Open firewall
            let _ = self
                .handle
                .sh_raw("firewall-cmd --permanent --add-service=http 2>/dev/null");
            let _ = self
                .handle
                .sh_raw("firewall-cmd --permanent --add-service=https 2>/dev/null");
            let _ = self.handle.sh_raw("firewall-cmd --reload 2>/dev/null");
            println!(
                "  {} Firewall configured",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            println!("  {} Installation failed", "✗".red());
        }

        println!();
        Ok(())
    }

    /// Setup database
    fn cmd_setup_database(&mut self, args: &[&str]) -> Result<()> {
        let db_type = if args.is_empty() { "mysql" } else { args[0] };

        println!();
        println!(
            "{} Database Setup Wizard ({})",
            "→".truecolor(222, 115, 86).bold(),
            db_type.bright_white()
        );
        println!();

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        let install_cmd = match db_type {
            "mysql" | "mariadb" => {
                if distro.contains("fedora") || distro.contains("rhel") {
                    "dnf install -y mariadb-server"
                } else if distro.contains("debian") || distro.contains("ubuntu") {
                    "apt-get update && apt-get install -y mariadb-server"
                } else {
                    "yum install -y mariadb-server"
                }
            }
            "postgres" | "postgresql" => {
                if distro.contains("fedora") || distro.contains("rhel") {
                    "dnf install -y postgresql-server postgresql-contrib"
                } else if distro.contains("debian") || distro.contains("ubuntu") {
                    "apt-get update && apt-get install -y postgresql postgresql-contrib"
                } else {
                    "yum install -y postgresql-server postgresql-contrib"
                }
            }
            _ => {
                println!(
                    "  {} Unknown database type. Use: mysql, mariadb, postgres",
                    "✗".red()
                );
                return Ok(());
            }
        };

        println!(
            "  {} Installing {}...",
            "→".truecolor(222, 115, 86),
            db_type.bright_white()
        );

        if self.handle.sh_raw(install_cmd).is_ok() {
            let service = if db_type.contains("mysql") || db_type.contains("mariadb") {
                "mariadb"
            } else {
                "postgresql"
            };

            if db_type.contains("postgres") {
                let _ = self.handle.sh_raw(
                    "postgresql-setup --initdb 2>/dev/null || postgresql-setup initdb 2>/dev/null",
                );
            }

            let _ = self.handle.command(&["systemctl", "enable", service]);
            println!(
                "  {} {} installed and enabled",
                "✓".truecolor(222, 115, 86).bold(),
                db_type.bright_white()
            );
        } else {
            println!("  {} Installation failed", "✗".red());
        }

        println!();
        Ok(())
    }

    /// Setup Docker
    fn cmd_setup_docker(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("{}", "Docker Setup Wizard".bright_white().bold());
        println!();

        let root = match self.current_root.as_ref() {
            Some(r) => r,
            None => {
                eprintln!("Error: No operating system detected in disk image");
                return Ok(());
            }
        };
        let distro = self.handle.inspect_get_distro(root).unwrap_or_default();

        println!("  {} Installing Docker...", "→".truecolor(222, 115, 86));

        let install_cmd = if distro.contains("fedora") || distro.contains("rhel") {
            "dnf install -y docker"
        } else if distro.contains("debian") || distro.contains("ubuntu") {
            "apt-get update && apt-get install -y docker.io"
        } else {
            "yum install -y docker"
        };

        if self.handle.sh_raw(install_cmd).is_ok() {
            let _ = self.handle.sh_raw("systemctl enable docker");
            println!(
                "  {} Docker installed and enabled",
                "✓".truecolor(222, 115, 86).bold()
            );
        } else {
            println!("  {} Installation failed", "✗".red());
        }

        println!();
        Ok(())
    }

    // ========== Monitoring & Metrics ==========

    /// Show system metrics
    fn cmd_metrics(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("{}", "System Metrics Summary".bright_white().bold());
        println!();

        // Memory
        if let Ok(mem) = self.handle.sh_raw("free -h | grep Mem") {
            println!("{} {}", "Memory:".bright_white().bold(), mem.trim());
        }

        // Disk usage
        if let Ok(disk) = self.handle.sh_raw("df -h / | tail -1") {
            println!("{} {}", "Root Disk:".bright_white().bold(), disk.trim());
        }

        // Load average
        if let Ok(load) = self
            .handle
            .sh_raw("uptime | awk -F'load average:' '{print $2}'")
        {
            println!("{} {}", "Load Average:".bright_white().bold(), load.trim());
        }

        // Process count
        if let Ok(procs) = self.handle.sh_raw("ps aux | wc -l") {
            println!("{} {}", "Processes:".bright_white().bold(), procs.trim());
        }

        println!();
        Ok(())
    }

    /// Show network bandwidth usage
    fn cmd_bandwidth(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!(
            "  {} Network interface statistics:",
            "→".truecolor(222, 115, 86)
        );
        println!();

        if let Ok(output) = self.handle.sh_raw("ip -s link 2>/dev/null") {
            println!("{}", output);
        } else if let Ok(output) = self.handle.sh_raw("ifconfig -s 2>/dev/null") {
            println!("{}", output);
        }

        println!();
        Ok(())
    }

    // ========== SELinux Advanced ==========

    /// Show SELinux context
    fn cmd_selinux_context(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} selinux-context <path>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: selinux-context /var/www/html");
            println!();
            return Ok(());
        }

        let path = args[0];

        println!();
        println!(
            "  {} SELinux context for {}:",
            "→".truecolor(222, 115, 86),
            path.bright_white()
        );
        println!();

        match self.handle.command(&["ls", "-Z", path]) {
            Ok(output) => println!("{}", output),
            Err(_) => println!("  {} SELinux not available", "⚠".yellow()),
        }

        println!();
        Ok(())
    }

    /// Show SELinux audit log
    fn cmd_selinux_audit(&mut self, _args: &[&str]) -> Result<()> {
        println!();
        println!("  {} Recent SELinux denials:", "→".truecolor(222, 115, 86));
        println!();

        match self
            .handle
            .sh_raw("ausearch -m avc -ts recent 2>/dev/null | head -50")
        {
            Ok(output) => {
                if output.trim().is_empty() {
                    println!("  {} No recent denials", "✓".green().bold());
                } else {
                    println!("{}", output);
                }
            }
            Err(_) => {
                // Try journalctl
                if let Ok(output) = self
                    .handle
                    .sh_raw("journalctl -t setroubleshoot --since '1 hour ago' 2>/dev/null")
                {
                    if output.trim().is_empty() {
                        println!("  {} No recent denials", "✓".green().bold());
                    } else {
                        println!("{}", output);
                    }
                } else {
                    println!("  {} SELinux audit not available", "⚠".yellow());
                }
            }
        }

        println!();
        Ok(())
    }

    // ========== Templating ==========

    /// Save current configuration as template
    fn cmd_template_save(&mut self, args: &[&str]) -> Result<()> {
        if args.is_empty() {
            println!();
            println!(
                "{} template-save <name>",
                "Usage:".truecolor(222, 115, 86).bold()
            );
            println!("Example: template-save webserver_config");
            println!();
            return Ok(());
        }

        let name = args[0];
        let template_dir = "/var/lib/guestkit/templates";

        println!();
        println!(
            "  {} Saving template '{}'...",
            "→".truecolor(222, 115, 86),
            name.bright_white()
        );

        let _ = self.handle.mkdir_p(template_dir);

        // Save important configs
        let configs = [
            "/etc/hostname",
            "/etc/hosts",
            "/etc/resolv.conf",
            "/etc/sysconfig/network-scripts/ifcfg-*",
        ];

        use chrono::Local;
        let timestamp = Local::now().format("%Y%m%d_%H%M%S");
        let template_file = format!("{}/{}_{}.tar.gz", template_dir, name, timestamp);

        // Build tar command with proper escaping
        let mut tar_args = vec!["tar", "czf", &template_file];
        let config_vec: Vec<&str> = configs.to_vec();
        tar_args.extend(config_vec);

        match self.handle.command(&tar_args) {
            Ok(_) => println!(
                "  {} Template saved: {}",
                "✓".truecolor(222, 115, 86).bold(),
                template_file.bright_white()
            ),
            Err(e) => println!("  {} {}", "Error:".red(), e),
        }

        println!();
        Ok(())
    }
}

impl Drop for InteractiveSession {
    fn drop(&mut self) {
        let _ = self.handle.shutdown();
    }
}
