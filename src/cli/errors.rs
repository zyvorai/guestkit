// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Enhanced error messages with suggestions

use crate::cli::invocation;
use owo_colors::OwoColorize;
use std::fmt;

/// Enhanced error with helpful suggestions
pub struct EnhancedError {
    pub message: String,
    pub suggestion: Option<String>,
    pub examples: Vec<String>,
}

impl EnhancedError {
    /// Create a new enhanced error
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            suggestion: None,
            examples: Vec::new(),
        }
    }

    /// Add a suggestion
    pub fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    /// Add an example
    pub fn with_example(mut self, example: impl Into<String>) -> Self {
        self.examples.push(example.into());
        self
    }

    /// Add multiple examples
    pub fn with_examples(mut self, examples: Vec<String>) -> Self {
        self.examples = examples;
        self
    }

    /// Display the error with formatting
    pub fn display(&self) {
        // Error message
        eprintln!("{} {}", "Error:".red().bold(), self.message.red());

        // Suggestion
        if let Some(ref suggestion) = self.suggestion {
            eprintln!();
            eprintln!("{} {}", "Suggestion:".yellow().bold(), suggestion.yellow());
        }

        // Examples
        if !self.examples.is_empty() {
            eprintln!();
            eprintln!("{}", "Examples:".truecolor(222, 115, 86).bold());
            for example in &self.examples {
                eprintln!("  {}", example.dimmed());
            }
        }
    }
}

impl fmt::Display for EnhancedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl fmt::Debug for EnhancedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for EnhancedError {}

/// Common error builders for consistent error messages with suggestions
#[allow(dead_code)]
pub mod builders {
    use super::*;

    /// Invalid command usage
    pub fn invalid_usage(command: &str, usage: &str) -> EnhancedError {
        EnhancedError::new(format!("Invalid usage of '{}'", command))
            .with_suggestion(format!("Usage: {}", usage))
    }

    /// Unknown command
    pub fn unknown_command(command: &str, available: &[&str]) -> EnhancedError {
        let mut err = EnhancedError::new(format!("Unknown command: '{}'", command))
            .with_suggestion("Type 'help' to see all available commands");

        // Find similar commands (simple prefix matching)
        let similar: Vec<String> = available
            .iter()
            .filter(|&&cmd| {
                command.starts_with(&cmd[..1.min(cmd.len())])
                    || cmd.starts_with(&command[..1.min(command.len())])
            })
            .take(3)
            .map(|&s| s.to_string())
            .collect();

        if !similar.is_empty() {
            err = err.with_suggestion(format!("Did you mean: {}?", similar.join(", ")));
        }

        err
    }

    /// File not found
    pub fn file_not_found(path: &str) -> EnhancedError {
        EnhancedError::new(format!("File not found: {}", path))
            .with_suggestion("Check that the file path is correct and the file exists")
            .with_example("ls /path/to/verify")
    }

    /// Mount required
    pub fn mount_required() -> EnhancedError {
        EnhancedError::new("No filesystem is mounted")
            .with_suggestion("Mount a filesystem first before accessing files")
            .with_examples(vec![
                "mount /dev/sda1 /".to_string(),
                "mount /dev/vda1 /".to_string(),
            ])
    }

    /// OS detection failed
    pub fn os_detection_failed() -> EnhancedError {
        EnhancedError::new("No operating system detected in the disk image")
            .with_suggestion(
                "This command requires OS detection. The disk may be empty or corrupted",
            )
            .with_examples(vec![
                "# Try mounting manually:".to_string(),
                "filesystems".to_string(),
                "mount /dev/sda1 /".to_string(),
            ])
    }

    /// Permission denied
    pub fn permission_denied(operation: &str) -> EnhancedError {
        EnhancedError::new(format!("Permission denied: {}", operation))
            .with_suggestion("Try running with elevated privileges or check file permissions")
            .with_example(format!("sudo {} ...", invocation::name()))
    }

    /// Disk image not found
    pub fn disk_not_found(path: &str) -> EnhancedError {
        EnhancedError::new(format!("Disk image not found: {}", path))
            .with_suggestion("Verify the disk image path exists and is accessible")
            .with_examples(vec![
                "# Check if file exists:".to_string(),
                format!("ls -lh {}", path),
                "# Common locations:".to_string(),
                "ls /var/lib/libvirt/images/".to_string(),
                "ls ~/VirtualBox\\ VMs/".to_string(),
            ])
    }

