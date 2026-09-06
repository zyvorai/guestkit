// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! List command handler

use anyhow::Result;
use prettytable::{Table, row};
use super::commands::ListArgs;
use super::client::WorkerClient;

pub async fn run_list(args: ListArgs) -> Result<()> {
    let client = WorkerClient::new(args.api_url);

    // Fetch job list
    let response = client.list_jobs().await?;

    // Apply filters
    let filtered_jobs: Vec<_> = response.jobs.into_iter()
        .filter(|job| {
            if let Some(ref status_filter) = args.status {
                if job.status != *status_filter {
                    return false;
                }
            }
            if let Some(ref op_filter) = args.operation {
                if job.operation != *op_filter {
                    return false;
                }
            }
            true
        })
        .collect();

    match args.output.as_str() {
        "json" => {
            println!("{}", serde_json::to_string_pretty(&filtered_jobs)?);
        },
        "yaml" => {
            println!("{}", serde_yaml::to_string(&filtered_jobs)?);
        },
        "table" | _ => {
            let mut table = Table::new();
            table.add_row(row![
                "Job ID",
                "Status",
                "Operation",
                "Worker ID",
                "Submitted At"
            ]);

            for job in &filtered_jobs {
                table.add_row(row![
                    job.job_id,
                    job.status,
                    job.operation,
                    job.worker_id.as_deref().unwrap_or("-"),
                    job.submitted_at,
                ]);
            }

            table.printstd();

            println!("\nTotal: {} jobs", filtered_jobs.len());
        }
    }

    Ok(())
}
