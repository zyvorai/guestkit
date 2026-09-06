// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Result command handler

use anyhow::{Result, Context};
use std::fs;
use super::commands::ResultArgs;
use super::client::WorkerClient;

pub async fn run_result(args: ResultArgs) -> Result<()> {
    let client = WorkerClient::new(args.api_url);

    // Fetch result
    let result = client.get_job_result(&args.job_id).await?;

    // Format output
    let output_str = match args.output.as_str() {
        "yaml" => serde_yaml::to_string(&result)?,
        "json" | _ => serde_json::to_string_pretty(&result)?,
    };

    // Save to file or print to stdout
    if let Some(save_path) = args.save {
        fs::write(&save_path, &output_str)
            .with_context(|| format!("Failed to write result to {}", save_path.display()))?;

        println!("Result saved to: {}", save_path.display());
    } else {
        println!("{}", output_str);
    }

    Ok(())
}
