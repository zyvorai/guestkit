// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Submit command handler

use anyhow::{Result, Context, bail};
use guestkit_job_spec::{JobDocument, JobBuilder};
use serde_json::json;
use std::fs;
use std::path::PathBuf;
use prettytable::{Table, row};
use super::commands::SubmitArgs;
use super::client::WorkerClient;

pub async fn run_submit(args: SubmitArgs) -> Result<()> {
    // Create job document from args
    let job = if let Some(file_path) = args.file {
        // Load from file
        load_job_from_file(&file_path)?
    } else if let Some(json_str) = args.json {
        // Parse inline JSON
        serde_json::from_str(&json_str)
            .context("Failed to parse inline JSON")?
    } else if let Some(operation) = args.operation {
        // Create quick job from operation
        create_quick_job(&operation, args.image)?
    } else {
        bail!("Must provide --file, --json, or --operation");
    };

    // Validate job
    if job.operation.is_empty() {
        bail!("Job operation cannot be empty");
    }

    // Create API client
    let client = WorkerClient::new(args.api_url);

    // Submit job
    println!("Submitting job...");
    let response = client.submit_job(job).await?;

    // Output response
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
            table.add_row(row!["Job ID", response.job_id]);
            table.add_row(row!["Status", response.status]);
            table.add_row(row!["Message", response.message]);
            table.printstd();
        }
    }

    // Wait for completion if requested
    if args.wait {
        println!("\nWaiting for job to complete...");
        wait_for_completion(&client, &response.job_id).await?;
    }

    Ok(())
}

fn load_job_from_file(path: &PathBuf) -> Result<JobDocument> {
    let content = fs::read_to_string(path)
        .with_context(|| format!("Failed to read job file: {}", path.display()))?;

    // Try JSON first, then YAML
    if let Ok(job) = serde_json::from_str::<JobDocument>(&content) {
        return Ok(job);
    }

    serde_yaml::from_str(&content)
        .context("Failed to parse job file as JSON or YAML")
}

fn create_quick_job(operation: &str, image: Option<PathBuf>) -> Result<JobDocument> {
    let data = if let Some(img_path) = image {
        json!({
            "image": img_path.to_string_lossy().to_string()
        })
    } else {
        json!({})
    };

    let job = JobBuilder::new()
        .generate_job_id()
        .operation(operation)
        .payload(operation, data)
        .build()?;

    Ok(job)
}

async fn wait_for_completion(client: &WorkerClient, job_id: &str) -> Result<()> {
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let status = client.get_job_status(job_id).await?;

        match status.status.as_str() {
            "completed" => {
                println!("✓ Job completed successfully");

                // Fetch and display result
                match client.get_job_result(job_id).await {
                    Ok(result) => {
                        println!("\nResult:");
                        println!("{}", serde_json::to_string_pretty(&result)?);
                    },
                    Err(e) => {
                        println!("Warning: Could not fetch result: {}", e);
                    }
                }

                break;
            },
            "failed" => {
                println!("✗ Job failed");
                if let Some(error) = status.error {
                    println!("Error: {}", error);
                }
                bail!("Job execution failed");
            },
            "cancelled" => {
                println!("○ Job cancelled");
                bail!("Job was cancelled");
            },
            other => {
                println!("Status: {}", other);
            }
        }
    }

    Ok(())
}
