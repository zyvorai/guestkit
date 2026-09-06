// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI entry (`guestkit` / `guestctl`).

use crate::cli::commands::*;
use crate::cli::commands_list;
use crate::cli::invocation;
use crate::cli::plan::PlanCommand;
use crate::cli::welcome;
use crate::converters::DiskConverter;
use crate::VERSION;
use anyhow::Context;
use clap::{CommandFactory, FromArgMatches, Parser, Subcommand};
use clap_complete::{generate, shells};
use colored::Colorize;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// guestkit - Guest VM toolkit for disk inspection and manipulation
#[derive(Parser)]
#[command(name = "guestkit")]
#[command(version = VERSION)]
#[command(about = "Guest VM toolkit for disk inspection and manipulation", long_about = None)]
#[command(subcommand_required = false, arg_required_else_help = false)]
struct Cli {
    /// Verbose output
    #[arg(long, global = true)]
    verbose: bool,

    /// Debug output (show internal operations)
    #[arg(long, global = true)]
    debug: bool,

    /// Quiet mode (suppress non-error output)
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    quiet: bool,

    /// Disable colored output
    #[arg(long, global = true)]
    no_color: bool,

    /// Read-only mode (prevent any write operations to disk images)
    #[arg(short = 'R', long, global = true)]
    read_only: bool,

    /// Operation timeout in seconds (0 = no timeout)
    #[arg(short = 'T', long, global = true, default_value = "0")]
    timeout: u64,

    /// Custom cache directory path
    #[arg(long, global = true, value_name = "DIR")]
    cache_dir: Option<PathBuf>,

    /// Number of parallel workers for operations that support parallelism
    #[arg(short = 'j', long, global = true, value_name = "N")]
    jobs: Option<usize>,

    /// Show timestamps in output
    #[arg(long, global = true)]
    timestamps: bool,

    /// Output in machine-readable format (implies --no-color)
    #[arg(long, global = true)]
    machine_readable: bool,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum AgentChannelArg {
    Virtio,
    Vsock,
    Stdio,
}

#[derive(Subcommand)]
enum Commands {
    /// Inspect a disk image and display OS information
    Inspect {
        /// Disk image path
        image: PathBuf,

        /// Output format (text, json, yaml, csv)
        #[arg(short, long, value_name = "FORMAT")]
        output: Option<String>,

        /// Inspection profile (security, migration, performance, compliance, hardening)
        #[arg(short, long, value_name = "PROFILE")]
        profile: Option<String>,

        /// Export format (html, markdown, pdf)
        #[arg(short, long, value_name = "EXPORT_FORMAT")]
        export: Option<String>,

        /// Export output path
        #[arg(long, value_name = "PATH")]
        export_output: Option<PathBuf>,

        /// Disable caching of inspection results (enabled by default)
        #[arg(long)]
        no_cache: bool,

        /// Force refresh cache (ignore existing cached results)
        #[arg(long)]
        cache_refresh: bool,

        /// Show only summary information
        #[arg(short = 'S', long)]
        summary: bool,

        /// Include detailed package list in output
        #[arg(long)]
        include_packages: bool,

        /// Include full service list in output
        #[arg(long)]
        include_services: bool,

        /// Include network configuration details
        #[arg(long)]
        include_network: bool,

        /// Inspection depth (quick, standard, deep)
        #[arg(long, value_name = "DEPTH", default_value = "standard")]
        depth: String,

        /// Save inspection report to file
        #[arg(long, value_name = "FILE")]
        save_report: Option<PathBuf>,
    },

    /// Diff two disk images to show configuration changes
    Diff {
        /// First disk image
        image1: PathBuf,

        /// Second disk image
        image2: PathBuf,

        /// Output format (text, json, yaml)
        #[arg(short, long, value_name = "FORMAT")]
        output: Option<String>,
    },

    /// Compare multiple VMs against a baseline
    Compare {
        /// Baseline disk image
        baseline: PathBuf,

        /// Disk images to compare
        #[arg(required = true)]
        images: Vec<PathBuf>,
    },

    /// List files in a disk image
    #[command(alias = "ls")]
    List {
        /// Disk image path
        image: PathBuf,

        /// Path to list (default: /)
        #[arg(default_value = "/")]
        path: String,

        /// Recursive listing
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Show detailed information (permissions, size, owner)
        #[arg(short, long)]
        long: bool,

        /// Show hidden files (starting with .)
        #[arg(short = 'a', long)]
        all: bool,

        /// Human-readable file sizes
        #[arg(short = 'H', long)]
        human_readable: bool,

        /// Sort by modification time
        #[arg(short = 't', long)]
        sort_time: bool,

        /// Reverse sort order
        #[arg(long)]
        reverse: bool,

        /// Filter by file pattern (glob)
        #[arg(short = 'f', long)]
        filter: Option<String>,

        /// Show only directories
        #[arg(short = 'D', long)]
        directories_only: bool,

        /// Limit number of results
        #[arg(short = 'n', long)]
        limit: Option<usize>,
    },

    /// Extract a file from disk image
    #[command(alias = "get")]
    Extract {
        /// Disk image path
        image: PathBuf,

        /// Path in guest filesystem
        guest_path: String,

        /// Output file path on host
        #[arg(short, long)]
        output: PathBuf,

        /// Preserve file permissions and timestamps
        #[arg(short, long)]
        preserve: bool,

        /// Extract multiple files (guest_path can be a directory)
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Overwrite existing files without asking
        #[arg(short = 'f', long)]
        force: bool,

        /// Show progress during extraction
        #[arg(long)]
        progress: bool,

        /// Verify extracted file with checksum
        #[arg(long)]
        verify: bool,
    },

    /// Execute a command in the guest
    #[command(alias = "exec")]
    Execute {
        /// Disk image path
        image: PathBuf,

        /// Command and arguments to execute
        #[arg(trailing_var_arg = true, required = true)]
        command: Vec<String>,
    },

    /// Backup files from guest to tar archive
    Backup {
        /// Disk image path
        image: PathBuf,

        /// Path to backup in guest
        #[arg(default_value = "/")]
        path: String,

        /// Output tar.gz file
        #[arg(short, long)]
        output: PathBuf,
    },

    /// Convert disk image format
    Convert {
        /// Source disk image path
        source: PathBuf,

        /// Output disk image path
        #[arg(short, long)]
        output: PathBuf,

        /// Output format (qcow2, raw, vmdk, vhd, vdi)
        #[arg(short, long, default_value = "qcow2", value_parser = ["qcow2", "raw", "vmdk", "vhd", "vdi"])]
        format: String,

        /// Enable compression (qcow2 only)
        #[arg(short, long)]
        compress: bool,

        /// Flatten snapshot chains
        #[arg(short = 'F', long)]
        flatten: bool,

        /// Show progress bar during conversion
        #[arg(short = 'P', long)]
        progress: bool,

        /// Verify conversion with checksum
        #[arg(long)]
        verify: bool,

        /// Sparse output (don't write zeros)
        #[arg(short = 'S', long)]
        sparse: bool,

        /// Preallocate disk space (faster but uses more space)
        #[arg(long)]
        preallocate: bool,

        /// Compression level (1-9, higher = better compression)
        #[arg(long, value_name = "LEVEL", value_parser = clap::value_parser!(u8).range(1..=9))]
        compression_level: Option<u8>,

        /// Buffer size in MB for I/O operations
        #[arg(long, value_name = "SIZE", default_value = "4", value_parser = clap::value_parser!(u64).range(1..=1024))]
        buffer_size: u64,
    },

    /// Create a new disk image
    Create {
        /// Output disk image path
        path: PathBuf,

        /// Size in megabytes
        #[arg(short, long)]
        size: u64,

        /// Disk format (raw, qcow2, vmdk, vhd, vdi)
        #[arg(short, long, default_value = "raw", value_parser = ["raw", "qcow2", "vmdk", "vhd", "vdi"])]
        format: String,
    },

    /// Check filesystem on a disk image
    #[command(alias = "fsck")]
    Check {
        /// Disk image path
        image: PathBuf,

        /// Specific device to check (optional)
        #[arg(long)]
        device: Option<String>,
    },

    /// Show disk usage statistics
    #[command(alias = "df")]
    Usage {
        /// Disk image path
        image: PathBuf,
    },

    /// Shrink an oversized-but-mostly-empty disk to its real footprint
    /// (single/last ext2/3/4 partition, MBR or GPT, no LVM/LUKS)
    Shrink {
        /// Disk image path
        image: PathBuf,

        /// Only analyze and report; don't modify anything
        #[arg(long)]
        dry_run: bool,

        /// Minimum virtual/actual size ratio to consider shrinking worthwhile
        #[arg(long, default_value = "3.0")]
        min_ratio: f64,

        /// Extra headroom over the filesystem's true minimum size, as a percentage
        #[arg(long, default_value = "20")]
        headroom_pct: u32,

        /// Emit machine-readable JSON instead of text
        #[arg(long)]
        json: bool,
    },

    /// Detect disk image format
    Detect {
        /// Disk image path
        image: PathBuf,
    },

    /// Get disk image information
    Info {
        /// Disk image path
        image: PathBuf,
    },

    /// Inspect multiple disk images in batch
    #[command(name = "inspect-batch")]
    InspectBatch {
        /// Disk image paths (can use glob patterns)
        #[arg(required = true)]
        images: Vec<PathBuf>,

        /// Number of parallel workers (default: 4)
        #[arg(short, long, default_value = "4", value_parser = clap::value_parser!(u64).range(1..=64))]
        parallel: u64,

        /// Output format (text, json, yaml)
        #[arg(short, long, value_name = "FORMAT")]
        output: Option<String>,

        /// Disable caching of inspection results (enabled by default)
        #[arg(long)]
        no_cache: bool,
    },

    /// Clear inspection cache
    #[command(name = "cache-clear")]
    CacheClear,

    /// Show cache statistics
    #[command(name = "cache-stats")]
    CacheStats,

    /// List filesystems and partitions
    #[command(alias = "fs")]
    Filesystems {
        /// Disk image path
        image: PathBuf,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,
    },

    /// List installed packages
    #[command(alias = "pkg")]
    Packages {
        /// Disk image path
        image: PathBuf,

        /// Filter packages by name
        #[arg(short, long)]
        filter: Option<String>,

        /// Limit number of results
        #[arg(short, long)]
        limit: Option<usize>,

        /// Output as JSON
        #[arg(long)]
        json: bool,
    },

    /// Read file content from disk image
    Cat {
        /// Disk image path
        image: PathBuf,

        /// Path to file in guest filesystem
        path: String,

        /// Show line numbers
        #[arg(short = 'n', long)]
        line_numbers: bool,

        /// Show non-printing characters
        #[arg(short = 'A', long)]
        show_all: bool,
    },

    /// Search for files by name or pattern
    #[command(alias = "find")]
    Search {
        /// Disk image path
        image: PathBuf,

        /// Search pattern (glob or regex)
        pattern: String,

        /// Starting directory for search
        #[arg(short, long, default_value = "/")]
        path: String,

        /// Use regex instead of glob
        #[arg(short = 'E', long)]
        regex: bool,

        /// Case-insensitive search
        #[arg(short = 'i', long)]
        ignore_case: bool,

        /// Search file content, not just names
        #[arg(short = 'c', long)]
        content: bool,

        /// File type filter (file, dir, link, socket, etc.)
        #[arg(short = 't', long)]
        file_type: Option<String>,

        /// Maximum search depth
        #[arg(short = 'D', long)]
        max_depth: Option<usize>,

        /// Limit number of results
        #[arg(short = 'l', long)]
        limit: Option<usize>,
    },

    /// Search file contents (like grep)
    Grep {
        /// Disk image path
        image: PathBuf,

        /// Search pattern
        pattern: String,

        /// File or directory to search in
        #[arg(default_value = "/")]
        path: String,

        /// Case-insensitive search
        #[arg(short = 'i', long)]
        ignore_case: bool,

        /// Show line numbers
        #[arg(short = 'n', long)]
        line_numbers: bool,

        /// Recursive search
        #[arg(short = 'r', long)]
        recursive: bool,

        /// Show only matching filenames
        #[arg(short = 'l', long)]
        files_only: bool,

        /// Invert match (show non-matching lines)
        #[arg(short = 'V', long)]
        invert: bool,

        /// Context lines before match
        #[arg(short = 'B', long, value_name = "NUM")]
        before_context: Option<usize>,

        /// Context lines after match
        #[arg(short = 'A', long, value_name = "NUM")]
        after_context: Option<usize>,

        /// Maximum results
        #[arg(short = 'm', long)]
        max_count: Option<usize>,
    },

    /// Calculate file checksums
    Hash {
        /// Disk image path
        image: PathBuf,

        /// Path to file in guest filesystem
        path: String,

        /// Hash algorithm (md5, sha1, sha256, sha512)
        #[arg(short = 'a', long, default_value = "sha256")]
        algorithm: String,

        /// Verify against expected hash
        #[arg(short = 'c', long)]
        check: Option<String>,

        /// Recursive hashing for directories
        #[arg(short = 'r', long)]
        recursive: bool,
    },

    /// Security vulnerability scan
    Scan {
        /// Disk image path
        image: PathBuf,

        /// Scan type (packages, config, permissions, all)
        #[arg(short = 't', long, default_value = "all", value_parser = ["packages", "config", "permissions", "all"])]
        scan_type: String,

        /// Severity threshold (low, medium, high, critical)
        #[arg(short = 's', long)]
        severity: Option<String>,

        /// Output format (text, json, sarif)
        #[arg(short, long, value_name = "FORMAT")]
        output: Option<String>,

        /// Generate detailed report
        #[arg(short = 'r', long)]
        report: bool,

        /// Check CVE database for vulnerabilities
        #[arg(long)]
        check_cve: bool,
    },

