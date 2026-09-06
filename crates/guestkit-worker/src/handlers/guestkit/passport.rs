// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit Cutover Passport handler

use async_trait::async_trait;
use guestkit::assurance::{emit_passport, PassportEmitOptions};
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

use crate::error::{WorkerError, WorkerResult};
use crate::handler::{HandlerContext, HandlerResult, OperationHandler};

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PassportPayload {
    image: ImageSpec,
    #[serde(default = "default_target")]
    target: String,
    #[serde(default)]
    content_hash: bool,
    #[serde(default)]
    live_url: Option<String>,
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

pub struct PassportHandler;

#[async_trait]
impl OperationHandler for PassportHandler {
    fn name(&self) -> &str {
        "guestkit-passport"
    }

    fn operations(&self) -> Vec<String> {
        vec![guestkit_job_spec::operations::GUESTKIT_PASSPORT.to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let p: PassportPayload = serde_json::from_value(payload.data.clone())
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
        let p: PassportPayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context
            .report_progress("passport", Some(10), "Emitting Cutover Passport")
            .await?;

        let image = PathBuf::from(&p.image.path);
        let target = p.target.clone();
        let content_hash = p.content_hash;
        let live_url = p.live_url.clone();

        let (passport, plan) = tokio::task::spawn_blocking(move || {
            emit_passport(
                &image,
                &target,
                &PassportEmitOptions {
                    verbose: false,
                    content_hash,
                    virtio_win_dir: None,
                    live_url,
                    sign_key: None,
                    issuer: None,
                    expires_hours: None,
                },
            )
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {e}")))?
        .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;

        context.report_progress("complete", Some(100), "Done").await?;

        let data = serde_json::json!({
            "passport": passport,
            "fix_plan": plan,
        });
        Ok(HandlerResult::new().with_data(data))
    }
}
