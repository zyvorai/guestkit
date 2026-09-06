// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Batch script execution for guestkit CLI

use super::errors::builders as errors;
use crate::Guestfs;
use anyhow::{Context, Result};
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

/// Output redirection mode
#[derive(Debug, Clone)]
enum RedirectMode {
    Write,
    Append,
}

/// Output redirection configuration
#[derive(Debug, Clone)]
struct OutputRedirect {
    path: String,
    mode: RedirectMode,
}

/// Batch script executor
pub struct BatchExecutor {
    handle: Guestfs,
    _disk_path: PathBuf,
    mounted: HashMap<String, String>,
    current_root: Option<String>,
    fail_fast: bool,
    verbose: bool,
}

impl BatchExecutor {
    /// Create a new batch executor
    pub fn new(disk_path: PathBuf, fail_fast: bool, verbose: bool) -> Result<Self> {
        if verbose {
            println!(
                "{}",
                "Initializing batch executor...".truecolor(222, 115, 86)
            );
        }

        // Create handle
        let mut handle = Guestfs::new().context("Failed to create guestfs handle")?;

        // Add drive
        if verbose {
            println!(
                "  {} Loading disk: {}",
                "→".truecolor(222, 115, 86),
                disk_path.display()
            );
        }
        handle
            .add_drive_ro(disk_path.to_str().ok_or_else(|| {
                anyhow::anyhow!("Path contains invalid UTF-8: {}", disk_path.display())
            })?)
            .context("Failed to add drive")?;

        // Launch
        if verbose {
            println!("  {} Launching appliance...", "→".truecolor(222, 115, 86));
        }
        handle.launch().context("Failed to launch guestfs")?;

        // Auto-inspect
        let roots = match handle.inspect_os() {
            Ok(r) => r,
            Err(e) => {
                if verbose {
                    eprintln!("  {} OS inspection failed: {}", "!".yellow(), e);
                }
                Vec::new()
            }
        };
        let current_root = roots.first().cloned();

        if verbose && current_root.is_some() {
            if let Some(ref root) = current_root {
                if let Ok(os_type) = handle.inspect_get_type(root) {
                    if let Ok(distro) = handle.inspect_get_distro(root) {
                        println!(
                            "  {} Detected: {} {}",
                            "✓".green(),
                            os_type.truecolor(222, 115, 86),
                            distro.truecolor(222, 115, 86)
                        );
                    }
                }
            }
        }

        Ok(Self {
            handle,
            _disk_path: disk_path,
            mounted: HashMap::new(),
            current_root,
            fail_fast,
            verbose,
        })
    }

    /// Execute a script file
    pub fn execute_script<P: AsRef<Path>>(&mut self, script_path: P) -> Result<ExecutionReport> {
        let script_content = fs::read_to_string(&script_path)
            .with_context(|| format!("Failed to read script: {:?}", script_path.as_ref()))?;

        let mut report = ExecutionReport::new(script_path.as_ref().to_path_buf());

        for (line_num, line) in script_content.lines().enumerate() {
            let line = line.trim();

            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            report.total_commands += 1;

            if self.verbose {
                println!(
                    "\n{} {}",
                    format!("[{}]", line_num + 1).dimmed(),
                    line.truecolor(222, 115, 86)
                );
            }

            match self.execute_command(line) {
                Ok(()) => {
                    report.successful_commands += 1;
                    if self.verbose {
                        println!("  {}", "✓ Success".green());
                    }
                }
                Err(e) => {
                    report.failed_commands += 1;
                    report.errors.push(CommandError {
                        line_number: line_num + 1,
                        command: line.to_string(),
                        error: e.to_string(),
                    });

                    eprintln!("  {} {}", "✗ Error:".red(), e.to_string().red());

                    if self.fail_fast {
                        return Err(anyhow::anyhow!(
                            "Script execution failed at line {}: {}",
                            line_num + 1,
                            e
                        ));
                    }
                }
            }
        }

        Ok(report)
    }

