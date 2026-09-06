// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Capabilities command handler

use anyhow::Result;
use prettytable::{Table, row, cell};
use super::commands::CapabilitiesArgs;
use super::client::WorkerClient;

pub async fn run_capabilities(args: CapabilitiesArgs) -> Result<()> {
    let client = WorkerClient::new(args.api_url);

    // Fetch capabilities
    let response = client.get_capabilities().await?;

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
            table.add_row(row!["Worker ID", response.worker_id]);

            if let Some(pool) = response.pool {
                table.add_row(row!["Pool", pool]);
            }

            table.printstd();

            // Operations
            println!("\nOperations ({}):", response.operations.len());
            for op in &response.operations {
                println!("  • {}", op);
            }

            // Features
            println!("\nFeatures ({}):", response.features.len());
            for feature in &response.features {
                println!("  • {}", feature);
            }

            // Disk formats
            println!("\nDisk Formats ({}):", response.disk_formats.len());
            for format in &response.disk_formats {
                println!("  • {}", format);
            }
        }
    }

    Ok(())
}