    /// Benchmark disk I/O performance
    Benchmark {
        /// Disk image path
        image: PathBuf,

        /// Test type (read, write, random, sequential)
        #[arg(short = 't', long, default_value = "all")]
        test_type: String,

        /// Block size for I/O operations (in KB)
        #[arg(short = 'b', long, default_value = "4", value_parser = clap::value_parser!(u64).range(1..=65536))]
        block_size: u64,

        /// Duration of test in seconds
        #[arg(long, default_value = "10")]
        duration: u64,

        /// Number of iterations
        #[arg(short = 'n', long, default_value = "3")]
        iterations: usize,
    },

    /// Manage disk snapshots
    Snapshot {
        /// Disk image path
        image: PathBuf,

        /// Snapshot operation (create, list, delete, revert)
        #[arg(value_enum)]
        operation: SnapshotOperation,

        /// Snapshot name
        #[arg(short = 'n', long)]
        name: Option<String>,

        /// Snapshot description
        #[arg(long)]
        description: Option<String>,
    },

    /// Compare specific files between disk images
    DiffFiles {
        /// First disk image
        image1: PathBuf,

        /// Second disk image
        image2: PathBuf,

        /// Path to compare
        #[arg(default_value = "/")]
        path: String,

        /// Unified diff format
        #[arg(short = 'u', long)]
        unified: bool,

        /// Context lines
        #[arg(short = 'C', long, default_value = "3")]
        context: usize,

        /// Ignore whitespace differences
        #[arg(short = 'w', long)]
        ignore_whitespace: bool,
    },

    /// Find large files in disk image
    FindLarge {
        /// Disk image path
        image: PathBuf,

        /// Starting path
        #[arg(default_value = "/")]
        path: String,

        /// Minimum file size in bytes
        #[arg(short = 's', long, default_value = "10485760")]
        min_size: u64,

        /// Maximum number of results
        #[arg(short = 'n', long, default_value = "20", value_parser = clap::value_parser!(u64).range(1..=10000))]
        max_results: u64,

        /// Human-readable sizes
        #[arg(short = 'H', long)]
        human_readable: bool,
    },

    /// Copy files between disk images
    Copy {
        /// Source disk image
        source_image: PathBuf,

        /// Source file path
        source_path: String,

        /// Destination disk image
        dest_image: PathBuf,

        /// Destination file path
        dest_path: String,

        /// Preserve permissions and timestamps
        #[arg(short = 'p', long)]
        preserve: bool,

        /// Force overwrite if destination exists
        #[arg(short = 'f', long)]
        force: bool,
    },

    /// Find duplicate files
    FindDuplicates {
        /// Disk image path
        image: PathBuf,

        /// Starting path
        #[arg(default_value = "/")]
        path: String,

        /// Minimum file size to consider
        #[arg(short = 's', long, default_value = "1048576")]
        min_size: u64,

        /// Hash algorithm
        #[arg(short = 'a', long, default_value = "sha256")]
        algorithm: String,
    },

    /// Analyze disk usage by directory
    DiskUsage {
        /// Disk image path
        image: PathBuf,

        /// Starting path
        #[arg(default_value = "/")]
        path: String,

        /// Maximum directory depth
        #[arg(short = 'D', long, default_value = "5")]
        max_depth: usize,

        /// Minimum size to display
        #[arg(short = 's', long, default_value = "1048576")]
        min_size: u64,

        /// Human-readable sizes
        #[arg(short = 'H', long)]
        human_readable: bool,
    },

    /// Build forensic timeline from multiple sources
    Timeline {
        /// Disk image path
        image: PathBuf,

        /// Start time filter (ISO 8601)
        #[arg(long)]
        start_time: Option<String>,

        /// End time filter (ISO 8601)
        #[arg(long)]
        end_time: Option<String>,

        /// Data sources (files, packages, logs)
        #[arg(short = 's', long, value_delimiter = ',')]
        sources: Vec<String>,

        /// Output format (text, json, csv)
        #[arg(short = 'f', long, default_value = "text")]
        format: String,
    },

    /// Create unique fingerprint for disk image
    Fingerprint {
        /// Disk image path
        image: PathBuf,

        /// Hash algorithm
        #[arg(short = 'a', long, default_value = "sha256")]
        algorithm: String,

        /// Include file content hashes
        #[arg(short = 'c', long)]
        include_content: bool,

        /// Output file path
        #[arg(short = 'o', long)]
        output: Option<PathBuf>,
    },

    /// Detect configuration drift from baseline
    Drift {
        /// Baseline disk image
        baseline: PathBuf,

        /// Current disk image to compare
        current: PathBuf,

        /// Paths to ignore (comma-separated)
        #[arg(long, value_delimiter = ',')]
        ignore_paths: Vec<String>,

        /// Drift threshold percentage (0-100)
        #[arg(short = 't', long, default_value = "20", value_parser = clap::value_parser!(u8).range(0..=100))]
        threshold: u8,

        /// Generate detailed report
        #[arg(short = 'r', long)]
        report: bool,
    },

    /// AI-powered deep analysis with insights
    Analyze {
        /// Disk image path
        image: PathBuf,

        /// Analysis focus areas (security, performance, compliance, maintainability)
        #[arg(short = 'f', long, value_delimiter = ',')]
        focus: Vec<String>,

        /// Analysis depth (quick, standard, deep)
        #[arg(long, default_value = "standard")]
        depth: String,

        /// Show actionable suggestions
        #[arg(short = 's', long)]
        suggestions: bool,
    },

    /// Scan for exposed secrets and credentials
    Secrets {
        /// Disk image path
        image: PathBuf,

        /// Paths to scan (comma-separated)
        #[arg(long, value_delimiter = ',')]
        scan_paths: Vec<String>,

        /// Custom regex patterns to search for
        #[arg(short = 'p', long, value_delimiter = ',')]
        patterns: Vec<String>,

        /// Paths to exclude (comma-separated)
        #[arg(short = 'e', long, value_delimiter = ',')]
        exclude: Vec<String>,

        /// Show actual secret content (WARNING: sensitive)
        #[arg(long)]
        show_content: bool,

        /// Export report to file
        #[arg(short = 'o', long)]
        export: Option<PathBuf>,
    },

    /// Automated rescue and recovery operations
    Rescue {
        /// Disk image path
        image: PathBuf,

        /// Rescue operation (reset-password, fix-fstab, check-grub, fix-grub, enable-ssh,
        /// inject-ssh-key, set-hostname, enable-rdp, enable-winrm, set-timezone,
        /// install-packages)
        #[arg(short = 'o', long)]
        operation: String,

        /// Username (for reset-password / inject-ssh-key)
        #[arg(short = 'u', long)]
        user: Option<String>,

        /// New password (for reset-password; Linux shadow; Windows AES SAM hash or blank+RunOnce)
        #[arg(short = 'p', long)]
        password: Option<String>,

        /// Force operation even if risky
        #[arg(short = 'f', long)]
        force: bool,

        /// Backup files before modification
        #[arg(short = 'b', long)]
        backup: bool,

        /// SSH public key string (for inject-ssh-key)
        #[arg(long)]
        key: Option<String>,

        /// Path to SSH public key file (for inject-ssh-key)
        #[arg(long)]
        key_file: Option<PathBuf>,

        /// Hostname (for set-hostname)
        #[arg(long)]
        hostname: Option<String>,

        /// Windows TimeZoneKeyName (for set-timezone)
        #[arg(long)]
        timezone: Option<String>,

        /// Write a reviewable FixPlan YAML instead of applying (enable-ssh, inject-ssh-key, set-hostname, reset-password, fix-fstab, enable-rdp, enable-winrm, set-timezone)
        #[arg(long, value_name = "PLAN.yaml")]
        export_plan: Option<PathBuf>,

        /// Packages to install (for install-packages), comma-delimited
        #[arg(long, value_delimiter = ',')]
        packages: Vec<String>,

        /// Allow network access during install-packages (swaps in the
        /// host's /etc/resolv.conf for DNS, restored afterward)
        #[arg(long)]
        network: bool,
    },

    /// Optimize disk image (cleanup, compact)
    Optimize {
        /// Disk image path
        image: PathBuf,

        /// Operations to perform (temp, logs, cache, packages)
        #[arg(short = 'o', long, value_delimiter = ',')]
        operations: Vec<String>,

        /// Aggressive cleanup (may remove more files)
        #[arg(short = 'a', long)]
        aggressive: bool,

        /// Dry run (show what would be removed)
        #[arg(long)]
        dry_run: bool,
    },

    /// Analyze network configuration
    Network {
        /// Disk image path
        image: PathBuf,

        /// Show routing information
        #[arg(long)]
        show_routes: bool,

        /// Show network interfaces
        #[arg(long)]
        show_interfaces: bool,

        /// Show DNS configuration
        #[arg(long)]
        show_dns: bool,

        /// Export as JSON
        #[arg(long)]
        export_json: bool,
    },

    /// Compliance checking against security standards
    Compliance {
        /// Disk image path
        image: PathBuf,

        /// Security standard (cis, pci-dss, hipaa)
        #[arg(short = 's', long)]
        standard: String,

        /// Compliance profile (e.g., level1, level2)
        #[arg(short = 'p', long)]
        profile: Option<String>,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,

        /// Attempt to fix issues
        #[arg(short = 'f', long)]
        fix: bool,
    },

    /// Malware and rootkit detection
    Malware {
        /// Disk image path
        image: PathBuf,

        /// Deep scan (more thorough but slower)
        #[arg(long)]
        deep_scan: bool,

        /// Check for rootkit indicators
        #[arg(long)]
        check_rootkits: bool,

        /// YARA rules file
        #[arg(long)]
        yara_rules: Option<PathBuf>,

        /// Quarantine suspicious files
        #[arg(long)]
        quarantine: bool,
    },

    /// System health and diagnostics
    Health {
        /// Disk image path
        image: PathBuf,

        /// Specific checks to run (disk, services, security, packages, logs)
        #[arg(short = 'c', long, value_delimiter = ',')]
        checks: Vec<String>,

        /// Show detailed information
        #[arg(long)]
        detailed: bool,

        /// Export as JSON
        #[arg(long)]
        export_json: Option<PathBuf>,
    },

    /// Clone disk image with customizations
    Clone {
        /// Source disk image (required unless --lvm)
        #[arg(required_unless_present = "lvm")]
        source: Option<PathBuf>,

        /// Destination disk image (required unless --lvm)
        #[arg(required_unless_present = "lvm")]
        dest: Option<PathBuf>,

        /// Run sysprep (generalize image)
        #[arg(short = 's', long)]
        sysprep: bool,

        /// Set new hostname
        #[arg(long)]
        hostname: Option<String>,

        /// Remove SSH host keys
        #[arg(long)]
        remove_keys: bool,

        /// Preserve user accounts and history
        #[arg(long)]
        preserve_users: bool,

        /// Use LVM cloning mode (clone host logical volumes directly)
        #[arg(long)]
        lvm: bool,

        /// Source volume group name (requires --lvm)
        #[arg(long, value_name = "VG", requires = "lvm")]
        source_vg: Option<String>,

        /// Source logical volume name (requires --lvm)
        #[arg(long, value_name = "LV", requires = "lvm")]
        source_lv: Option<String>,

        /// Target volume group (defaults to source VG; requires --lvm)
        #[arg(long, value_name = "VG", requires = "lvm")]
        target_vg: Option<String>,

        /// Regenerate filesystem UUIDs on the clone
        #[arg(long, default_value_t = true, requires = "lvm")]
        regenerate_uuids: bool,

        /// Update /etc/fstab inside the clone with new UUIDs
        #[arg(long, requires = "lvm")]
        update_fstab: bool,

        /// Update GRUB bootloader config with new UUIDs
        #[arg(long, requires = "lvm")]
        update_bootloader: bool,

        /// Dry-run: validate parameters without performing the clone
        #[arg(long, requires = "lvm")]
        dry_run: bool,

        /// LVM snapshot size (e.g. "10G"; requires --lvm)
        #[arg(long, value_name = "SIZE", requires = "lvm")]
        snapshot_size: Option<String>,

        /// Name for the cloned LV (defaults to {source_lv}-clone; requires --lvm)
        #[arg(long, value_name = "NAME", requires = "lvm")]
        clone_name: Option<String>,

        /// Update /etc/crypttab inside the clone with new UUIDs
        #[arg(long, requires = "lvm")]
        update_crypttab: bool,

        /// Regenerate initramfs after UUID changes (requires chroot tools)
        #[arg(long, requires = "lvm")]
        regenerate_initramfs: bool,

        /// Namespace isolation level: none, mount, full (mount+pid+uts+ipc+net)
        #[arg(long, requires = "lvm", default_value = "none", value_parser = ["none", "mount", "full"])]
        isolation_level: String,

        /// Run post-clone security verification
        #[arg(long, requires = "lvm")]
        verify_security: bool,

        /// Regenerate GRUB config via grub-mkconfig inside the clone
        #[arg(long, requires = "lvm")]
        regenerate_grub: bool,

        /// Verify boot configuration (kernel, initramfs, GRUB) after changes
        #[arg(long, requires = "lvm")]
        verify_boot: bool,
    },

    /// Security patch analysis and CVE detection
    Patch {
        /// Disk image path
        image: PathBuf,

        /// Check for CVEs in installed packages
        #[arg(long)]
        check_cves: bool,

        /// Filter by severity (CRITICAL, HIGH, MEDIUM, LOW, ALL)
        #[arg(short = 's', long)]
        severity: Option<String>,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,

        /// Simulate package updates
        #[arg(long)]
        simulate_update: bool,

        /// Use demo CVE/outdated-package data (training only — not a live vuln feed)
        #[arg(long)]
        simulate: bool,
    },

