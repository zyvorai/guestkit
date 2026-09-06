// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fluent builder for creating job documents

use crate::error::{JobError, JobResult};
use crate::types::*;
use crate::PROTOCOL_VERSION;
use chrono::Utc;
use std::collections::HashMap;

/// Fluent builder for creating job documents
#[derive(Debug, Clone, Default)]
pub struct JobBuilder {
    job_id: Option<String>,
    operation: Option<String>,
    payload_type: Option<String>,
    payload_data: Option<serde_json::Value>,
    metadata: JobMetadata,
    execution: ExecutionPolicy,
    constraints: Constraints,
    routing: Routing,
    observability: Observability,
    audit: Audit,
}

impl JobBuilder {
    /// Create a new job builder
    pub fn new() -> Self {
        Self {
            execution: ExecutionPolicy::default(),
            ..Default::default()
        }
    }

    /// Set job ID
    pub fn job_id(mut self, id: impl Into<String>) -> Self {
        self.job_id = Some(id.into());
        self
    }

    /// Generate a new ULID job ID
    pub fn generate_job_id(mut self) -> Self {
        self.job_id = Some(format!("job-{}", ulid::Ulid::new()));
        self
    }

    /// Set operation
    pub fn operation(mut self, operation: impl Into<String>) -> Self {
        self.operation = Some(operation.into());
        self
    }

    /// Set payload
    pub fn payload(mut self, payload_type: impl Into<String>, data: serde_json::Value) -> Self {
        self.payload_type = Some(payload_type.into());
        self.payload_data = Some(data);
        self
    }

    /// Set metadata name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.metadata.name = Some(name.into());
        self
    }

    /// Set metadata namespace
    pub fn namespace(mut self, namespace: impl Into<String>) -> Self {
        self.metadata.namespace = Some(namespace.into());
        self
    }

    /// Add a label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .labels
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Add an annotation
    pub fn annotation(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata
            .annotations
            .get_or_insert_with(HashMap::new)
            .insert(key.into(), value.into());
        self
    }

    /// Set idempotency key
    pub fn idempotency_key(mut self, key: impl Into<String>) -> Self {
        self.execution.idempotency_key = Some(key.into());
        self
    }

    /// Set priority (1-10)
    pub fn priority(mut self, priority: u8) -> Self {
        self.execution.priority = priority.clamp(1, 10);
        self
    }

    /// Set timeout in seconds
    pub fn timeout_seconds(mut self, seconds: u64) -> Self {
        self.execution.timeout_seconds = seconds;
        self
    }

    /// Set max attempts
    pub fn max_attempts(mut self, attempts: u32) -> Self {
        self.execution.max_attempts = attempts;
        self
    }

    /// Add required capability
    pub fn require_capability(mut self, capability: impl Into<String>) -> Self {
        self.constraints
            .required_capabilities
            .get_or_insert_with(Vec::new)
            .push(capability.into());
        self
    }

    /// Add required feature
    pub fn require_feature(mut self, feature: impl Into<String>) -> Self {
        self.constraints
            .required_features
            .get_or_insert_with(Vec::new)
            .push(feature.into());
        self
    }

    /// Set worker pool
    pub fn worker_pool(mut self, pool: impl Into<String>) -> Self {
        self.routing.worker_pool = Some(pool.into());
        self
    }

    /// Set trace ID
    pub fn trace_id(mut self, trace_id: impl Into<String>) -> Self {
        self.observability.trace_id = Some(trace_id.into());
        self
    }

    /// Set correlation ID
    pub fn correlation_id(mut self, correlation_id: impl Into<String>) -> Self {
        self.observability.correlation_id = Some(correlation_id.into());
        self
    }

    /// Set submitter
    pub fn submitted_by(mut self, submitter: impl Into<String>) -> Self {
        self.audit.submitted_by = Some(submitter.into());
        self
    }

    /// Build the job document
    pub fn build(self) -> JobResult<JobDocument> {
        let job_id = self
            .job_id
            .ok_or_else(|| JobError::MissingField("job_id".to_string()))?;

        let operation = self
            .operation
            .ok_or_else(|| JobError::MissingField("operation".to_string()))?;

        let payload_type = self
            .payload_type
            .ok_or_else(|| JobError::MissingField("payload.type".to_string()))?;

        let payload_data = self
            .payload_data
            .ok_or_else(|| JobError::MissingField("payload.data".to_string()))?;

        let job = JobDocument {
            schema: Some("https://guestkit.dev/schemas/job-v1.json".to_string()),
            version: PROTOCOL_VERSION.to_string(),
            job_id,
            created_at: Utc::now(),
            kind: "VMOperation".to_string(),
            operation,
            metadata: if self.metadata != JobMetadata::default() {
                Some(self.metadata)
            } else {
                None
            },
            execution: Some(self.execution),
            constraints: if self.constraints != Constraints::default() {
                Some(self.constraints)
            } else {
                None
            },
            routing: if self.routing != Routing::default() {
                Some(self.routing)
            } else {
                None
            },
            payload: Payload {
                payload_type,
                data: payload_data,
            },
            observability: if self.observability != Observability::default() {
                Some(self.observability)
            } else {
                None
            },
            audit: if self.audit != Audit::default() {
                Some(self.audit)
            } else {
                None
            },
        };

        // Validate the built job
        crate::validation::JobValidator::validate(&job)?;

        Ok(job)
    }
}

