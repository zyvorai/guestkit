// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Echo handler - simple test handler that echoes back payload

use async_trait::async_trait;
use guestkit_job_spec::Payload;
use crate::error::WorkerResult;
use crate::handler::{OperationHandler, HandlerContext, HandlerResult};

/// Echo handler - useful for testing
pub struct EchoHandler;

impl EchoHandler {
    /// Create a new echo handler
    pub fn new() -> Self {
        Self
    }
}

impl Default for EchoHandler {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl OperationHandler for EchoHandler {
    fn name(&self) -> &str {
        "echo-handler"
    }

    fn operations(&self) -> Vec<String> {
        vec!["system.echo".to_string(), "test.echo".to_string()]
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        context.report_progress("starting", Some(0), "Echo handler starting").await?;

        log::info!("Echo handler executing for job {}", context.job_id);
        log::info!("Payload type: {}", payload.payload_type);
        log::info!("Payload data: {}", payload.data);

        context.report_progress("processing", Some(50), "Processing payload").await?;

        // Simulate some work
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

        context.report_progress("completing", Some(100), "Echo complete").await?;

        Ok(HandlerResult::new()
            .with_data(serde_json::json!({
                "echo": payload.data,
                "message": "Echo handler executed successfully"
            })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::progress::ProgressTracker;
    use std::sync::Arc;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_echo_handler() {
        let temp_dir = TempDir::new().unwrap();
        let handler = EchoHandler::new();

        assert_eq!(handler.name(), "echo-handler");
        assert!(handler.operations().contains(&"system.echo".to_string()));

        let (progress, _rx) = ProgressTracker::new("test-job");
        let context = HandlerContext::new(
            "test-job",
            "test-worker",
            Arc::new(progress),
            temp_dir.path(),
        );

        let payload = Payload {
            payload_type: "test.echo.v1".to_string(),
            data: serde_json::json!({"message": "hello world"}),
        };

        let result = handler.execute(context, payload).await.unwrap();

        assert!(result.data.get("echo").is_some());
        assert_eq!(
            result.data.get("echo").unwrap().get("message").unwrap(),
            "hello world"
        );
    }
}
