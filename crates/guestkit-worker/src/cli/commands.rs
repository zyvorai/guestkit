// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI command definitions

use clap::{Parser, Subcommand};
use std::path::PathBuf;

/// Guestkit Worker - Distributed job processing system
#[derive(Parser, Debug)]
#[command(name = "guestkit-worker")]
#[command(about = "Guestkit distributed worker daemon and job management CLI")]
#[command(version)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Start the worker daemon
    Daemon(DaemonArgs),

    /// Submit a job to the worker
    Submit(SubmitArgs),

    /// Get job status
    Status(StatusArgs),

    /// Get job result
    Result(ResultArgs),

    /// List all jobs
    List(ListArgs),

    /// Get worker capabilities
    Capabilities(CapabilitiesArgs),

    /// Check worker health
    Health(HealthArgs),
}

/// Daemon command arguments
#[derive(Parser, Debug)]
pub struct DaemonArgs {
    /// Worker ID (defaults to generated ULID)
    #[arg(short, long)]
    pub worker_id: Option<String>,

    /// Worker pool name
    #[arg(short, long, default_value = "default")]
    pub pool: String,

    /// Job directory to watch
    #[arg(short, long, default_value = "./jobs")]
    pub jobs_dir: PathBuf,

    /// Working directory
    #[arg(long, default_value = "/tmp/guestkit-worker")]
    pub work_dir: PathBuf,

    /// Results output directory
    #[arg(short, long, default_value = "./results")]
    pub results_dir: PathBuf,

    /// Maximum concurrent jobs
    #[arg(short, long, default_value = "4")]
    pub max_concurrent: usize,

    /// Log level
    #[arg(long, default_value = "info")]
    pub log_level: String,

    /// Enable Prometheus metrics server
    #[arg(long, default_value = "true")]
    pub metrics_enabled: bool,

    /// Metrics server bind address
    #[arg(long, default_value = "0.0.0.0:9090")]
    pub metrics_addr: String,

    /// Enable REST API server
    #[arg(long, default_value = "true")]
    pub api_enabled: bool,

    /// API server bind address
    #[arg(long, default_value = "0.0.0.0:8080")]
    pub api_addr: String,

    /// Transport mode: file, http, or redis
    #[arg(long, default_value = "file")]
    pub transport: String,

    /// Redis URL (redis transport)
    #[arg(long, env = "QUEUE_URL", default_value = "redis://127.0.0.1:6379")]
    pub redis_url: String,
}

/// Submit command arguments
#[derive(Parser, Debug)]
pub struct SubmitArgs {
    /// Job file path (JSON or YAML)
    #[arg(short, long)]
    pub file: Option<PathBuf>,

    /// Job JSON (inline)
    #[arg(short, long)]
    pub json: Option<String>,

    /// Operation to perform
    #[arg(short, long)]
    pub operation: Option<String>,

    /// Image path (for quick jobs)
    #[arg(short, long)]
    pub image: Option<PathBuf>,

    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Wait for job to complete
    #[arg(short, long)]
    pub wait: bool,

    /// Output format: json, yaml, or table
    #[arg(long, default_value = "table")]
    pub output: String,
}

/// Status command arguments
#[derive(Parser, Debug)]
pub struct StatusArgs {
    /// Job ID to check
    pub job_id: String,

    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Output format: json, yaml, or table
    #[arg(long, default_value = "table")]
    pub output: String,

    /// Watch mode (continuously update)
    #[arg(short, long)]
    pub watch: bool,
}

/// Result command arguments
#[derive(Parser, Debug)]
pub struct ResultArgs {
    /// Job ID to get result for
    pub job_id: String,

    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Output format: json or yaml
    #[arg(long, default_value = "json")]
    pub output: String,

    /// Save result to file
    #[arg(short, long)]
    pub save: Option<PathBuf>,
}

/// List command arguments
#[derive(Parser, Debug)]
pub struct ListArgs {
    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Output format: json, yaml, or table
    #[arg(long, default_value = "table")]
    pub output: String,

    /// Filter by status
    #[arg(short, long)]
    pub status: Option<String>,

    /// Filter by operation
    #[arg(short, long)]
    pub operation: Option<String>,
}

/// Capabilities command arguments
#[derive(Parser, Debug)]
pub struct CapabilitiesArgs {
    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Output format: json, yaml, or table
    #[arg(long, default_value = "table")]
    pub output: String,
}

/// Health command arguments
#[derive(Parser, Debug)]
pub struct HealthArgs {
    /// API server URL
    #[arg(long, default_value = "http://localhost:8080")]
    pub api_url: String,

    /// Output format: json, yaml, or table
    #[arg(long, default_value = "table")]
    pub output: String,
}
