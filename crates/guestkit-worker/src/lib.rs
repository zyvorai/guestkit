// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit Worker - Distributed VM Operations Executor
//!
//! This crate provides the worker implementation for executing VM operations
//! jobs defined by the guestkit-job-spec protocol.

pub mod error;
pub mod worker;
pub mod executor;
pub mod handler;
pub mod transport;
pub mod state;
pub mod progress;
pub mod result;
pub mod handlers;
pub mod metrics;
pub mod metrics_server;
pub mod api;
pub mod cli;

// Re-exports
pub use error::{WorkerError, WorkerResult};
pub use worker::{Worker, WorkerConfig};
pub use executor::JobExecutor;
pub use handler::{OperationHandler, HandlerRegistry, HandlerContext};
pub use transport::{JobTransport, FileTransport};
pub use state::{JobState, JobStateMachine};
pub use progress::ProgressTracker;

/// Worker capabilities
pub mod capabilities {
    use serde::{Deserialize, Serialize};

    /// Worker capabilities set
    #[derive(Debug, Clone, Serialize, Deserialize, Default)]
    pub struct Capabilities {
        /// Supported operations
        pub operations: Vec<String>,

        /// Supported features
        pub features: Vec<String>,

        /// Supported disk formats
        pub disk_formats: Vec<String>,

        /// Maximum concurrent jobs
        pub max_concurrent_jobs: usize,

        /// Maximum disk size (GB)
        pub max_disk_size_gb: u64,
    }

    impl Capabilities {
        /// Create a new capabilities set
        pub fn new() -> Self {
            Self::default()
        }

        /// Add an operation
        pub fn with_operation(mut self, operation: impl Into<String>) -> Self {
            self.operations.push(operation.into());
            self
        }

        /// Add a feature
        pub fn with_feature(mut self, feature: impl Into<String>) -> Self {
            self.features.push(feature.into());
            self
        }

        /// Add a disk format
        pub fn with_disk_format(mut self, format: impl Into<String>) -> Self {
            self.disk_formats.push(format.into());
            self
        }

        /// Check if operation is supported
        pub fn supports_operation(&self, operation: &str) -> bool {
            self.operations.iter().any(|op| op == operation)
        }

        /// Check if feature is available
        pub fn has_feature(&self, feature: &str) -> bool {
            self.features.iter().any(|f| f == feature)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_capabilities() {
        let caps = capabilities::Capabilities::new()
            .with_operation("guestkit.inspect")
            .with_feature("lvm")
            .with_disk_format("qcow2");

        assert!(caps.supports_operation("guestkit.inspect"));
        assert!(caps.has_feature("lvm"));
        assert!(!caps.supports_operation("guestkit.fix"));
    }
}