    /// Invalid disk format
    pub fn invalid_format(path: &str) -> EnhancedError {
        EnhancedError::new(format!("Unable to recognize disk image format: {}", path))
            .with_suggestion("Supported formats: qcow2, raw, vmdk, vhd, vdi")
            .with_examples(vec![
                "# Check file format:".to_string(),
                "file disk.qcow2".to_string(),
                "qemu-img info disk.qcow2".to_string(),
            ])
    }

    /// Cache error
    pub fn cache_error(message: &str) -> EnhancedError {
        EnhancedError::new(format!("Cache error: {}", message))
            .with_suggestion("Try clearing the cache or running without --cache")
            .with_examples(vec![
                invocation::example("cache-clear"),
                format!(
                    "{}  # without --cache",
                    invocation::example("inspect vm.qcow2")
                ),
            ])
    }

    /// Export error
    pub fn export_error(format: &str, message: &str) -> EnhancedError {
        EnhancedError::new(format!("Export to {} failed: {}", format, message))
            .with_suggestion("Check that the output path is writable".to_string())
            .with_examples(vec![
                "# Verify output directory:".to_string(),
                "ls -ld $(dirname output.html)".to_string(),
                "# Try different output location:".to_string(),
                invocation::example("inspect vm.qcow2 --export html --export-output ~/report.html"),
            ])
    }

    /// Network error
    pub fn network_error(message: &str) -> EnhancedError {
        EnhancedError::new(format!("Network error: {}", message))
            .with_suggestion("Check your internet connection or try again later")
    }

    /// Timeout error
    pub fn timeout_error(operation: &str) -> EnhancedError {
        EnhancedError::new(format!("Operation timed out: {}", operation)).with_suggestion(
            "The operation took too long. Try with a smaller disk or increase timeout",
        )
    }

    /// Insufficient space
    pub fn insufficient_space(required: &str) -> EnhancedError {
        EnhancedError::new("Insufficient disk space")
            .with_suggestion(format!("At least {} of free space is required", required))
            .with_examples(vec![
                "# Check available space:".to_string(),
                "df -h /tmp".to_string(),
                "df -h ~".to_string(),
            ])
    }

    /// Dependency missing
    pub fn dependency_missing(dependency: &str) -> EnhancedError {
        EnhancedError::new(format!("Required dependency not found: {}", dependency))
            .with_suggestion(format!("Install {} to use this feature", dependency))
            .with_examples(vec![
                "# On Ubuntu/Debian:".to_string(),
                format!("sudo apt-get install {}", dependency),
                "# On Fedora/RHEL:".to_string(),
                format!("sudo dnf install {}", dependency),
            ])
    }

    /// Invalid argument
    pub fn invalid_argument(arg: &str, expected: &str) -> EnhancedError {
        EnhancedError::new(format!("Invalid argument: {}", arg))
            .with_suggestion(format!("Expected: {}", expected))
    }

