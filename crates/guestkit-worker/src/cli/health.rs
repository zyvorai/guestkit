// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Health command handler

use anyhow::Result;
use prettytable::{Table, row};
use super::commands::HealthArgs;
use super::client::WorkerClient;

pub async fn run_health(args: HealthArgs) -> Result<()> {
    let client = WorkerClient::new(args.api_url);

    // Fetch health status
    let response = client.health_check().await?;

    match args.output.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        },
        "yaml" => {
            println!("{}", serde_yaml::to_string(&response)?);
        },
        "table" | _ => {
            let mut table = Table::new();
            table.add_row(row!["Field", "Value"]);
            table.add_row(row!["Status", response.status]);
            table.add_row(row!["Uptime (seconds)", response.uptime_seconds]);

            table.printstd();

            // Human-readable uptime
            let hours = response.uptime_seconds / 3600;
            let minutes = (response.uptime_seconds % 3600) / 60;
            let seconds = response.uptime_seconds % 60;

            println!("\nUptime: {}h {}m {}s", hours, minutes, seconds);

            // Status indicator
            if response.status == "healthy" {
                println!("✓ Worker is healthy");
            } else {
                println!("✗ Worker is unhealthy");
            }
        }
    }

    Ok(())
}