    /// Generate Software Bill of Materials (SBOM)
    Inventory {
        /// Disk image path
        image: PathBuf,

        /// Output format (spdx, cyclonedx, json, csv)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "spdx")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Include license information
        #[arg(long)]
        include_licenses: bool,

        /// Include file manifests
        #[arg(long)]
        include_files: bool,

        /// Include CVE mappings
        #[arg(long)]
        include_cves: bool,

        /// Filter CVEs by severity (critical, high, medium, low)
        #[arg(long, value_name = "SEVERITY")]
        severity: Option<String>,

        /// Show summary before export
        #[arg(short = 'S', long)]
        summary: bool,
    },

    /// Validate disk image against policy
    Validate {
        /// Disk image path
        image: PathBuf,

        /// Policy file path (YAML)
        #[arg(short, long, value_name = "FILE")]
        policy: Option<PathBuf>,

        /// Use industry benchmark (cis-ubuntu, cis-rhel, nist, pci, hipaa)
        #[arg(short, long, value_name = "BENCHMARK")]
        benchmark: Option<String>,

        /// Generate example policy file
        #[arg(long)]
        example_policy: bool,

        /// Output format (text, json)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Fail on any validation failure
        #[arg(long)]
        strict: bool,
    },

    /// License compliance checking
    License {
        /// Disk image path
        image: PathBuf,

        /// Output format (text, json, csv)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Prohibited licenses (comma-separated)
        #[arg(long, value_delimiter = ',')]
        prohibit: Vec<String>,

        /// Show detailed package list
        #[arg(long)]
        details: bool,

        /// Generate attribution notices
        #[arg(long)]
        attribution: bool,

        /// Fail on prohibited licenses
        #[arg(long)]
        strict: bool,
    },

    /// Generate infrastructure-as-code blueprints
    Blueprint {
        /// Disk image path
        image: PathBuf,

        /// Blueprint format (terraform, ansible, kubernetes, compose)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "terraform")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Cloud provider for Terraform (aws, azure, gcp)
        #[arg(long, value_name = "PROVIDER")]
        provider: Option<String>,
    },

    /// Plan OS migrations and platform changes
    Migrate {
        /// Disk image path
        image: PathBuf,

        /// Migration target (os, cloud, container)
        #[arg(short = 't', long, value_name = "TYPE", default_value = "os")]
        target_type: String,

        /// Target OS or platform
        #[arg(long, value_name = "TARGET")]
        target: String,

        /// Target version
        #[arg(long, value_name = "VERSION")]
        version: Option<String>,

        /// Output format (text, json, html)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Show detailed analysis
        #[arg(long)]
        detailed: bool,
    },

    /// Cloud cost optimization analysis
    Cost {
        /// Disk image path
        image: PathBuf,

        /// Cloud provider (aws, azure, gcp)
        #[arg(short = 'p', long, value_name = "PROVIDER", default_value = "aws")]
        provider: String,

        /// Cloud region
        #[arg(short = 'r', long, value_name = "REGION", default_value = "us-east-1")]
        region: String,

        /// Output format (text, json, csv)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Show detailed analysis
        #[arg(long)]
        detailed: bool,
    },

    /// Comprehensive security audit with detailed reporting
    Audit {
        /// Disk image path
        image: PathBuf,

        /// Audit categories (permissions, users, network, services)
        #[arg(short = 'c', long, value_delimiter = ',')]
        categories: Vec<String>,

        /// Output format (text, json)
        #[arg(short = 'f', long, default_value = "text")]
        output_format: String,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,

        /// Attempt to fix issues automatically
        #[arg(long)]
        fix_issues: bool,
    },

    /// Automated system repair operations
    Repair {
        /// Disk image path
        image: PathBuf,

        /// Repair type (permissions, packages, network, bootloader, filesystem, boot)
        #[arg(short = 't', long)]
        repair_type: Option<String>,

        /// Fix category (boot, network, etc.)
        #[arg(long)]
        fix: Option<String>,

        /// Dry-run repair without applying changes
        #[arg(long)]
        dry_run: bool,

        /// Force repair even if risky
        #[arg(short = 'f', long)]
        force: bool,

        /// Backup before repair
        #[arg(short = 'b', long)]
        backup: bool,

        /// Inject GuestKit agent binary and systemd unit into disk image
        #[arg(long)]
        inject_agent: bool,

        /// Path to guestkit binary for agent injection
        #[arg(long, value_name = "PATH")]
        agent_binary: Option<PathBuf>,

        /// Path to guestkit-agent.service unit file
        #[arg(long, value_name = "PATH")]
        agent_unit: Option<PathBuf>,
    },

    /// System hardening configuration
    Harden {
        /// Disk image path
        image: PathBuf,

        /// Hardening profile (basic, moderate, strict)
        #[arg(short = 'p', long, default_value = "basic")]
        profile: String,

        /// Apply hardening (default is dry-run)
        #[arg(short = 'a', long)]
        apply: bool,

        /// Preview changes without applying
        #[arg(long)]
        preview: bool,
    },

