// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Status command handler

use anyhow::Result;
use prettytable::{Table, row};
use super::commands::StatusArgs;
use super::client::WorkerClient;

pub async fn run_status(args: StatusArgs) -> Result<()> {
    let client = WorkerClient::new(args.api_url);

    if args.watch {
        // Watch mode - continuously update
        loop {
            // Clear screen
            print!("\x1B[2J\x1B[1;1H");

            display_status(&client, &args.job_id, &args.output).await?;

            // Check if job is terminal
            let status_response = client.get_job_status(&args.job_id).await?;
            if matches!(status_response.status.as_str(), "completed" | "failed" | "cancelled") {
                println!("\nJob reached terminal state: {}", status_response.status);
                break;
            }

            // Wait before next update
            tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;
        }
    } else {
        // Single status check
        display_status(&client, &args.job_id, &args.output).await?;
    }

    Ok(())
}

async fn display_status(client: &WorkerClient, job_id: &str, output: &str) -> Result<()> {
    let response = client.get_job_status(job_id).await?;

    match output {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&response)?);
        },
        "yaml" => {
            println!("{}", serde_yaml::to_string(&response)?);
        },
        "table" | _ => {
            let mut table = Table::new();
            table.add_row(row!["Field", "Value"]);
            table.add_row(row!["Job ID", response.job_id]);
            table.add_row(row!["Status", response.status]);
            table.add_row(row!["Operation", response.operation]);
            table.add_row(row!["Submitted At", response.submitted_at]);

            if let Some(started_at) = response.started_at {
                table.add_row(row!["Started At", started_at]);
            }

            if let Some(completed_at) = response.completed_at {
                table.add_row(row!["Completed At", completed_at]);
            }

            if let Some(worker_id) = response.worker_id {
                table.add_row(row!["Worker ID", worker_id]);
            }

            if let Some(error) = response.error {
                table.add_row(row!["Error", error]);
            }

            table.printstd();
        }
    }

    Ok(())
}
