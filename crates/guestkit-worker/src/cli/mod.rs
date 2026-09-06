// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI module for guestkit-worker

pub mod client;
pub mod commands;
pub mod daemon;
pub mod submit;
pub mod status;
pub mod result;
pub mod list;
pub mod capabilities;
pub mod health;

use anyhow::Result;
use clap::Parser;
use commands::{Cli, Commands};

/// Run the CLI
pub async fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Daemon(args) => daemon::run_daemon(args).await,
        Commands::Submit(args) => submit::run_submit(args).await,
        Commands::Status(args) => status::run_status(args).await,
        Commands::Result(args) => result::run_result(args).await,
        Commands::List(args) => list::run_list(args).await,
        Commands::Capabilities(args) => capabilities::run_capabilities(args).await,
        Commands::Health(args) => health::run_health(args).await,
    }
}