    /// AI-powered anomaly detection
    Anomaly {
        /// Disk image path
        image: PathBuf,

        /// Baseline image for comparison
        #[arg(short = 'b', long)]
        baseline: Option<PathBuf>,

        /// Detection sensitivity (low, medium, high)
        #[arg(short = 's', long, default_value = "medium")]
        sensitivity: String,

        /// Categories to check (files, config, processes, network)
        #[arg(short = 'c', long, value_delimiter = ',')]
        categories: Vec<String>,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Smart recommendations engine
    Recommend {
        /// Disk image path
        image: PathBuf,

        /// Focus areas (security, performance, reliability, cost)
        #[arg(short = 'f', long, value_delimiter = ',')]
        focus: Vec<String>,

        /// Priority filter (critical, high, medium, low)
        #[arg(short = 'p', long, default_value = "medium")]
        priority: String,

        /// Auto-apply safe recommendations
        #[arg(short = 'a', long)]
        apply: bool,
    },

    /// Dependency graph and impact analysis
    Dependencies {
        /// Disk image path
        image: PathBuf,

        /// Output format (text, dot, json, csv, html)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file (stdout if not specified)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Show detailed package information
        #[arg(long)]
        detailed: bool,

        /// Package to analyze (show dependency tree for specific package)
        #[arg(long, value_name = "PACKAGE")]
        package: Option<String>,

        /// Show reverse dependencies (what depends on package)
        #[arg(long)]
        reverse: bool,

        /// Maximum tree depth
        #[arg(long, value_name = "DEPTH", default_value = "5")]
        max_depth: usize,

        /// Show all packages in graph (default: top 50)
        #[arg(long)]
        show_all: bool,
    },

    /// Predictive analysis and capacity planning
    Predict {
        /// Disk image path
        image: PathBuf,

        /// Metric to predict (disk-growth, log-growth, package-updates)
        #[arg(short = 'm', long, default_value = "disk-growth")]
        metric: String,

        /// Forecast timeframe in days
        #[arg(short = 't', long, default_value = "30")]
        timeframe: u32,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Threat intelligence correlation and IOC detection
    Intelligence {
        /// Disk image path
        image: PathBuf,

        /// Custom IOC file (STIX, OpenIOC, or CSV format)
        #[arg(short = 'i', long)]
        ioc_file: Option<PathBuf>,

        /// Threat level filter (critical, high, medium, low)
        #[arg(short = 'l', long, default_value = "medium")]
        threat_level: String,

        /// Enable correlation analysis
        #[arg(short = 'c', long)]
        correlate: bool,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,

        /// Use demo IOC database (training only — not live threat intel)
        #[arg(long)]
        simulate: bool,
    },

    /// Change simulation and impact modeling
    Simulate {
        /// Disk image path
        image: PathBuf,

        /// Change type (remove-package, modify-config, disable-service, kernel-update)
        #[arg(short = 't', long)]
        change_type: String,

        /// Target (package name, config file, service name, etc.)
        #[arg(long)]
        target: String,

        /// Dry run - simulate without making changes
        #[arg(long)]
        dry_run: bool,

        /// Include comprehensive risk assessment
        #[arg(short = 'r', long)]
        risk_assessment: bool,
    },

    /// Comprehensive multi-dimensional risk scoring
    Score {
        /// Disk image path
        image: PathBuf,

        /// Risk dimensions to check (security, compliance, reliability, performance, maintainability)
        #[arg(long, value_delimiter = ',')]
        dimensions: Vec<String>,

        /// Custom weights (format: security=40,compliance=30,...)
        #[arg(short = 'w', long)]
        weights: Option<String>,

        /// Compare against benchmark image
        #[arg(short = 'b', long)]
        benchmark: Option<PathBuf>,

        /// Export report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Golden image template validation
    Template {
        /// Disk image path
        image: PathBuf,

        /// Template type (web-server, database, docker-host, cis-level1)
        #[arg(short = 't', long)]
        template: String,

        /// Strict mode - fail on any violation
        #[arg(short = 's', long)]
        strict: bool,

        /// Automatically fix violations where possible
        #[arg(short = 'f', long)]
        fix: bool,

        /// Export template definition to file
        #[arg(short = 'e', long)]
        export_template: Option<PathBuf>,
    },

    /// Proactive threat hunting with hypothesis-driven investigation
    Hunt {
        /// Disk image path
        image: PathBuf,

        /// Threat hunting hypothesis
        #[arg(short = 'H', long)]
        hypothesis: String,

        /// Hunting framework (mitre-attack, custom)
        #[arg(short = 'f', long, default_value = "mitre-attack")]
        framework: String,

        /// Specific techniques to hunt (comma-separated tactics)
        #[arg(short = 't', long, value_delimiter = ',')]
        techniques: Vec<String>,

        /// Hunt depth (surface, shallow, deep, comprehensive)
        #[arg(short = 'D', long, default_value = "deep")]
        depth: String,

        /// Export hunt report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Forensic incident reconstruction and attack path visualization
    Reconstruct {
        /// Disk image path
        image: PathBuf,

        /// Incident type (compromise, data-exfiltration, ransomware, generic)
        #[arg(short = 't', long)]
        incident_type: String,

        /// Start time for analysis window
        #[arg(short = 's', long)]
        start_time: Option<String>,

        /// End time for analysis window
        #[arg(short = 'E', long)]
        end_time: Option<String>,

        /// Generate attack path visualization
        #[arg(short = 'V', long)]
        visualize: bool,

        /// Export reconstruction report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Automated progressive system evolution and self-improvement
    Evolve {
        /// Disk image path
        image: PathBuf,

        /// Target state (hardened, optimized, compliant, production-ready)
        #[arg(short = 't', long)]
        target_state: String,

        /// Evolution strategy (aggressive, balanced, conservative)
        #[arg(short = 's', long, default_value = "balanced")]
        strategy: String,

        /// Number of evolution stages
        #[arg(short = 'S', long, default_value = "3")]
        stages: u32,

        /// Enable safety checks and rollback plans
        #[arg(short = 'c', long)]
        safety_checks: bool,

        /// Export evolution plan to file
        #[arg(short = 'e', long)]
        export_plan: Option<PathBuf>,
    },

    /// Zero-trust continuous verification and supply chain integrity
    Verify {
        /// Disk image path
        image: PathBuf,

        /// Verification level (basic, standard, strict, paranoid)
        #[arg(short = 'l', long, default_value = "standard")]
        verification_level: String,

        /// Check supply chain integrity
        #[arg(short = 's', long)]
        check_supply_chain: bool,

        /// Verify identity and accounts
        #[arg(short = 'i', long)]
        check_identity: bool,

        /// Verify file integrity
        #[arg(short = 'I', long)]
        check_integrity: bool,

        /// Export verification report to file
        #[arg(short = 'e', long)]
        export: Option<PathBuf>,
    },

    /// Show version information
    Version,

    /// Start interactive mode for exploring disk image
    #[command(alias = "repl")]
    Interactive {
        /// Disk image path
        image: PathBuf,
    },

    /// Launch interactive file explorer (TUI mode)
    #[command(alias = "ex")]
    Explore {
        /// Disk image path
        image: PathBuf,

        /// Starting path in VM filesystem (default: /)
        #[arg(default_value = "/")]
        path: String,
    },

    /// Execute commands from a script file (batch mode)
    #[command(alias = "batch")]
    Script {
        /// Disk image path
        image: PathBuf,

        /// Script file with commands (one per line)
        script: PathBuf,

        /// Stop on first error
        #[arg(short, long)]
        fail_fast: bool,
    },

    /// Analyze systemd journal logs
    #[command(name = "systemd-journal")]
    SystemdJournal {
        /// Disk image path
        image: PathBuf,

        /// Filter by priority (0=emerg, 3=err, 4=warning, 6=info)
        #[arg(short, long)]
        priority: Option<u8>,

        /// Filter by unit name
        #[arg(short, long)]
        unit: Option<String>,

        /// Show only errors (priority 0-3)
        #[arg(short, long)]
        errors: bool,

        /// Show only warnings (priority 4)
        #[arg(short, long)]
        warnings: bool,

        /// Show statistics
        #[arg(short, long)]
        stats: bool,

        /// Limit number of entries
        #[arg(short, long)]
        limit: Option<usize>,
    },

    /// Analyze systemd services and dependencies
    #[command(name = "systemd-services")]
    SystemdServices {
        /// Disk image path
        image: PathBuf,

        /// Show dependency tree for specific service
        #[arg(short, long)]
        service: Option<String>,

        /// Show only failed services
        #[arg(short, long)]
        failed: bool,

        /// Generate Mermaid diagram for dependencies
        #[arg(long)]
        diagram: bool,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT")]
        output: Option<String>,
    },

    /// Analyze systemd boot performance
    #[command(name = "systemd-boot")]
    SystemdBoot {
        /// Disk image path
        image: PathBuf,

        /// Show boot timeline diagram
        #[arg(short, long)]
        timeline: bool,

        /// Show optimization recommendations
        #[arg(short, long)]
        recommendations: bool,

        /// Show summary statistics
        #[arg(short, long)]
        summary: bool,

        /// Number of slowest services to show
        #[arg(short = 'n', long, default_value = "10")]
        top: usize,
    },

    /// Interactive TUI for VM inspection with orange color theme
    #[command(alias = "ui")]
    Tui {
        /// Disk image path
        image: PathBuf,
        /// Optional second image for comparison summary
        #[arg(long)]
        compare: Option<PathBuf>,
        /// Directory of disk images for fleet mode (sidebar, N/P to switch)
        #[arg(long)]
        fleet: Option<PathBuf>,
    },

    /// Interactive shell for VM inspection (REPL mode)
    #[command(alias = "sh")]
    Shell {
        /// Disk image path
        image: PathBuf,
    },

    /// AI-powered diagnostics and assistance (requires --features ai and OPENAI_API_KEY)
    Ai {
        /// Disk image path
        image: PathBuf,

        /// Question or problem description
        #[arg(required = true)]
        query: String,
    },

    /// Generate shell completion scripts
    Completion {
        /// Shell type
        #[arg(value_enum)]
        shell: CompletionShell,

        /// Emit completions for both guestkit and guestctl
        #[arg(long)]
        all: bool,
    },

    /// List subcommands grouped by category
    #[command(name = "commands")]
    CommandCatalog,

    /// Manage fix plans (preview, validate, export, apply)
    Plan(PlanCommand),

    /// Bootability prediction — will this VM survive first boot?
    Doctor {
        /// Disk image path
        image: PathBuf,

        /// Target hypervisor (kvm, proxmox, hyperv, cloud)
        #[arg(long, default_value = "kvm")]
        target: String,

        /// Deterministic root-cause analysis
        #[arg(long)]
        explain: bool,

        /// LLM-powered Guest Intelligence summary (requires --features ai)
        #[arg(long)]
        ai: bool,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Exit with code 1 if boot score is below this threshold (0–100, for CI gates)
        #[arg(long, value_name = "SCORE")]
        fail_below: Option<u8>,
    },

    /// MCP server over stdio for a single VM — read-only evidence tools,
    /// same 6 as the AI copilot, for external MCP hosts (requires --features mcp)
    #[cfg(feature = "mcp")]
    #[command(name = "mcp-serve")]
    McpServe {
        /// Disk image path
        image: PathBuf,

        /// Target hypervisor (kvm, proxmox, hyperv, cloud)
        #[arg(long, default_value = "kvm")]
        target: String,
    },

    /// Policy-as-code compliance check
    Policy {
        #[command(subcommand)]
        action: PolicyAction,
    },

    /// Print or export a cloud cutover profile (aws/azure/gcp/openstack)
    #[command(name = "cloud-profile")]
    CloudProfile {
        /// aws, azure, gcp, openstack
        target: String,
        /// Write the Policy YAML
        #[arg(short, long)]
        export: Option<PathBuf>,
        /// Also run `policy check` against this disk
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long)]
        strict: bool,
    },

    /// Fleet-wide VM analysis
    Fleet {
        #[command(subcommand)]
        action: FleetAction,
    },

    /// Hypervisor-aware migration plan
    #[command(name = "migrate-plan")]
    MigratePlan {
        /// Disk image path
        image: PathBuf,

        /// Target platform (proxmox, kvm, aws, azure, gcp)
        #[arg(long, value_name = "TARGET")]
        target: String,

        /// Include root-cause explanation
        #[arg(long)]
        explain: bool,

        /// LLM-powered Guest Intelligence summary (requires --features ai)
        #[arg(long)]
        ai: bool,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Export executable fix plan (YAML or JSON from file extension)
        #[arg(long, value_name = "FILE")]
        export: Option<PathBuf>,

        /// Include GuestKit agent install ops in exported fix plan
        #[arg(long)]
        inject_agent: bool,

        /// Path to guestkit binary for agent injection (export / repair)
        #[arg(long, value_name = "PATH")]
        agent_binary: Option<PathBuf>,

        /// Path to guestkit-agent.service unit file
        #[arg(long, value_name = "PATH")]
        agent_unit: Option<PathBuf>,
    },

    /// Categorized migration-readiness assessment (sub-scores + blockers)
    #[command(name = "migrate-assess")]
    MigrateAssess {
        /// Disk image path
        image: PathBuf,

        /// Target platform (proxmox, kvm, kubevirt, aws, azure, gcp)
        #[arg(long, value_name = "TARGET")]
        target: String,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Exit non-zero when the score is below this threshold (CI gate)
        #[arg(long, value_name = "SCORE")]
        fail_below: Option<f64>,
    },

    /// Generate (and optionally apply) a migration repair plan offline
    #[command(name = "migrate-repair")]
    MigrateRepair {
        /// Disk image path
        image: PathBuf,

        /// Target platform (proxmox, kvm, kubevirt, aws, azure, gcp)
        #[arg(long, value_name = "TARGET")]
        target: String,

        /// Apply the plan to the image (full-image backup is taken first)
        #[arg(long)]
        apply: bool,

        /// Include destructive repairs (tool uninstall, ghost-NIC removal)
        #[arg(long)]
        include_destructive: bool,

        /// Host path to a virtio-win tree (or single driver dir) for offline DriverInject.
        /// Also read from `$GUESTKIT_VIRTIO_WIN`.
        #[arg(long, value_name = "DIR")]
        virtio_win: Option<PathBuf>,

        /// Export the plan as JSON
        #[arg(long, value_name = "FILE")]
        export: Option<PathBuf>,
    },

    /// Cutover Passport — CI-gateable assurance artifact (GuestKit certifies; HyperSDK/hyper2kvm convert)
    Passport {
        #[command(subcommand)]
        action: PassportAction,
    },

    /// qemu-img operations owned by GuestKit (info/check/snapshot/resize/rebase/commit)
    Img {
        #[command(subcommand)]
        action: ImgAction,
    },

    /// List disk sources from libvirt XML or KubeVirt VM/VMI YAML
    #[command(name = "domain-disks")]
    DomainDisks {
        /// domain.xml / vm.yaml / vmi.yaml
        file: PathBuf,
        /// Only print file= sources, one per line (scriptable)
        #[arg(long)]
        files_only: bool,
        /// JSON output
        #[arg(long)]
        json: bool,
    },

    /// Discover a virtio-win tree and plan offline driver injection
    #[command(name = "virtio-win")]
    VirtioWin {
        #[command(subcommand)]
        action: VirtioWinAction,
    },

    /// First-boot attestation: offline doctor + optional live QGA ping
    Firstboot {
        /// Offline disk image (qcow2/raw/…)
        image: Option<PathBuf>,
        /// Target hypervisor profile
        #[arg(long, default_value = "kvm")]
        target: String,
        /// QGA unix socket for live guest-ping
        #[arg(long)]
        socket: Option<String>,
        /// libvirt XML or KubeVirt YAML to attach disk inventory
        #[arg(long)]
        domain: Option<PathBuf>,
        /// virtio-win tree (else GUESTKIT_VIRTIO_WIN)
        #[arg(long, value_name = "DIR")]
        virtio_win: Option<PathBuf>,
        /// Fail when offline score is below N (also requires no blockers)
        #[arg(long, value_name = "SCORE")]
        fail_below: Option<f64>,
        /// Write JSON report to FILE (stdout if omitted)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
    },

    /// Offline cloud-init datasource + NoCloud seed
    #[command(name = "cloud-init")]
    CloudInit {
        /// aws/ec2, azure, gcp/gce, openstack, nocloud
        target: String,
        image: PathBuf,
        #[arg(long, value_name = "FILE")]
        user_data: Option<PathBuf>,
        #[arg(long, value_name = "FILE")]
        meta_data: Option<PathBuf>,
        #[arg(long)]
        instance_id: Option<String>,
        /// Write network: {config: disabled}
        #[arg(long)]
        disable_network: bool,
        #[arg(short, long)]
        export: Option<PathBuf>,
    },

    #[command(name = "selinux-relabel")]
    SelinuxRelabel {
        image: PathBuf,
        #[arg(short, long)]
        export: Option<PathBuf>,
    },
    Sysprep {
        image: PathBuf,
        #[arg(long)]
        hostname: Option<String>,
        #[arg(long)]
        no_firstboot: bool,
        #[arg(short, long)]
        export: Option<PathBuf>,
    },
    Bitlocker {
        #[command(subcommand)]
        action: BitlockerAction,
    },
    /// Combined passport + SBOM + BitLocker + score gate
    Gate {
        #[arg(long)]
        passport: Option<PathBuf>,
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long, default_value = "kvm")]
        target: String,
        #[arg(long, default_value_t = 80.0)]
        fail_below: f64,
        #[arg(long)]
        sbom_old: Option<PathBuf>,
        #[arg(long)]
        sbom_new: Option<PathBuf>,
        #[arg(long)]
        rego: Option<PathBuf>,
        #[arg(long, default_value_t = true)]
        fail: bool,
        #[arg(short, long, default_value = "text")]
        output: String,
    },
    /// Sign/verify agent update manifests (`--features agent`)
    #[command(name = "agent-sign")]
    AgentSign {
        #[command(subcommand)]
        action: AgentSignAction,
    },
    #[command(name = "virtio-initramfs")]
    VirtioInitramfs {
        image: PathBuf,
        #[arg(long)]
        dracut: bool,
        #[arg(short, long)]
        export: Option<PathBuf>,
    },

    /// Forensic diff with security drift scoring
    #[command(name = "forensic-diff")]
    ForensicDiff {
        /// Before disk image
        old: PathBuf,

        /// After disk image
        new: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Optional SPDX / CycloneDX / inventory JSON (before)
        #[arg(long, value_name = "FILE")]
        sbom_old: Option<PathBuf>,

        /// Optional SPDX / CycloneDX / inventory JSON (after)
        #[arg(long, value_name = "FILE")]
        sbom_new: Option<PathBuf>,
    },

    /// Diff two SBOMs (SPDX, CycloneDX, or GuestKit inventory JSON)
    #[command(name = "sbom-diff")]
    SbomDiff {
        /// Before SBOM JSON
        old: PathBuf,
        /// After SBOM JSON
        new: PathBuf,
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,
        /// Exit 1 when any package was added, removed, or version-bumped
        #[arg(long)]
        fail_on_drift: bool,
    },

    /// Run GuestKit in-guest agent daemon (requires --features agent)
    Agent {
        /// Communication channel
        #[arg(long, value_enum, default_value = "virtio")]
        channel: AgentChannelArg,

        /// Virtio device path override
        #[arg(long, value_name = "PATH")]
        device: Option<String>,

        /// Reserved: vsock socket override
        #[arg(long, value_name = "PATH")]
        socket: Option<String>,

        /// Reserved: drop privileges to user
        #[arg(long, value_name = "USER")]
        user: Option<String>,
    },

    /// Host-side proxy to guest agent unix socket or vsock listener (requires --features agent)
    #[command(name = "agent-proxy")]
    AgentProxy {
        /// Libvirt channel unix socket path (optional when --vsock-port is set)
        #[arg(long, value_name = "PATH")]
        socket: Option<String>,

        /// Optional HTTP listen address (e.g. 127.0.0.1:8765)
        #[arg(long, value_name = "ADDR")]
        listen: Option<String>,

        /// Listen for guest vsock connections on this port (Linux host)
        #[arg(long, value_name = "PORT")]
        vsock_port: Option<u32>,
    },

    /// Raw QEMU guest-agent command over a unix socket (replaces `virsh qemu-agent-command`)
    #[command(name = "qga")]
    Qga {
        /// QGA channel unix socket. Auto-discovered when omitted.
        #[arg(long, value_name = "PATH")]
        socket: Option<String>,

        /// QGA execute name (e.g. guest-ping, guest-info, guest-exec)
        #[arg(long, value_name = "CMD")]
        execute: Option<String>,

        /// JSON object passed as QGA `arguments`
        #[arg(long, value_name = "JSON")]
        arguments: Option<String>,

        /// Raw QGA JSON body, e.g. '{"execute":"guest-ping"}'
        #[arg(long, value_name = "JSON")]
        raw: Option<String>,
    },

    /// GuestKit-native local QEMU lifecycle (define/plan/start — no libvirt)
    Vm {
        #[command(subcommand)]
        action: crate::vm::VmAction,
    },

    /// One-shot JSON-RPC call to guest agent unix socket (requires --features agent)
    #[command(name = "agent-call")]
    AgentCall {
        /// Libvirt channel unix socket path
        #[arg(long, value_name = "PATH")]
        socket: String,

        /// JSON-RPC method (e.g. guestkit.getEvidence)
        #[arg(long, value_name = "METHOD")]
        method: String,

        /// JSON params object
        #[arg(long, value_name = "JSON", default_value = "{}")]
        params: String,
    },

    /// Offline-inject the GuestKit agent into a disk image (Linux systemd unit
    /// or, with --windows, the Windows GuestKitAgent service via hivex).
    #[command(name = "agent-inject")]
    AgentInject {
        /// Disk image path
        image: PathBuf,

        /// Agent binary to install (guestkitd[.exe]); defaults to this binary
        #[arg(long, value_name = "PATH")]
        agent_binary: Option<PathBuf>,

        /// Install the Windows GuestKitAgent service (registry-write) instead of
        /// a Linux systemd unit. Requires a Windows agent_binary (.exe).
        #[arg(long)]
        windows: bool,

        /// Windows only: directory holding the virtio-serial driver
        /// (vioser.inf/.sys/.cat) to preinstall so the QGA channel device
        /// exists. Without it, an agent-only inject can't be reached over QGA
        /// unless the guest already has the driver.
        #[arg(long, value_name = "DIR")]
        virtio_serial_driver: Option<PathBuf>,

        /// systemd unit file (Linux only; defaults to the built-in template)
        #[arg(long, value_name = "PATH")]
        agent_unit: Option<PathBuf>,

        /// Show what would change without writing
        #[arg(long)]
        dry_run: bool,
    },
}

#[derive(Subcommand)]
enum PolicyAction {
    /// Check disk image against policy
    Check {
        /// Disk image path
        image: PathBuf,

        /// Policy file path (YAML)
        #[arg(short, long, value_name = "FILE")]
        policy: Option<PathBuf>,

        /// Use industry benchmark
        #[arg(short, long, value_name = "BENCHMARK")]
        benchmark: Option<String>,

        /// Generate example policy file
        #[arg(long)]
        example_policy: bool,

        /// Output format (text, json)
        #[arg(short = 'f', long, value_name = "FORMAT", default_value = "text")]
        format: String,

        /// Output file
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,

        /// Fail on any validation failure
        #[arg(long)]
        strict: bool,
    },