    /// Execute a single command
    fn execute_command(&mut self, line: &str) -> Result<()> {
        // Handle output redirection
        let (command, output_redirect) = self.parse_redirection(line)?;
        let parts: Vec<&str> = command.split_whitespace().collect();

        if parts.is_empty() {
            return Ok(());
        }

        // Capture output if redirecting
        let result = if output_redirect.is_some() {
            self.execute_with_output_capture(&parts)
        } else {
            self.execute_command_parts(&parts)
        };

        // Write to file if needed
        if let Some(redirect) = output_redirect {
            if let Ok(ref output) = result {
                self.write_output(&redirect, output)?;
            }
        }

        result.map(|_| ())
    }

    /// Parse output redirection (>, >>)
    fn parse_redirection(&self, line: &str) -> Result<(String, Option<OutputRedirect>)> {
        // Check for append mode first (>>) before write mode (>)
        if let Some(pos) = line.find(" >> ") {
            let command = line[..pos].trim().to_string();
            let output_path = line[pos + 4..].trim().to_string();
            Ok((
                command,
                Some(OutputRedirect {
                    path: output_path,
                    mode: RedirectMode::Append,
                }),
            ))
        } else if let Some(pos) = line.find(" > ") {
            let command = line[..pos].trim().to_string();
            let output_path = line[pos + 3..].trim().to_string();
            Ok((
                command,
                Some(OutputRedirect {
                    path: output_path,
                    mode: RedirectMode::Write,
                }),
            ))
        } else {
            Ok((line.to_string(), None))
        }
    }

    /// Execute command and capture output
    fn execute_with_output_capture(&mut self, parts: &[&str]) -> Result<String> {
        match parts[0] {
            "ls" => {
                let path = parts.get(1).unwrap_or(&"/");
                let entries = self.handle.ls(path)?;
                Ok(entries.join("\n"))
            }
            "cat" => {
                if parts.len() < 2 {
                    return Err(errors::invalid_usage("cat", "cat <path>").into());
                }
                Ok(self.handle.cat(parts[1])?)
            }
            "packages" | "pkg" => {
                if let Some(ref root) = self.current_root {
                    let apps = self.handle.inspect_list_applications(root)?;
                    let filter = parts.get(1);

                    let output: Vec<String> = apps
                        .iter()
                        .filter(|app| {
                            if let Some(f) = filter {
                                app.name.contains(f)
                            } else {
                                true
                            }
                        })
                        .map(|app| format!("{} {} {}", app.name, app.version, app.description))
                        .collect();

                    Ok(output.join("\n"))
                } else {
                    Err(errors::os_detection_failed().into())
                }
            }
            "services" | "svc" => {
                if let Some(ref _root) = self.current_root {
                    let services = self.handle.list_services()?;
                    Ok(services.join("\n"))
                } else {
                    Err(errors::os_detection_failed().into())
                }
            }
            _ => {
                // For commands that don't produce text output
                self.execute_command_parts(parts)?;
                Ok(String::new())
            }
        }
    }