/// Helper to create a guestkit inspect job
pub fn inspect_job(image_path: impl Into<String>) -> JobBuilder {
    let payload = serde_json::json!({
        "image": {
            "path": image_path.into(),
            "format": "qcow2",
            "read_only": true
        },
        "options": {
            "deep_scan": false,
            "include_packages": true,
            "include_services": true,
            "include_network": true,
            "include_security": true
        }
    });

    JobBuilder::new()
        .generate_job_id()
        .operation("guestkit.inspect")
        .payload("guestkit.inspect.v1", payload)
        .require_capability("guestkit.inspect")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_minimal() {
        let job = JobBuilder::new()
            .job_id("job-test-123")
            .operation("guestkit.inspect")
            .payload("guestkit.inspect.v1", serde_json::json!({}))
            .build()
            .unwrap();

        assert_eq!(job.job_id, "job-test-123");
        assert_eq!(job.operation, "guestkit.inspect");
        assert_eq!(job.version, "1.0");
        assert_eq!(job.kind, "VMOperation");
    }

    #[test]
    fn test_builder_with_metadata() {
        let job = JobBuilder::new()
            .job_id("job-test-456")
            .operation("guestkit.inspect")
            .payload("guestkit.inspect.v1", serde_json::json!({}))
            .name("test-job")
            .namespace("production")
            .label("env", "prod")
            .label("team", "platform")
            .annotation("ticket", "INC-123")
            .build()
            .unwrap();

        let metadata = job.metadata.unwrap();
        assert_eq!(metadata.name.unwrap(), "test-job");
        assert_eq!(metadata.namespace.unwrap(), "production");
        assert_eq!(metadata.labels.unwrap().get("env").unwrap(), "prod");
        assert_eq!(
            metadata.annotations.unwrap().get("ticket").unwrap(),
            "INC-123"
        );
    }

    #[test]
    fn test_builder_with_constraints() {
        let job = JobBuilder::new()
            .job_id("job-test-789")
            .operation("guestkit.fix")
            .payload("guestkit.fix.v1", serde_json::json!({}))
            .require_capability("guestkit.fix")
            .require_capability("disk.qcow2")
            .require_feature("lvm")
            .require_feature("selinux")
            .build()
            .unwrap();

        let constraints = job.constraints.unwrap();
        assert_eq!(constraints.required_capabilities.unwrap().len(), 2);
        assert_eq!(constraints.required_features.unwrap().len(), 2);
    }

    #[test]
    fn test_inspect_job_helper() {
        let job = inspect_job("/vms/test.qcow2").build().unwrap();

        assert_eq!(job.operation, "guestkit.inspect");
        assert_eq!(job.payload.payload_type, "guestkit.inspect.v1");
        assert!(job.job_id.starts_with("job-"));
    }

    #[test]
    fn test_builder_missing_operation() {
        let result = JobBuilder::new()
            .job_id("job-test")
            .payload("guestkit.inspect.v1", serde_json::json!({}))
            .build();

        assert!(matches!(result, Err(JobError::MissingField(_))));
    }
}