    /// Evaluate a Rego deny-policy against passport/facts JSON
    Rego {
        /// Rego source (default policies/cutover.rego if present)
        #[arg(long, value_name = "FILE")]
        rego: PathBuf,
        /// Input JSON (passport.json or policy facts)
        #[arg(long, value_name = "FILE")]
        input: PathBuf,
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,
        /// Exit 1 when any deny fired
        #[arg(long)]
        fail: bool,
    },
}

#[derive(Subcommand)]
enum BitlockerAction {
    Status {
        image: PathBuf,
    },
    Escrow {
        image: PathBuf,
        #[arg(long)]
        key_file: PathBuf,
        #[arg(short, long)]
        output: Option<PathBuf>,
        #[arg(long)]
        include_secret: bool,
        #[arg(long)]
        export_plan: Option<PathBuf>,
    },
}

#[derive(Subcommand)]
enum AgentSignAction {
    Keygen {
        #[arg(long)]
        seed: PathBuf,
        #[arg(long)]
        public: PathBuf,
    },
    Sign {
        manifest: PathBuf,
        #[arg(short, long)]
        output: PathBuf,
    },
    Verify {
        manifest: PathBuf,
        #[arg(long)]
        signature: PathBuf,
    },
}

#[derive(Subcommand)]
enum ImgAction {
    /// qemu-img info --output=json
    Info { image: PathBuf },
    /// qemu-img check (JSON). --repair runs `-r leaks`
    Check {
        image: PathBuf,
        #[arg(long)]
        repair: bool,
    },
    /// List qcow2 internal snapshots
    Snapshots { image: PathBuf },
    /// Create a qcow2 internal snapshot
    #[command(name = "snapshot-create")]
    SnapshotCreate {
        image: PathBuf,
        #[arg(long)]
        name: String,
    },
    /// Delete a qcow2 internal snapshot
    #[command(name = "snapshot-delete")]
    SnapshotDelete {
        image: PathBuf,
        #[arg(long)]
        name: String,
    },
    /// Apply (revert to) a qcow2 internal snapshot
    #[command(name = "snapshot-apply")]
    SnapshotApply {
        image: PathBuf,
        #[arg(long)]
        name: String,
    },
    /// qemu-img resize (size like +10G or 40G)
    Resize { image: PathBuf, size: String },
    /// qemu-img rebase -b BACKING
    Rebase {
        image: PathBuf,
        #[arg(long)]
        backing: PathBuf,
        /// Skip image-length comparison (`qemu-img rebase -u`)
        #[arg(long)]
        unsafe_mode: bool,
    },
    /// qemu-img commit (flatten overlay into backing)
    Commit { image: PathBuf },
}

#[derive(Subcommand)]
enum VirtioWinAction {
    /// List drivers resolved from a virtio-win tree
    List {
        #[arg(long)]
        tree: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
    /// Show missing critical drivers + migrate-repair hint
    Plan {
        #[arg(long)]
        tree: Option<PathBuf>,
        #[arg(long)]
        image: Option<PathBuf>,
        #[arg(long)]
        json: bool,
    },
}

#[derive(Subcommand)]
enum PassportAction {
    /// Emit a Cutover Passport from an offline disk image
    Emit {
        /// Disk image path
        image: PathBuf,

        /// Target platform (kvm, proxmox, kubevirt, aws, …)
        #[arg(long, value_name = "TARGET")]
        target: String,

        /// Output passport JSON path
        #[arg(short, long, value_name = "FILE")]
        output: PathBuf,

        /// Also write a directory bundle with passport + fix-plan YAML
        #[arg(long)]
        bundle: bool,

        /// Include SHA-256 of image contents (slow for large disks)
        #[arg(long)]
        content_hash: bool,

        /// Host virtio-win tree for DriverInject planning
        #[arg(long, value_name = "DIR")]
        virtio_win: Option<PathBuf>,

        /// Agent-proxy base URL for live attestation (e.g. http://127.0.0.1:8765)
        #[arg(long, value_name = "URL")]
        live_url: Option<String>,

        /// Ed25519 signing key (32 raw bytes or 64 hex chars). Requires --features agent.
        #[arg(long, value_name = "FILE")]
        sign_key: Option<PathBuf>,

        /// Issuer identity recorded on the passport (CI job, env, team)
        #[arg(long, value_name = "NAME")]
        issuer: Option<String>,

        /// Passport validity window from emit time (hours); sets expires_at
        #[arg(long, value_name = "HOURS")]
        expires_hours: Option<u64>,
    },

    /// Generate an Ed25519 seed + public key for passport signing (requires --features agent)
    Keygen {
        /// Output path for 32-byte seed (raw bytes; chmod 0600 on Unix)
        #[arg(long, value_name = "FILE")]
        seed: PathBuf,

        /// Output path for public key hex (64 chars + newline)
        #[arg(long, value_name = "FILE")]
        public: PathBuf,
    },

    /// Verify a Cutover Passport (CI gate)
    Verify {
        /// Passport JSON path
        passport: PathBuf,

        /// Fail when min(boot, migration) score is below this threshold
        #[arg(long, value_name = "SCORE")]
        fail_below: Option<f64>,

        /// Require a valid Ed25519 signature
        #[arg(long)]
        require_signature: bool,

        /// Optional public key hex (defaults to key embedded in passport)
        #[arg(long, value_name = "HEX")]
        public_key: Option<String>,

        /// Allowlist file: one Ed25519 public key hex per line (# comments ok)
        #[arg(long, value_name = "FILE")]
        trust_keys: Option<PathBuf>,

        /// Reject passports whose generated_at is older than this many hours
        #[arg(long, value_name = "HOURS")]
        max_age_hours: Option<u64>,
    },

    /// Verify a passport and write the h2kvmctl job document
    Handoff {
        /// Passport JSON path
        passport: PathBuf,
        /// Output YAML/JSON (default: <passport>.handoff.yaml)
        #[arg(short, long, value_name = "FILE")]
        output: Option<PathBuf>,
        #[arg(long, value_name = "SCORE")]
        fail_below: Option<f64>,
        #[arg(long)]
        require_signature: bool,
        #[arg(long, value_name = "HEX")]
        public_key: Option<String>,
        #[arg(long, value_name = "FILE")]
        trust_keys: Option<PathBuf>,
        #[arg(long, value_name = "HOURS")]
        max_age_hours: Option<u64>,
        /// Exit non-zero when the passport is not allowed to convert
        #[arg(long, default_value_t = true)]
        fail: bool,
        /// Allow writing a refused handoff without failing the process
        #[arg(long)]
        allow_refused: bool,
    },
}

#[derive(Subcommand)]
enum FleetAction {
    /// Analyze a directory of VM images
    Analyze {
        /// Directory containing disk images
        dir: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Scan subdirectories for disk images (default: top level only)
        #[arg(long)]
        recursive: bool,

        /// Parallel workers (default: min(4, CPUs); env GUESTKIT_FLEET_JOBS)
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,
    },

    /// Order a fleet's disk images into dependency-aware migration waves
    WavePlan {
        /// Directory containing disk images
        dir: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Scan subdirectories for disk images (default: top level only)
        #[arg(long)]
        recursive: bool,

        /// Parallel workers (default: min(4, CPUs); env GUESTKIT_FLEET_JOBS)
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,
    },

    /// Scheduled drift check: diff each VM's current evidence against its
    /// stored golden baseline (first run establishes the baseline)
    Watch {
        /// Directory containing disk images
        dir: PathBuf,

        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,

        /// Scan subdirectories for disk images (default: top level only)
        #[arg(long)]
        recursive: bool,

        /// Parallel workers (default: min(4, CPUs); env GUESTKIT_FLEET_JOBS)
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,

        /// Overwrite each VM's stored baseline with its current evidence
        /// instead of diffing against it (use after a reviewed, accepted change)
        #[arg(long)]
        reset_baseline: bool,

        /// Exit non-zero if any VM has drifted from its baseline (for cron/CI gating)
        #[arg(long)]
        fail_on_drift: bool,
    },

