// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit repair handler

use async_trait::async_trait;
use guestkit::assurance::{run_repair_plan, RepairOptions};
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{HandlerContext, HandlerResult, OperationHandler};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct RepairPayload {
    image: ImageSpec,
    #[serde(default = "default_true")]
    dry_run: bool,
    #[serde(default = "default_fix")]
    fix: String,
    #[serde(default)]
    inject_qga: bool,
    #[serde(default)]
    inject_zyvor_agent: bool,
    #[serde(default = "default_true")]
    enable_systemd: bool,
    #[serde(default)]
    fix_cloud_init_network: bool,
    #[serde(default)]
    validate_fstab: bool,
}

fn default_true() -> bool {
    true
}

fn default_fix() -> String {
    "boot".to_string()
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

pub struct RepairHandler;

#[async_trait]
impl OperationHandler for RepairHandler {
    fn name(&self) -> &str {
        "guestkit-repair"
    }

    fn operations(&self) -> Vec<String> {
        vec![guestkit_job_spec::operations::GUESTKIT_REPAIR.to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let p: RepairPayload = serde_json::from_value(payload.data.clone())
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;
        if p.image.path.is_empty() {
            return Err(WorkerError::ExecutionError("image.path is required".into()));
        }
        if p.fix != "boot" {
            return Err(WorkerError::ExecutionError(format!(
                "unsupported fix type: {}",
                p.fix
            )));
        }
        Ok(())
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        let p: RepairPayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context
            .report_progress("repair", Some(10), "Generating repair plan")
            .await?;

        let image = PathBuf::from(p.image.path);
        let options = RepairOptions {
            dry_run: p.dry_run,
            verbose: false,
            inject_agent: p.inject_zyvor_agent,
            inject_qga: p.inject_qga,
            enable_systemd: p.enable_systemd,
            fix_cloud_init_network: p.fix_cloud_init_network,
            validate_fstab: p.validate_fstab,
            ..Default::default()
        };

        let result = tokio::task::spawn_blocking(move || run_repair_plan(&image, &options))
            .await
            .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {e}")))?
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context.report_progress("complete", Some(100), "Done").await?;

        let data = serde_json::to_value(&result)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        Ok(HandlerResult::new().with_data(data))
    }
}
