// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit doctor handler — bootability scoring

use async_trait::async_trait;
use guestkit::assurance::run_doctor;
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{HandlerContext, HandlerResult, OperationHandler};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct DoctorPayload {
    image: ImageSpec,
    #[serde(default = "default_target")]
    target: String,
    #[serde(default)]
    explain: bool,
}

fn default_target() -> String {
    "kubevirt".to_string()
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

pub struct DoctorHandler;

#[async_trait]
impl OperationHandler for DoctorHandler {
    fn name(&self) -> &str {
        "guestkit-doctor"
    }

    fn operations(&self) -> Vec<String> {
        vec![guestkit_job_spec::operations::GUESTKIT_DOCTOR.to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let p: DoctorPayload = serde_json::from_value(payload.data.clone())
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
        let p: DoctorPayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context
            .report_progress("doctor", Some(10), "Analyzing bootability")
            .await?;

        let image = PathBuf::from(&p.image.path);
        let target = p.target.clone();
        let explain = p.explain;

        let result = tokio::task::spawn_blocking(move || {
            run_doctor(&image, &target, explain, false)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {e}")))?
        .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context.report_progress("complete", Some(100), "Done").await?;

        let data = serde_json::to_value(&result)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        Ok(HandlerResult::new().with_data(data))
    }
}