    /// Feature not available
    pub fn feature_not_available(feature: &str, reason: &str) -> EnhancedError {
        EnhancedError::new(format!("Feature not available: {}", feature))
            .with_suggestion(reason.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_enhanced_error() {
        let err = EnhancedError::new("Test error")
            .with_suggestion("Try this instead")
            .with_example("example command");

        assert_eq!(err.message, "Test error");
        assert_eq!(err.suggestion, Some("Try this instead".to_string()));
        assert_eq!(err.examples.len(), 1);
    }

    #[test]
    fn test_unknown_command() {
        let err = builders::unknown_command("pac", &["packages", "pkg", "services"]);
        assert!(err.message.contains("Unknown command"));
    }

    #[test]
    fn test_enhanced_error_no_suggestion() {
        let err = EnhancedError::new("Error message");
        assert_eq!(err.message, "Error message");
        assert!(err.suggestion.is_none());
        assert_eq!(err.examples.len(), 0);
    }

    #[test]
    fn test_enhanced_error_with_examples() {
        let err = EnhancedError::new("Test")
            .with_examples(vec!["example 1".to_string(), "example 2".to_string()]);

        assert_eq!(err.examples.len(), 2);
        assert_eq!(err.examples[0], "example 1");
        assert_eq!(err.examples[1], "example 2");
    }

    #[test]
    fn test_enhanced_error_display_format() {
        let err = EnhancedError::new("Display test");
        let display_str = format!("{}", err);
        assert_eq!(display_str, "Display test");
    }

    #[test]
    fn test_enhanced_error_debug_format() {
        let err = EnhancedError::new("Debug test");
        let debug_str = format!("{:?}", err);
        assert_eq!(debug_str, "Debug test");
    }

    #[test]
    fn test_invalid_usage_error() {
        let err = builders::invalid_usage("ls", "ls [OPTIONS] [PATH]");
        assert!(err.message.contains("Invalid usage"));
        assert!(err.message.contains("ls"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_file_not_found_error() {
        let err = builders::file_not_found("/path/to/file");
        assert!(err.message.contains("File not found"));
        assert!(err.message.contains("/path/to/file"));
        assert!(err.suggestion.is_some());
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_mount_required_error() {
        let err = builders::mount_required();
        assert!(err.message.contains("No filesystem is mounted"));
        assert!(err.suggestion.is_some());
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_os_detection_failed_error() {
        let err = builders::os_detection_failed();
        assert!(err.message.contains("No operating system detected"));
        assert!(err.suggestion.is_some());
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_permission_denied_error() {
        let err = builders::permission_denied("reading file");
        assert!(err.message.contains("Permission denied"));
        assert!(err.message.contains("reading file"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_disk_not_found_error() {
        let err = builders::disk_not_found("/vm/disk.qcow2");
        assert!(err.message.contains("Disk image not found"));
        assert!(err.message.contains("/vm/disk.qcow2"));
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_invalid_format_error() {
        let err = builders::invalid_format("disk.img");
        assert!(err.message.contains("Unable to recognize"));
        assert!(err.suggestion.is_some());
        assert!(err.suggestion.as_ref().unwrap().contains("qcow2"));
    }

    #[test]
    fn test_cache_error() {
        let err = builders::cache_error("corrupted cache");
        assert!(err.message.contains("Cache error"));
        assert!(err.message.contains("corrupted cache"));
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_export_error() {
        let err = builders::export_error("HTML", "write failed");
        assert!(err.message.contains("Export to HTML failed"));
        assert!(err.message.contains("write failed"));
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_network_error() {
        let err = builders::network_error("connection refused");
        assert!(err.message.contains("Network error"));
        assert!(err.message.contains("connection refused"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_timeout_error() {
        let err = builders::timeout_error("inspection");
        assert!(err.message.contains("Operation timed out"));
        assert!(err.message.contains("inspection"));
        assert!(err.suggestion.is_some());
    }

    #[test]
    fn test_insufficient_space_error() {
        let err = builders::insufficient_space("10 GB");
        assert!(err.message.contains("Insufficient disk space"));
        assert!(err.suggestion.as_ref().unwrap().contains("10 GB"));
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_dependency_missing_error() {
        let err = builders::dependency_missing("qemu-img");
        assert!(err.message.contains("Required dependency not found"));
        assert!(err.message.contains("qemu-img"));
        assert!(!err.examples.is_empty());
    }

    #[test]
    fn test_invalid_argument_error() {
        let err = builders::invalid_argument("--format", "json, yaml, or text");
        assert!(err.message.contains("Invalid argument"));
        assert!(err.message.contains("--format"));
        assert!(err
            .suggestion
            .as_ref()
            .unwrap()
            .contains("json, yaml, or text"));
    }

    #[test]
    fn test_feature_not_available_error() {
        let err = builders::feature_not_available("AI assistant", "Requires --features ai");
        assert!(err.message.contains("Feature not available"));
        assert!(err.message.contains("AI assistant"));
        assert!(err
            .suggestion
            .as_ref()
            .unwrap()
            .contains("Requires --features ai"));
    }

    #[test]
    fn test_unknown_command_with_similar() {
        let err = builders::unknown_command("serv", &["services", "service", "shell"]);
        assert!(err.message.contains("Unknown command"));
        assert!(err.message.contains("serv"));
    }

    #[test]
    fn test_unknown_command_no_similar() {
        let err = builders::unknown_command("xyz", &["abc", "def", "ghi"]);
        assert!(err.message.contains("Unknown command"));
        assert!(err.message.contains("xyz"));
    }
}
