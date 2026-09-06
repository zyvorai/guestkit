// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Operation handler trait and registry

use async_trait::async_trait;
use guestkit_job_spec::{JobDocument, Payload};
use std::collections::HashMap;
use std::sync::Arc;
use crate::error::{WorkerError, WorkerResult};
use crate::progress::ProgressTracker;
use crate::metrics::MetricsRegistry;

/// Context provided to operation handlers
#[derive(Debug, Clone)]
pub struct HandlerContext {
    /// Job ID
    pub job_id: String,

    /// Worker ID
    pub worker_id: String,

    /// Progress tracker
    pub progress: Arc<ProgressTracker>,

    /// Working directory
    pub work_dir: std::path::PathBuf,

    /// Metrics registry (optional)
    pub metrics: Option<Arc<MetricsRegistry>>,
}

impl HandlerContext {
    /// Create a new handler context
    pub fn new(
        job_id: impl Into<String>,
        worker_id: impl Into<String>,
        progress: Arc<ProgressTracker>,
        work_dir: impl Into<std::path::PathBuf>,
    ) -> Self {
        Self {
            job_id: job_id.into(),
            worker_id: worker_id.into(),
            progress,
            work_dir: work_dir.into(),
            metrics: None,
        }
    }

    /// Attach metrics registry
    pub fn with_metrics(mut self, metrics: Arc<MetricsRegistry>) -> Self {
        self.metrics = Some(metrics);
        self
    }

    /// Report progress
    pub async fn report_progress(
        &self,
        phase: impl Into<String>,
        progress_percent: Option<u8>,
        message: impl Into<String>,
    ) -> WorkerResult<()> {
        self.progress
            .report(phase.into(), progress_percent, message.into())
            .await
    }

    /// Record checksum verification metric
    pub fn record_checksum_verification(&self, status: &str) {
        if let Some(ref metrics) = self.metrics {
            metrics.record_checksum_verification(status);
        }
    }
}

/// Result of operation execution
#[derive(Debug, Clone)]
pub struct HandlerResult {
    /// Primary output file (if any)
    pub output_file: Option<String>,

    /// Additional artifacts
    pub artifacts: Vec<String>,

    /// Custom result data
    pub data: serde_json::Value,
}

impl HandlerResult {
    /// Create a new handler result
    pub fn new() -> Self {
        Self {
            output_file: None,
            artifacts: Vec::new(),
            data: serde_json::Value::Null,
        }
    }

    /// Set output file
    pub fn with_output(mut self, path: impl Into<String>) -> Self {
        self.output_file = Some(path.into());
        self
    }

    /// Add artifact
    pub fn with_artifact(mut self, path: impl Into<String>) -> Self {
        self.artifacts.push(path.into());
        self
    }

    /// Set result data
    pub fn with_data(mut self, data: serde_json::Value) -> Self {
        self.data = data;
        self
    }
}

impl Default for HandlerResult {
    fn default() -> Self {
        Self::new()
    }
}

/// Operation handler trait
#[async_trait]
pub trait OperationHandler: Send + Sync {
    /// Handler name (for logging)
    fn name(&self) -> &str;

    /// Operations this handler supports
    fn operations(&self) -> Vec<String>;

    /// Validate job can be executed
    async fn validate(&self, _payload: &Payload) -> WorkerResult<()> {
        Ok(())
    }

    /// Execute the operation
    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult>;

    /// Cleanup after execution (success or failure)
    async fn cleanup(&self, _context: &HandlerContext) -> WorkerResult<()> {
        Ok(())
    }
}

/// Handler registry - Maps operations to handlers
pub struct HandlerRegistry {
    handlers: HashMap<String, Arc<dyn OperationHandler>>,
}

impl HandlerRegistry {
    /// Create a new registry
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
        }
    }

    /// Register a handler
    pub fn register(&mut self, handler: Arc<dyn OperationHandler>) {
        for operation in handler.operations() {
            log::info!(
                "Registering handler '{}' for operation '{}'",
                handler.name(),
                operation
            );
            self.handlers.insert(operation, handler.clone());
        }
    }

    /// Get handler for operation
    pub fn get(&self, operation: &str) -> Option<&Arc<dyn OperationHandler>> {
        self.handlers.get(operation)
    }

    /// Check if operation is supported
    pub fn supports(&self, operation: &str) -> bool {
        self.handlers.contains_key(operation)
    }

    /// List all supported operations
    pub fn operations(&self) -> Vec<String> {
        self.handlers.keys().cloned().collect()
    }

    /// Number of registered handlers
    pub fn len(&self) -> usize {
        self.handlers.len()
    }

    /// Check if registry is empty
    pub fn is_empty(&self) -> bool {
        self.handlers.is_empty()
    }
}

impl Default for HandlerRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockHandler {
        name: String,
        operations: Vec<String>,
    }

    impl MockHandler {
        fn new(name: &str, operations: Vec<&str>) -> Self {
            Self {
                name: name.to_string(),
                operations: operations.iter().map(|s| s.to_string()).collect(),
            }
        }
    }

    #[async_trait]
    impl OperationHandler for MockHandler {
        fn name(&self) -> &str {
            &self.name
        }

        fn operations(&self) -> Vec<String> {
            self.operations.clone()
        }

        async fn execute(
            &self,
            _context: HandlerContext,
            _payload: Payload,
        ) -> WorkerResult<HandlerResult> {
            Ok(HandlerResult::new())
        }
    }

    #[test]
    fn test_registry() {
        let mut registry = HandlerRegistry::new();

        let handler = Arc::new(MockHandler::new(
            "test-handler",
            vec!["guestkit.inspect", "guestkit.profile"],
        ));

        registry.register(handler);

        assert!(registry.supports("guestkit.inspect"));
        assert!(registry.supports("guestkit.profile"));
        assert!(!registry.supports("guestkit.fix"));
        assert_eq!(registry.len(), 2);
    }
}