    /// Execute command parts (without output capture)
    fn execute_command_parts(&mut self, parts: &[&str]) -> Result<String> {
        match parts[0] {
            "mount" => {
                if parts.len() < 3 {
                    return Err(
                        errors::invalid_usage("mount", "mount <device> <mountpoint>").into(),
                    );
                }
                self.handle.mount(parts[1], parts[2])?;
                self.mounted
                    .insert(parts[1].to_string(), parts[2].to_string());
                Ok(format!("Mounted {} at {}", parts[1], parts[2]))
            }
            "umount" | "unmount" => {
                if parts.len() < 2 {
                    return Err(errors::invalid_usage("umount", "umount <mountpoint>").into());
                }
                self.handle.umount(parts[1])?;
                self.mounted.retain(|_, mp| mp != parts[1]);
                Ok(format!("Unmounted {}", parts[1]))
            }
            "download" | "dl" => {
                if parts.len() < 3 {
                    return Err(errors::invalid_usage(
                        "download",
                        "download <remote_path> <local_path>",
                    )
                    .into());
                }
                self.handle.download(parts[1], parts[2])?;
                Ok(format!("Downloaded {} to {}", parts[1], parts[2]))
            }
            "ls" => {
                let path = parts.get(1).unwrap_or(&"/");
                let entries = self.handle.ls(path)?;
                println!("{}", entries.join("\n"));
                Ok(format!("{} entries", entries.len()))
            }
            "cat" => {
                if parts.len() < 2 {
                    return Err(errors::invalid_usage("cat", "cat <path>").into());
                }
                let content = self.handle.cat(parts[1])?;
                println!("{}", content);
                Ok(String::new())
            }
            "find" => {
                if parts.len() < 2 {
                    return Err(errors::invalid_usage("find", "find <pattern>").into());
                }
                let files = self.handle.find(parts[1])?;
                println!("{}", files.join("\n"));
                Ok(format!("{} matches", files.len()))
            }
            _ => {
                let available = vec![
                    "mount", "umount", "ls", "cat", "find", "download", "packages", "services",
                ];
                Err(errors::unknown_command(parts[0], &available).into())
            }
        }
    }

    /// Write output to file
    fn write_output(&self, redirect: &OutputRedirect, content: &str) -> Result<()> {
        let file = match redirect.mode {
            RedirectMode::Write => fs::File::create(&redirect.path)
                .with_context(|| format!("Failed to create output file: {}", redirect.path))?,
            RedirectMode::Append => fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&redirect.path)
                .with_context(|| {
                    format!("Failed to open output file for append: {}", redirect.path)
                })?,
        };

        let mut writer = std::io::BufWriter::new(file);
        writer.write_all(content.as_bytes())?;
        writer.flush()?;

        if self.verbose {
            let mode_str = match redirect.mode {
                RedirectMode::Write => "Wrote",
                RedirectMode::Append => "Appended",
            };
            println!(
                "  {} {} output to {}",
                "→".truecolor(222, 115, 86),
                mode_str,
                redirect.path
            );
        }

        Ok(())
    }
}

/// Execution report
#[derive(Debug)]
pub struct ExecutionReport {
    pub script_path: PathBuf,
    pub total_commands: usize,
    pub successful_commands: usize,
    pub failed_commands: usize,
    pub errors: Vec<CommandError>,
}

impl ExecutionReport {
    fn new(script_path: PathBuf) -> Self {
        Self {
            script_path,
            total_commands: 0,
            successful_commands: 0,
            failed_commands: 0,
            errors: Vec::new(),
        }
    }

    /// Print the execution report
    pub fn print(&self) {
        println!("\n{}", "=".repeat(60).dimmed());
        println!("{}", "Batch Execution Report".bold());
        println!("{}", "=".repeat(60).dimmed());

        println!(
            "\n{}: {}",
            "Script".truecolor(222, 115, 86),
            self.script_path.display()
        );
        println!(
            "{}: {}",
            "Total Commands".truecolor(222, 115, 86),
            self.total_commands
        );
        println!("{}: {}", "Successful".green(), self.successful_commands);

        if self.failed_commands > 0 {
            println!("{}: {}", "Failed".red(), self.failed_commands);

            println!("\n{}", "Errors:".red().bold());
            for error in &self.errors {
                println!(
                    "  {} Line {}: {}",
                    "✗".red(),
                    error.line_number,
                    error.command.dimmed()
                );
                println!("    {}", error.error.red());
            }
        }

        println!("\n{}", "=".repeat(60).dimmed());

        if self.failed_commands == 0 {
            println!("{}", "✓ All commands executed successfully!".green().bold());
        } else {
            println!(
                "{}",
                format!("⚠ {} command(s) failed", self.failed_commands)
                    .yellow()
                    .bold()
            );
        }
    }

    /// Get exit code
    pub fn exit_code(&self) -> i32 {
        if self.failed_commands > 0 {
            1
        } else {
            0
        }
    }
}

/// Command execution error
#[derive(Debug)]
pub struct CommandError {
    pub line_number: usize,
    pub command: String,
    pub error: String,
}