    /// Hold VMs below a doctor score out of the hyper2kvm wave
    Quarantine {
        /// Directory containing disk images
        dir: PathBuf,
        /// Output format (text, json)
        #[arg(short, long, value_name = "FORMAT", default_value = "text")]
        output: String,
        /// Scan subdirectories
        #[arg(long)]
        recursive: bool,
        /// Parallel workers
        #[arg(short = 'j', long, value_name = "N")]
        jobs: Option<usize>,
        /// Quarantine when boot score is below this (default 80)
        #[arg(long, default_value_t = 80.0)]
        threshold: f64,
        /// Exit non-zero if anyone is quarantined
        #[arg(long)]
        fail: bool,
    },
}

#[derive(clap::ValueEnum, Clone)]
enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    PowerShell,
    Elvish,
}

#[derive(clap::ValueEnum, Clone, Debug)]
enum SnapshotOperation {
    Create,
    List,
    Delete,
    Revert,
    Info,
}

/// Run standalone file explorer (direct from CLI)
fn run_standalone_explorer(
    image_path: &Path,
    start_path: &str,
    verbose: bool,
) -> anyhow::Result<()> {
    use crate::cli::shell::commands::ShellContext;
    use crate::cli::shell::explore::run_explorer;
    use crate::Guestfs;

    if verbose {
        println!("{} Loading VM image: {}", "→".cyan(), image_path.display());
    }

    // Initialize guestfs
    let mut guestfs = Guestfs::new().context("Failed to create Guestfs handle")?;

    guestfs
        .add_drive_opts(
            image_path
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
        anyhow::bail!("No operating systems found in disk image");
    }

    let root = &roots[0];

    if verbose {
        println!("{} Detected OS: {}", "→".cyan(), root.yellow());
    }

    let mounts = guestfs
        .inspect_get_mountpoints(root)
        .context("Failed to get mountpoints")?;

    for (mountpoint, device) in mounts {
        if let Err(e) = guestfs.mount(&device, &mountpoint) {
            eprintln!("{} Failed to mount {}: {}", "⚠".yellow(), mountpoint, e);
        }
    }

    if verbose {
        println!("{} VM filesystem mounted successfully", "✓".green());
    }

    // Get OS information for context
    let os_product = guestfs
        .inspect_get_product_name(root)
        .unwrap_or_else(|_| "Unknown OS".to_string());

    // Create shell context for explorer
    let mut ctx = ShellContext::new(guestfs, root.to_string());
    ctx.set_os_info(os_product);
    ctx.current_path = start_path.to_string();

    // Launch explorer
    println!(
        "\n{}",
        "╔═══════════════════════════════════════════════════════════╗".cyan()
    );
    println!(
        "{}",
        "║          GuestKit File Explorer (TUI Mode)              ║"
            .cyan()
            .bold()
    );
    println!(
        "{}",
        "╚═══════════════════════════════════════════════════════════╝".cyan()
    );
    println!();
    println!("{} Press 'h' for help, 'q' to quit", "ℹ".yellow());
    println!();

    run_explorer(&mut ctx, Some(start_path))?;

    println!("\n{} Explorer closed", "✓".green());

    Ok(())
}

/// Set environment variables based on CLI flags.
///
/// # Safety
/// Must be called before any threads are spawned (no async runtime, no thread pool).
/// `std::env::set_var` is unsafe in multi-threaded contexts.
unsafe fn set_env_vars_before_threads(cli: &Cli) -> anyhow::Result<()> {
    if cli.debug {
        std::env::set_var("GUESTKIT_DEBUG", "1");
    }
    if cli.no_color || cli.machine_readable {
        std::env::set_var("NO_COLOR", "1");
    }
    if cli.read_only {
        std::env::set_var("GUESTKIT_READONLY", "1");
    }
    if let Some(ref cache_dir) = cli.cache_dir {
        std::env::set_var(
            "GUESTKIT_CACHE_DIR",
            cache_dir
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Cache directory path contains invalid UTF-8"))?,
        );
    }
    if cli.timeout > 0 {
        std::env::set_var("GUESTKIT_TIMEOUT", cli.timeout.to_string());
    }
    Ok(())
}

/// Run the CLI (shared by `guestkit` and `guestctl` binaries).
pub fn run() -> anyhow::Result<()> {
    let raw: Vec<String> = std::env::args().collect();
    let processed = invocation::preprocess_args(raw);
    let os_args: Vec<OsString> = processed.into_iter().map(OsString::from).collect();

    let mut cmd = Cli::command();
    let bin = invocation::name();
    cmd = cmd.name(bin).bin_name(bin);
    let matches = cmd.get_matches_from(os_args);
    let cli = Cli::from_arg_matches(&matches)?;

    // Setup global environment variables.
    // SAFETY: These set_var calls happen in main() before any threads are spawned
    // or the async runtime is created. Clap parsing is synchronous and single-threaded.
    // This function must not be moved after tokio runtime initialization.
    unsafe {
        set_env_vars_before_threads(&cli)?;
    }

    // Setup logging
    let log_level = if cli.quiet {
        log::LevelFilter::Error
    } else if cli.verbose {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    let mut logger = env_logger::Builder::new();
    logger.filter_level(log_level);

    if cli.timestamps {
        logger.format_timestamp_secs();
    } else {
        logger.format_timestamp(None);
    }

    logger.init();

    let Some(command) = cli.command else {
        welcome::print_welcome();
        return Ok(());
    };

    match command {
        Commands::Inspect {
            image,
            output,
            profile,
            export,
            export_output,
            no_cache,
            cache_refresh,
            summary: _,
            include_packages: _,
            include_services: _,
            include_network: _,
            depth: _,
            save_report: _,
        } => {
            use crate::cli::formatters::OutputFormat;
            let output_format = output
                .as_ref()
                .map(|s| s.parse::<OutputFormat>())
                .transpose()
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            inspect_image(
                &image,
                cli.verbose,
                cli.debug,
                output_format,
                profile,
                export,
                export_output,
                !no_cache, // Cache enabled by default, disabled with --no-cache
                cache_refresh,
            )?;
        }

        Commands::Diff {
            image1,
            image2,
            output,
        } => {
            use crate::cli::formatters::OutputFormat;
            let output_format = output
                .as_ref()
                .map(|s| s.parse::<OutputFormat>())
                .transpose()
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            diff_images(&image1, &image2, cli.verbose, output_format)?;
        }

        Commands::Compare { baseline, images } => {
            compare_images(&baseline, &images, cli.verbose)?;
        }

        Commands::List {
            image,
            path,
            recursive,
            long,
            all,
            human_readable,
            sort_time,
            reverse,
            filter,
            directories_only,
            limit,
        } => {
            list_files_enhanced(
                &image,
                &path,
                recursive,
                long,
                all,
                human_readable,
                sort_time,
                reverse,
                filter,
                directories_only,
                limit,
                cli.verbose,
            )?;
        }

        Commands::Extract {
            image,
            guest_path,
            output,
            preserve,
            recursive,
            force,
            progress,
            verify,
        } => {
            extract_file_enhanced(
                &image,
                &guest_path,
                &output,
                preserve,
                recursive,
                force,
                progress,
                verify,
                cli.verbose,
            )?;
        }

        Commands::Execute { image, command } => {
            execute_command(&image, &command, cli.verbose)?;
        }

        Commands::Backup {
            image,
            path,
            output,
        } => {
            backup_files(&image, &path, &output, cli.verbose)?;
        }

        Commands::Create { path, size, format } => {
            create_disk(&path, size, &format, cli.verbose)?;
        }

        Commands::Check { image, device } => {
            check_filesystem(&image, device, cli.verbose)?;
        }

        Commands::Usage { image } => {
            show_disk_usage(&image, cli.verbose)?;
        }

        Commands::Shrink {
            image,
            dry_run,
            min_ratio,
            headroom_pct,
            json,
        } => {
            shrink_command(&image, dry_run, min_ratio, headroom_pct, json, cli.verbose)?;
        }

        Commands::Convert {
            source,
            output,
            format,
            compress,
            flatten,
            progress: _,
            verify: _,
            sparse: _,
            preallocate: _,
            compression_level: _,
            buffer_size: _,
        } => {
            log::info!("Converting {} -> {}", source.display(), output.display());

            let converter = DiskConverter::new();
            let result = converter.convert(&source, &output, &format, compress, flatten)?;

            if result.success {
                println!("✓ Conversion successful!");
                println!(
                    "  Source:  {} ({})",
                    source.display(),
                    result.source_format.as_str()
                );
                println!(
                    "  Output:  {} ({})",
                    output.display(),
                    result.output_format.as_str()
                );
                println!("  Size:    {} bytes", result.output_size);
                println!("  Time:    {:.2}s", result.duration_secs);
            } else {
                eprintln!("✗ Conversion failed: {}", result.error.unwrap_or_default());
                std::process::exit(1);
            }
        }

        Commands::Detect { image } => {
            let converter = DiskConverter::new();
            let format = converter.detect_format(&image)?;

            println!("Detected format: {}", format.as_str());
        }

        Commands::Info { image } => {
            let converter = DiskConverter::new();
            let info = converter.get_info(&image)?;

            println!("{}", serde_json::to_string_pretty(&info)?);
        }

        Commands::InspectBatch {
            images,
            parallel,
            output,
            no_cache,
        } => {
            use crate::cli::formatters::OutputFormat;
            let output_format = output
                .as_ref()
                .map(|s| s.parse::<OutputFormat>())
                .transpose()
                .map_err(|e| anyhow::anyhow!("{}", e))?;

            inspect_batch(
                &images,
                parallel as usize,
                cli.verbose,
                output_format,
                !no_cache,
            )?; // Cache enabled by default
        }

        Commands::CacheClear => {
            use crate::cli::cache::InspectionCache;
            let cache = InspectionCache::new()?;
            let count = cache.clear_all()?;

            println!("✓ Cleared {} cached inspection results", count);
        }

        Commands::CacheStats => {
            use crate::cli::cache::InspectionCache;
            let cache = InspectionCache::new()?;
            let stats = cache.stats()?;

            println!("Cache Statistics:");
            println!("  Entries: {}", stats.entries);
            println!("  Total Size: {}", stats.size_human());
        }

        Commands::Filesystems { image, detailed } => {
            list_filesystems(&image, detailed, cli.verbose)?;
        }

        Commands::Packages {
            image,
            filter,
            limit,
            json,
        } => {
            list_packages(&image, filter, limit, json, cli.verbose)?;
        }

        Commands::Cat {
            image,
            path,
            line_numbers,
            show_all,
        } => {
            cat_file_enhanced(&image, &path, line_numbers, show_all, cli.verbose)?;
        }

        Commands::Search {
            image,
            pattern,
            path,
            regex,
            ignore_case,
            content,
            file_type,
            max_depth,
            limit,
        } => {
            search_command(
                &image,
                &pattern,
                &path,
                regex,
                ignore_case,
                content,
                file_type,
                max_depth,
                limit,
                cli.verbose,
            )?;
        }

        Commands::Grep {
            image,
            pattern,
            path,
            ignore_case,
            line_numbers,
            recursive,
            files_only,
            invert,
            before_context,
            after_context,
            max_count,
        } => {
            grep_command(
                &image,
                &pattern,
                &path,
                ignore_case,
                line_numbers,
                recursive,
                files_only,
                invert,
                before_context,
                after_context,
                max_count,
                cli.verbose,
            )?;
        }

        Commands::Hash {
            image,
            path,
            algorithm,
            check,
            recursive,
        } => {
            hash_command(&image, &path, &algorithm, check, recursive, cli.verbose)?;
        }

        Commands::Scan {
            image,
            scan_type,
            severity,
            output,
            report,
            check_cve,
        } => {
            scan_command(
                &image,
                &scan_type,
                severity,
                output,
                report,
                check_cve,
                cli.verbose,
            )?;
        }

        Commands::Benchmark {
            image,
            test_type,
            block_size,
            duration,
            iterations,
        } => {
            benchmark_command(
                &image,
                &test_type,
                block_size as usize,
                duration,
                iterations,
                cli.verbose,
            )?;
        }

        Commands::Snapshot {
            image,
            operation,
            name,
            description,
        } => {
            let op_str = match operation {
                SnapshotOperation::Create => "create",
                SnapshotOperation::List => "list",
                SnapshotOperation::Delete => "delete",
                SnapshotOperation::Revert => "revert",
                SnapshotOperation::Info => "info",
            };
            snapshot_command(&image, op_str, name, description, cli.verbose)?;
        }

        Commands::DiffFiles {
            image1,
            image2,
            path,
            unified,
            context,
            ignore_whitespace,
        } => {
            diff_command(
                &image1,
                &image2,
                &path,
                unified,
                context,
                ignore_whitespace,
                cli.verbose,
            )?;
        }

        Commands::FindLarge {
            image,
            path,
            min_size,
            max_results,
            human_readable,
        } => {
            find_large_command(
                &image,
                &path,
                min_size,
                max_results as usize,
                human_readable,
                cli.verbose,
            )?;
        }

        Commands::Copy {
            source_image,
            source_path,
            dest_image,
            dest_path,
            preserve,
            force,
        } => {
            copy_command(
                &source_image,
                &source_path,
                &dest_image,
                &dest_path,
                preserve,
                force,
                cli.verbose,
            )?;
        }

        Commands::FindDuplicates {
            image,
            path,
            min_size,
            algorithm,
        } => {
            find_duplicates_command(&image, &path, min_size, &algorithm, cli.verbose)?;
        }

        Commands::DiskUsage {
            image,
            path,
            max_depth,
            min_size,
            human_readable,
        } => {
            disk_usage_command(
                &image,
                &path,
                max_depth,
                min_size,
                human_readable,
                cli.verbose,
            )?;
        }

        Commands::Timeline {
            image,
            start_time,
            end_time,
            sources,
            format,
        } => {
            timeline_command(&image, start_time, end_time, sources, &format, cli.verbose)?;
        }

        Commands::Fingerprint {
            image,
            algorithm,
            include_content,
            output,
        } => {
            fingerprint_command(&image, &algorithm, include_content, output, cli.verbose)?;
        }

        Commands::Drift {
            baseline,
            current,
            ignore_paths,
            threshold,
            report,
        } => {
            drift_command(
                &baseline,
                &current,
                ignore_paths,
                threshold,
                report,
                cli.verbose,
            )?;
        }

        Commands::Analyze {
            image,
            focus,
            depth,
            suggestions,
        } => {
            analyze_command(&image, focus, &depth, suggestions, cli.verbose)?;
        }

        Commands::Secrets {
            image,
            scan_paths,
            patterns,
            exclude,
            show_content,
            export,
        } => {
            secrets_command(
                &image,
                scan_paths,
                patterns,
                exclude,
                show_content,
                export,
                cli.verbose,
            )?;
        }

        Commands::Rescue {
            image,
            operation,
            user,
            password,
            force,
            backup,
            key,
            key_file,
            hostname,
            timezone,
            export_plan,
            packages,
            network,
        } => {
            rescue_command(
                &image,
                &operation,
                user,
                password,
                force,
                backup,
                cli.verbose,
                key,
                key_file,
                hostname,
                timezone,
                export_plan,
                packages,
                network,
            )?;
        }

        Commands::Optimize {
            image,
            operations,
            aggressive,
            dry_run,
        } => {
            optimize_command(&image, operations, aggressive, dry_run, cli.verbose)?;
        }

        Commands::Network {
            image,
            show_routes,
            show_interfaces,
            show_dns,
            export_json,
        } => {
            network_command(
                &image,
                show_routes,
                show_interfaces,
                show_dns,
                export_json,
                cli.verbose,
            )?;
        }

        Commands::Compliance {
            image,
            standard,
            profile,
            export,
            fix,
        } => {
            compliance_command(&image, &standard, profile, export, fix, cli.verbose)?;
        }

        Commands::Malware {
            image,
            deep_scan,
            check_rootkits,
            yara_rules,
            quarantine,
        } => {
            malware_command(
                &image,
                deep_scan,
                check_rootkits,
                yara_rules,
                quarantine,
                cli.verbose,
            )?;
        }

        Commands::Health {
            image,
            checks,
            detailed,
            export_json,
        } => {
            health_command(&image, checks, detailed, export_json, cli.verbose)?;
        }

        Commands::Clone {
            source,
            dest,
            sysprep,
            hostname,
            remove_keys,
            preserve_users,
            lvm,
            source_vg,
            source_lv,
            target_vg,
            regenerate_uuids,
            update_fstab,
            update_bootloader,
            dry_run,
            snapshot_size,
            clone_name,
            update_crypttab,
            regenerate_initramfs,
            isolation_level,
            verify_security,
            regenerate_grub,
            verify_boot,
        } => {
            if lvm {
                lvm_clone_command(
                    source_vg,
                    source_lv,
                    clone_name,
                    target_vg,
                    regenerate_uuids,
                    update_fstab,
                    update_bootloader,
                    update_crypttab,
                    hostname,
                    dry_run,
                    snapshot_size,
                    regenerate_initramfs,
                    &isolation_level,
                    verify_security,
                    regenerate_grub,
                    verify_boot,
                    cli.verbose,
                )?;
            } else {
                let source = source
                    .ok_or_else(|| anyhow::anyhow!("SOURCE is required for disk image clone"))?;
                let dest =
                    dest.ok_or_else(|| anyhow::anyhow!("DEST is required for disk image clone"))?;
                clone_command(
                    &source,
                    &dest,
                    sysprep,
                    hostname,
                    remove_keys,
                    preserve_users,
                    cli.verbose,
                )?;
            }
        }

        Commands::Patch {
            image,
            check_cves,
            severity,
            export,
            simulate_update,
            simulate,
        } => {
            patch_command(
                &image,
                check_cves,
                severity,
                export,
                simulate_update,
                simulate,
                cli.verbose,
            )?;
        }

        Commands::Inventory {
            image,
            format,
            output,
            include_licenses,
            include_files,
            include_cves,
            severity,
            summary,
        } => {
            inventory_command(
                &image,
                &format,
                output.as_deref().and_then(|p| p.to_str()),
                include_licenses,
                include_files,
                include_cves,
                severity,
                summary,
                cli.verbose,
            )?;
        }

        Commands::Validate {
            image,
            policy,
            benchmark,
            example_policy,
            format,
            output,
            strict,
        } => {
            validate_command(
                &image,
                policy.as_deref(),
                benchmark,
                example_policy,
                &format,
                output.as_deref(),
                strict,
                cli.verbose,
            )?;
        }

        Commands::License {
            image,
            format,
            output,
            prohibit,
            details,
            attribution,
            strict,
        } => {
            license_command(
                &image,
                &format,
                output.as_deref(),
                &prohibit,
                details,
                attribution,
                strict,
                cli.verbose,
            )?;
        }

        Commands::Blueprint {
            image,
            format,
            output,
            provider,
        } => {
            blueprint_command(
                &image,
                &format,
                output.as_deref(),
                provider.as_deref(),
                cli.verbose,
            )?;
        }

        Commands::Migrate {
            image,
            target_type,
            target,
            version,
            format,
            output,
            detailed,
        } => {
            migrate_command(
                &image,
                &target_type,
                &target,
                version.as_deref(),
                &format,
                output.as_deref(),
                detailed,
                cli.verbose,
            )?;
        }

        Commands::Cost {
            image,
            provider,
            region,
            format,
            output,
            detailed,
        } => {
            cost_command(
                &image,
                &provider,
                &region,
                &format,
                output.as_deref(),
                detailed,
                cli.verbose,
            )?;
        }

        Commands::Dependencies {
            image,
            format,
            output,
            detailed,
            package,
            reverse,
            max_depth,
            show_all,
        } => {
            dependencies_command(
                &image,
                &format,
                output.as_deref(),
                detailed,
                package.as_deref(),
                reverse,
                max_depth,
                show_all,
                cli.verbose,
            )?;
        }

        Commands::Audit {
            image,
            categories,
            output_format,
            export,
            fix_issues,
        } => {
            audit_command(
                &image,
                categories,
                &output_format,
                export,
                fix_issues,
                cli.verbose,
            )?;
        }

        Commands::Repair {
            image,
            repair_type,
            fix,
            dry_run,
            force,
            backup,
            inject_agent,
            agent_binary,
            agent_unit,
        } => {
            if fix.as_deref() == Some("boot") {
                #[cfg(feature = "agent")]
                {
                    repair_boot_command(
                        &image,
                        dry_run,
                        cli.verbose,
                        inject_agent,
                        agent_binary.as_deref(),
                        agent_unit.as_deref(),
                    )?;
                }
                #[cfg(not(feature = "agent"))]
                {
                    let _ = (&agent_binary, &agent_unit);
                    repair_boot_command(&image, dry_run, cli.verbose, inject_agent)?;
                }
            } else if let Some(rt) = repair_type {
                repair_command(&image, &rt, force, backup, cli.verbose)?;
            } else {
                anyhow::bail!("Specify --fix boot or --repair-type <type>");
            }
        }

        Commands::Harden {
            image,
            profile,
            apply,
            preview,
        } => {
            harden_command(&image, &profile, apply, preview, cli.verbose)?;
        }

        Commands::Anomaly {
            image,
            baseline,
            sensitivity,
            categories,
            export,
        } => {
            anomaly_command(
                &image,
                baseline,
                &sensitivity,
                categories,
                export,
                cli.verbose,
            )?;
        }

        Commands::Recommend {
            image,
            focus,
            priority,
            apply,
        } => {
            recommend_command(&image, focus, &priority, apply, cli.verbose)?;
        }

        Commands::Predict {
            image,
            metric,
            timeframe,
            export,
        } => {
            predict_command(&image, &metric, timeframe, export, cli.verbose)?;
        }

        Commands::Intelligence {
            image,
            ioc_file,
            threat_level,
            correlate,
            export,
            simulate,
        } => {
            intelligence_command(
                &image,
                ioc_file,
                &threat_level,
                correlate,
                export,
                simulate,
                cli.verbose,
            )?;
        }

        Commands::Simulate {
            image,
            change_type,
            target,
            dry_run,
            risk_assessment,
        } => {
            simulate_command(
                &image,
                &change_type,
                target,
                dry_run,
                risk_assessment,
                cli.verbose,
            )?;
        }

        Commands::Score {
            image,
            dimensions,
            weights,
            benchmark,
            export,
        } => {
            score_command(&image, dimensions, weights, benchmark, export, cli.verbose)?;
        }

        Commands::Template {
            image,
            template,
            strict,
            fix,
            export_template,
        } => {
            template_command(&image, &template, strict, fix, export_template, cli.verbose)?;
        }

        Commands::Hunt {
            image,
            hypothesis,
            framework,
            techniques,
            depth,
            export,
        } => {
            hunt_command(
                &image,
                hypothesis,
                &framework,
                techniques,
                &depth,
                export,
                cli.verbose,
            )?;
        }

        Commands::Reconstruct {
            image,
            incident_type,
            start_time,
            end_time,
            visualize,
            export,
        } => {
            reconstruct_command(
                &image,
                &incident_type,
                start_time,
                end_time,
                visualize,
                export,
                cli.verbose,
            )?;
        }

        Commands::Evolve {
            image,
            target_state,
            strategy,
            stages,
            safety_checks,
            export_plan,
        } => {
            evolve_command(
                &image,
                &target_state,
                &strategy,
                stages,
                safety_checks,
                export_plan,
                cli.verbose,
            )?;
        }

        Commands::Verify {
            image,
            verification_level,
            check_supply_chain,
            check_identity,
            check_integrity,
            export,
        } => {
            verify_command(
                &image,
                &verification_level,
                check_supply_chain,
                check_identity,
                check_integrity,
                export,
                cli.verbose,
            )?;
        }

        Commands::Version => {
            println!("{} {}", invocation::name(), VERSION);
            println!("A modern VM disk inspection and manipulation toolkit");
            println!();
            println!("Project: https://github.com/zyvorai/guestkit");
            println!("License: Apache-2.0");
            println!("Copyright: ZyvorAI Labs Private Limited");
        }

        Commands::CommandCatalog => {
            commands_list::print_grouped_commands(invocation::name());
        }

        Commands::Interactive { image } => {
            let mut session = crate::cli::InteractiveSession::new(image)?;
            session.run()?;
        }

        Commands::Explore { image, path } => {
            run_standalone_explorer(&image, &path, cli.verbose)?;
        }

        Commands::Script {
            image,
            script,
            fail_fast,
        } => {
            let mut executor = crate::cli::BatchExecutor::new(image, fail_fast, cli.verbose)?;
            let report = executor.execute_script(&script)?;
            report.print();
            std::process::exit(report.exit_code());
        }

        Commands::SystemdJournal {
            image,
            priority,
            unit,
            errors,
            warnings,
            stats,
            limit,
        } => {
            systemd_journal_command(
                &image,
                priority,
                unit.as_deref(),
                errors,
                warnings,
                stats,
                limit,
                cli.verbose,
            )?;
        }

        Commands::SystemdServices {
            image,
            service,
            failed,
            diagram,
            output,
        } => {
            systemd_services_command(
                &image,
                service.as_deref(),
                failed,
                diagram,
                output.as_deref(),
                cli.verbose,
            )?;
        }

        Commands::SystemdBoot {
            image,
            timeline,
            recommendations,
            summary,
            top,
        } => {
            systemd_boot_command(&image, timeline, recommendations, summary, top, cli.verbose)?;
        }

        Commands::Tui {
            image,
            compare,
            fleet,
        } => {
            crate::cli::tui::run_tui(&image, compare.as_deref(), fleet.as_deref())?;
        }

        Commands::Shell { image } => {
            crate::cli::shell::run_interactive_shell(&image)?;
        }

        Commands::Ai { image, query } => {
            crate::cli::ai::run_ai_assistant(&image, &query)?;
        }

        Commands::Completion { shell, all } => {
            let names: Vec<&str> = if all {
                vec!["guestkit", "guestctl"]
            } else {
                vec![invocation::name()]
            };
            for bin_name in names {
                let mut cmd = Cli::command();
                cmd = cmd.name(bin_name).bin_name(bin_name);
                match shell {
                    CompletionShell::Bash => {
                        generate(shells::Bash, &mut cmd, bin_name, &mut io::stdout())
                    }
                    CompletionShell::Zsh => {
                        generate(shells::Zsh, &mut cmd, bin_name, &mut io::stdout())
                    }
                    CompletionShell::Fish => {
                        generate(shells::Fish, &mut cmd, bin_name, &mut io::stdout())
                    }
                    CompletionShell::PowerShell => {
                        generate(shells::PowerShell, &mut cmd, bin_name, &mut io::stdout())
                    }
                    CompletionShell::Elvish => {
                        generate(shells::Elvish, &mut cmd, bin_name, &mut io::stdout())
                    }
                }
            }
        }

        Commands::Plan(plan_cmd) => {
            plan_cmd.execute()?;
        }

        Commands::Doctor {
            image,
            target,
            explain,
            ai,
            output,
            fail_below,
        } => {
            doctor_command(
                &image,
                &target,
                explain,
                ai,
                &output,
                fail_below,
                cli.verbose,
            )?;
        }

        #[cfg(feature = "mcp")]
        Commands::McpServe { image, target } => {
            mcp_serve_command(&image, &target, cli.verbose)?;
        }

        Commands::CloudProfile {
            target,
            export,
            image,
            strict,
        } => {
            let profile = crate::cli::validate::cloud_profiles::CloudProfile::parse(&target)
                .ok_or_else(|| {
                    anyhow::anyhow!("unknown cloud profile '{target}' (aws|azure|gcp|openstack)")
                })?;
            let policy = profile.to_policy();
            if let Some(path) = export {
                std::fs::write(&path, serde_yaml::to_string(&policy)?)?;
                println!("wrote {} profile to {}", profile.name(), path.display());
            } else {
                println!("{}", serde_yaml::to_string(&policy)?);
            }
            if let Some(img) = image {
                policy_check_command(
                    &img,
                    None,
                    Some(profile.name().to_string()),
                    false,
                    "text",
                    None,
                    strict,
                    cli.verbose,
                )?;
            }
        }

        Commands::Policy { action } => match action {
            PolicyAction::Check {
                image,
                policy,
                benchmark,
                example_policy,
                format,
                output,
                strict,
            } => {
                policy_check_command(
                    &image,
                    policy.as_deref(),
                    benchmark,
                    example_policy,
                    &format,
                    output.as_deref(),
                    strict,
                    cli.verbose,
                )?;
            }
            PolicyAction::Rego {
                rego,
                input,
                output,
                fail,
            } => {
                let raw = std::fs::read_to_string(&input)
                    .with_context(|| format!("read {}", input.display()))?;
                let facts = crate::cli::validate::rego::facts_from_passport_json(&raw)?;
                let report = crate::cli::validate::rego::eval_file(&rego, &facts)?;
                if output == "json" {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!(
                        "rego {} engine={} allowed={}",
                        report.package, report.engine, report.allowed
                    );
                    for d in &report.denies {
                        println!("  deny: {d}");
                    }
                }
                if fail && !report.allowed {
                    anyhow::bail!("{} deny rule(s) fired", report.denies.len());
                }
            }
        },

        Commands::Fleet { action } => match action {
            FleetAction::Analyze {
                dir,
                output,
                recursive,
                jobs,
            } => {
                let jobs = jobs
                    .or_else(|| {
                        std::env::var("GUESTKIT_FLEET_JOBS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap_or_else(|| {
                        std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(2)
                            .clamp(1, 4)
                    });
                fleet_analyze_command(&dir, &output, recursive, jobs, cli.verbose)?;
            }
            FleetAction::WavePlan {
                dir,
                output,
                recursive,
                jobs,
            } => {
                let jobs = jobs
                    .or_else(|| {
                        std::env::var("GUESTKIT_FLEET_JOBS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap_or_else(|| {
                        std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(2)
                            .clamp(1, 4)
                    });
                fleet_wave_plan_command(&dir, &output, recursive, jobs, cli.verbose)?;
            }
            FleetAction::Watch {
                dir,
                output,
                recursive,
                jobs,
                reset_baseline,
                fail_on_drift,
            } => {
                let jobs = jobs
                    .or_else(|| {
                        std::env::var("GUESTKIT_FLEET_JOBS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap_or_else(|| {
                        std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(2)
                            .clamp(1, 4)
                    });
                fleet_watch_command(
                    &dir,
                    &output,
                    recursive,
                    jobs,
                    reset_baseline,
                    fail_on_drift,
                    cli.verbose,
                )?;
            }
            FleetAction::Quarantine {
                dir,
                output,
                recursive,
                jobs,
                threshold,
                fail,
            } => {
                let jobs = jobs
                    .or_else(|| {
                        std::env::var("GUESTKIT_FLEET_JOBS")
                            .ok()
                            .and_then(|v| v.parse().ok())
                    })
                    .unwrap_or_else(|| {
                        std::thread::available_parallelism()
                            .map(|n| n.get())
                            .unwrap_or(2)
                            .clamp(1, 4)
                    });
                crate::cli::commands::assurance::fleet_quarantine_command(
                    &dir,
                    &output,
                    recursive,
                    jobs,
                    threshold,
                    fail,
                    cli.verbose,
                )?;
            }
        },

        Commands::MigratePlan {
            image,
            target,
            explain,
            ai,
            output,
            export,
            inject_agent,
            agent_binary,
            agent_unit,
        } => {
            #[cfg(feature = "agent")]
            {
                migrate_plan_command(
                    &image,
                    &target,
                    explain,
                    ai,
                    &output,
                    export.as_deref(),
                    cli.verbose,
                    inject_agent,
                    agent_binary.as_deref(),
                    agent_unit.as_deref(),
                )?;
            }
            #[cfg(not(feature = "agent"))]
            {
                let _ = (&agent_binary, &agent_unit);
                migrate_plan_command(
                    &image,
                    &target,
                    explain,
                    ai,
                    &output,
                    export.as_deref(),
                    cli.verbose,
                    inject_agent,
                )?;
            }
        }

        Commands::MigrateAssess {
            image,
            target,
            output,
            fail_below,
        } => {
            crate::cli::commands::assurance::migrate_assess_command(
                &image,
                &target,
                &output,
                fail_below,
                cli.verbose,
            )?;
        }

        Commands::MigrateRepair {
            image,
            target,
            apply,
            include_destructive,
            virtio_win,
            export,
        } => {
            crate::cli::commands::assurance::migrate_repair_command(
                &image,
                &target,
                apply,
                include_destructive,
                virtio_win.as_deref(),
                export.as_deref(),
                cli.verbose,
            )?;
        }

        Commands::Passport { action } => match action {
            PassportAction::Emit {
                image,
                target,
                output,
                bundle,
                content_hash,
                virtio_win,
                live_url,
                sign_key,
                issuer,
                expires_hours,
            } => {
                crate::cli::commands::assurance::passport_emit_command(
                    &image,
                    &target,
                    &output,
                    bundle,
                    content_hash,
                    virtio_win.as_deref(),
                    live_url.as_deref(),
                    sign_key.as_deref(),
                    issuer.as_deref(),
                    expires_hours,
                    cli.verbose,
                )?;
            }
            PassportAction::Keygen { seed, public } => {
                crate::cli::commands::assurance::passport_keygen_command(&seed, &public)?;
            }
            PassportAction::Verify {
                passport,
                fail_below,
                require_signature,
                public_key,
                trust_keys,
                max_age_hours,
            } => {
                crate::cli::commands::assurance::passport_verify_command(
                    &passport,
                    fail_below,
                    require_signature,
                    public_key.as_deref(),
                    trust_keys.as_deref(),
                    max_age_hours,
                )?;
            }
            PassportAction::Handoff {
                passport,
                output,
                fail_below,
                require_signature,
                public_key,
                trust_keys,
                max_age_hours,
                fail,
                allow_refused,
            } => {
                crate::cli::commands::assurance::passport_handoff_command(
                    &passport,
                    output.as_deref(),
                    fail_below,
                    require_signature,
                    public_key.as_deref(),
                    trust_keys.as_deref(),
                    max_age_hours,
                    fail && !allow_refused,
                )?;
            }
        },

        Commands::Img { action } => match action {
            ImgAction::Info { image } => crate::cli::img::info(&image)?,
            ImgAction::Check { image, repair } => crate::cli::img::check(&image, repair)?,
            ImgAction::Snapshots { image } => crate::cli::img::snapshots(&image)?,
            ImgAction::SnapshotCreate { image, name } => {
                crate::cli::img::snapshot_create(&image, &name)?
            }
            ImgAction::SnapshotDelete { image, name } => {
                crate::cli::img::snapshot_delete(&image, &name)?
            }
            ImgAction::SnapshotApply { image, name } => {
                crate::cli::img::snapshot_apply(&image, &name)?
            }
            ImgAction::Resize { image, size } => crate::cli::img::resize(&image, &size)?,
            ImgAction::Rebase {
                image,
                backing,
                unsafe_mode,
            } => crate::cli::img::rebase(&image, &backing, unsafe_mode)?,
            ImgAction::Commit { image } => crate::cli::img::commit(&image)?,
        },

        Commands::DomainDisks {
            file,
            files_only,
            json,
        } => {
            let report = crate::cli::domain_disks::parse_domain_disks(&file)?;
            if files_only {
                for p in crate::cli::domain_disks::file_sources(&report) {
                    println!("{}", p.display());
                }
            } else {
                let _ = json;
                println!("{}", serde_json::to_string_pretty(&report)?);
            }
        }

        Commands::VirtioWin { action } => match action {
            VirtioWinAction::List { tree, json } => {
                crate::cli::virtio_win::run_list(tree.as_deref(), json)?
            }
            VirtioWinAction::Plan { tree, image, json } => {
                crate::cli::virtio_win::run_plan(tree.as_deref(), image.as_deref(), json)?
            }
        },

        Commands::Firstboot {
            image,
            target,
            socket,
            domain,
            virtio_win,
            fail_below,
            output,
        } => crate::cli::firstboot::run(crate::cli::firstboot::FirstBootArgs {
            image,
            target,
            socket,
            domain,
            virtio_win,
            fail_below,
            output,
            verbose: cli.verbose,
        })?,

        Commands::CloudInit {
            target,
            image,
            user_data,
            meta_data,
            instance_id,
            disable_network,
            export,
        } => {
            let ds = crate::cli::plan::cloud_init::Datasource::parse(&target).ok_or_else(|| {
                anyhow::anyhow!("unknown datasource '{target}' (aws|azure|gcp|openstack|nocloud)")
            })?;
            let ud = user_data
                .as_ref()
                .map(std::fs::read_to_string)
                .transpose()?;
            let md = meta_data
                .as_ref()
                .map(std::fs::read_to_string)
                .transpose()?;
            let plan = crate::cli::plan::cloud_init::cloud_init_plan(
                crate::cli::plan::cloud_init::CloudInitOpts {
                    vm: &image.display().to_string(),
                    ds,
                    user_data: ud.as_deref(),
                    meta_data: md.as_deref(),
                    disable_network,
                    instance_id: instance_id.as_deref(),
                },
            );
            let dest = export.unwrap_or_else(|| image.with_extension("cloud-init.yaml"));
            std::fs::write(&dest, serde_yaml::to_string(&plan)?)?;
            println!(
                "wrote {} ({} ops). apply: guestkit plan apply {} --vm {} --yes",
                dest.display(),
                plan.operations.len(),
                dest.display(),
                image.display()
            );
        }

        Commands::SelinuxRelabel { image, export } => {
            crate::cli::cutover_cmd::selinux_relabel(&image, export.as_deref())?;
        }
        Commands::Sysprep {
            image,
            hostname,
            no_firstboot,
            export,
        } => {
            crate::cli::cutover_cmd::sysprep(
                &image,
                hostname.as_deref(),
                !no_firstboot,
                export.as_deref(),
            )?;
        }
        Commands::Bitlocker { action } => match action {
            BitlockerAction::Status { image } => {
                crate::cli::cutover_cmd::bitlocker_status(&image, cli.verbose)?;
            }
            BitlockerAction::Escrow {
                image,
                key_file,
                output,
                include_secret,
                export_plan,
            } => {
                crate::cli::cutover_cmd::bitlocker_escrow_cmd(
                    &image,
                    &key_file,
                    output.as_deref(),
                    include_secret,
                    export_plan.as_deref(),
                )?;
            }
        },
        Commands::Gate {
            passport,
            image,
            target,
            fail_below,
            sbom_old,
            sbom_new,
            rego,
            fail,
            output,
        } => {
            let report = crate::cli::gate::run(crate::cli::gate::GateArgs {
                passport,
                image,
                target,
                fail_below,
                sbom_old,
                sbom_new,
                rego,
                fail,
            })?;
            if output == "json" {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                crate::cli::gate::print(&report);
            }
            if fail && !report.allowed {
                anyhow::bail!("gate denied");
            }
        }
        Commands::AgentSign { action } => match action {
            AgentSignAction::Keygen { seed, public } => {
                crate::cli::agent_sign::keygen(&seed, &public)?;
            }
            AgentSignAction::Sign { manifest, output } => {
                crate::cli::agent_sign::sign(&manifest, &output)?;
            }
            AgentSignAction::Verify {
                manifest,
                signature,
            } => {
                crate::cli::agent_sign::verify(&manifest, &signature)?;
            }
        },
        Commands::VirtioInitramfs {
            image,
            dracut,
            export,
        } => {
            let plan = crate::cli::plan::linux_boot::virtio_initramfs_plan(
                &image.display().to_string(),
                dracut,
            );
            let dest = export.unwrap_or_else(|| image.with_extension("virtio-initramfs.yaml"));
            std::fs::write(&dest, serde_yaml::to_string(&plan)?)?;
            println!("wrote {}", dest.display());
        }

        Commands::ForensicDiff {
            old,
            new,
            output,
            sbom_old,
            sbom_new,
        } => {
            forensic_diff_command(
                &old,
                &new,
                &output,
                cli.verbose,
                sbom_old.as_deref(),
                sbom_new.as_deref(),
            )?;
        }

        Commands::SbomDiff {
            old,
            new,
            output,
            fail_on_drift,
        } => {
            let report = crate::cli::sbom_diff::diff_files(&old, &new)?;
            if output == "json" {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                crate::cli::sbom_diff::print_text(&report);
            }
            if fail_on_drift && report.dirty() {
                anyhow::bail!(
                    "SBOM drift: +{} -{} ~{}",
                    report.added.len(),
                    report.removed.len(),
                    report.updated.len()
                );
            }
        }

        Commands::Agent {
            channel,
            device,
            socket,
            user,
        } => {
            #[cfg(feature = "agent")]
            {
                use crate::agent::cli::{run_agent, AgentArgs, AgentChannel};
                let channel = match channel {
                    AgentChannelArg::Virtio => AgentChannel::Virtio,
                    AgentChannelArg::Vsock => AgentChannel::Vsock,
                    AgentChannelArg::Stdio => AgentChannel::Stdio,
                };
                let rt = tokio::runtime::Runtime::new()
                    .context("failed to start async runtime for agent")?;
                rt.block_on(run_agent(AgentArgs {
                    channel,
                    device,
                    socket,
                    user,
                }))?;
            }
            #[cfg(not(feature = "agent"))]
            {
                let _ = (channel, device, socket, user);
                anyhow::bail!("guestkit agent requires rebuilding with --features agent");
            }
        }

        Commands::AgentProxy {
            socket,
            listen,
            vsock_port,
        } => {
            #[cfg(feature = "agent")]
            {
                use crate::agent::cli::{run_agent_proxy, AgentProxyArgs};
                let rt = tokio::runtime::Runtime::new()
                    .context("failed to start async runtime for agent-proxy")?;
                rt.block_on(run_agent_proxy(AgentProxyArgs {
                    socket,
                    listen,
                    vsock_port,
                }))?;
            }
            #[cfg(not(feature = "agent"))]
            {
                let _ = (socket, listen, vsock_port);
                anyhow::bail!("guestkit agent-proxy requires rebuilding with --features agent");
            }
        }

        Commands::Qga {
            socket,
            execute,
            arguments,
            raw,
        } => {
            #[cfg(all(feature = "agent", unix))]
            {
                use crate::agent::cli::{run_qga, QgaArgs};
                let rt = tokio::runtime::Runtime::new()
                    .context("failed to start async runtime for qga")?;
                rt.block_on(run_qga(QgaArgs {
                    socket,
                    execute,
                    arguments,
                    raw,
                }))?;
            }
            #[cfg(not(all(feature = "agent", unix)))]
            {
                let _ = (socket, execute, arguments, raw);
                anyhow::bail!("guestkit qga requires rebuilding with --features agent on Unix");
            }
        }

        Commands::Vm { action } => crate::vm::run_cli(action, cli.verbose)?,

        Commands::AgentCall {
            socket,
            method,
            params,
        } => {
            #[cfg(feature = "agent")]
            {
                use crate::agent::cli::{run_agent_call, AgentCallArgs};
                let rt = tokio::runtime::Runtime::new()
                    .context("failed to start async runtime for agent-call")?;
                rt.block_on(run_agent_call(AgentCallArgs {
                    socket,
                    method,
                    params: Some(params),
                }))?;
            }
            #[cfg(not(feature = "agent"))]
            {
                let _ = (socket, method, params);
                anyhow::bail!("guestkit agent-call requires rebuilding with --features agent");
            }
        }

        Commands::AgentInject {
            image,
            agent_binary,
            windows,
            virtio_serial_driver,
            agent_unit,
            dry_run,
        } => {
            #[cfg(feature = "agent")]
            {
                use crate::agent::inject;
                let binary = inject::resolve_agent_binary(agent_binary.as_deref())?;
                if windows {
                    #[cfg(feature = "registry-write")]
                    {
                        inject::inject_windows_agent(
                            &image,
                            &binary,
                            virtio_serial_driver.as_deref(),
                            dry_run,
                            cli.verbose,
                        )?;
                    }
                    #[cfg(not(feature = "registry-write"))]
                    {
                        let _ = (agent_unit, dry_run, &binary, &virtio_serial_driver);
                        anyhow::bail!(
                            "guestkit agent-inject --windows requires rebuilding with --features registry-write"
                        );
                    }
                } else {
                    let unit = inject::resolve_agent_unit(agent_unit.as_deref())?;
                    inject::inject_agent_into_image(&image, &binary, &unit, dry_run, cli.verbose)?;
                }
            }
            #[cfg(not(feature = "agent"))]
            {
                let _ = (
                    image,
                    agent_binary,
                    windows,
                    virtio_serial_driver,
                    agent_unit,
                    dry_run,
                );
                anyhow::bail!("guestkit agent-inject requires rebuilding with --features agent");
            }
        }
    }

    Ok(())
}
