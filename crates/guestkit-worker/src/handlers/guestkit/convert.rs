// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit convert handler — qemu-img format conversion

use async_trait::async_trait;
use guestkit_job_spec::Payload;
use guestkit_job_spec::operations::GUESTKIT_CONVERT;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::process::Command;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{HandlerContext, HandlerResult, OperationHandler};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ConvertPayload {
    image: ImageSpec,
    target_format: String,
    #[serde(default)]
    compression: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ImageSpec {
    path: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "qcow2".to_string()
}

pub struct ConvertHandler;

#[async_trait]
impl OperationHandler for ConvertHandler {
    fn name(&self) -> &str {
        "guestkit-convert"
    }

    fn operations(&self) -> Vec<String> {
        vec![GUESTKIT_CONVERT.to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let p: ConvertPayload = serde_json::from_value(payload.data.clone())
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;
        if p.image.path.is_empty() {
            return Err(WorkerError::ExecutionError("image.path is required".into()));
        }
        Ok(())
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        let p: ConvertPayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context
            .report_progress("convert", Some(10), "Starting conversion")
            .await?;

        let src = PathBuf::from(&p.image.path);
        if !src.exists() {
            return Err(WorkerError::ExecutionError(format!(
                "image not found: {}",
                src.display()
            )));
        }

        let target_fmt = p.target_format.to_lowercase();
        let out_path = src.with_extension(&target_fmt);
        let mut cmd = Command::new("qemu-img");
        cmd.arg("convert")
            .arg("-p")
            .arg("-f")
            .arg(&p.image.format)
            .arg("-O")
            .arg(&target_fmt);
        if p.compression && target_fmt == "qcow2" {
            cmd.arg("-c");
        }
        cmd.arg(&src).arg(&out_path);

        context
            .report_progress("convert", Some(40), "Running qemu-img convert")
            .await?;

        let output = cmd
            .output()
            .await
            .map_err(|e| WorkerError::ExecutionError(format!("qemu-img failed: {e}")))?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(WorkerError::ExecutionError(format!(
                "qemu-img convert error: {stderr}"
            )));
        }

        context
            .report_progress("convert", Some(90), "Conversion complete")
            .await?;

        let meta = tokio::fs::metadata(&out_path)
            .await
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        Ok(HandlerResult::new()
            .with_output(out_path.display().to_string())
            .with_data(serde_json::json!({
                "source": src.display().to_string(),
                "output": out_path.display().to_string(),
                "target_format": target_fmt,
                "size_bytes": meta.len(),
            })))
    }
}
