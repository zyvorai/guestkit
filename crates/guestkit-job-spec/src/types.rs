// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Core type definitions for the VM Operations Job Protocol v1

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Top-level job document (envelope)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct JobDocument {
    /// Protocol version (always "1.0" for v1)
    #[serde(rename = "$schema")]
    pub schema: Option<String>,

    /// Protocol version
    pub version: String,

    /// Job specification
    pub job_id: String,

    /// Timestamp when job was created
    pub created_at: DateTime<Utc>,

    /// Job kind (always "VMOperation" for v1)
    pub kind: String,

    /// Namespaced operation name
    pub operation: String,

    /// Job metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metadata: Option<JobMetadata>,

    /// Execution policy
    #[serde(skip_serializing_if = "Option::is_none")]
    pub execution: Option<ExecutionPolicy>,

    /// Capability and resource constraints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub constraints: Option<Constraints>,

    /// Routing and scheduling hints
    #[serde(skip_serializing_if = "Option::is_none")]
    pub routing: Option<Routing>,

    /// Operation-specific payload
    pub payload: Payload,

    /// Observability metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observability: Option<Observability>,

    /// Audit trail
    #[serde(skip_serializing_if = "Option::is_none")]
    pub audit: Option<Audit>,
}

/// Job metadata (labels, annotations, etc.)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct JobMetadata {
    /// Human-readable job name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,

    /// Logical namespace for isolation
    #[serde(skip_serializing_if = "Option::is_none")]
    pub namespace: Option<String>,

    /// Key-value labels for filtering and grouping
    #[serde(skip_serializing_if = "Option::is_none")]
    pub labels: Option<HashMap<String, String>>,

    /// Arbitrary annotations
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotations: Option<HashMap<String, String>>,
}

/// Execution policy and retry configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ExecutionPolicy {
    /// Unique key for idempotent execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,

    /// Current attempt number
    pub attempt: u32,

    /// Maximum retry attempts
    pub max_attempts: u32,

    /// Job timeout in seconds
    pub timeout_seconds: u64,

    /// Hard deadline for completion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,

    /// Job priority (1-10, higher = more urgent)
    pub priority: u8,

    /// Whether job can be cancelled
    pub cancellable: bool,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            idempotency_key: None,
            attempt: 1,
            max_attempts: 1,
            timeout_seconds: 3600,
            deadline: None,
            priority: 5,
            cancellable: true,
        }
    }
}

/// Capability and resource constraints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Constraints {
    /// Required worker capabilities (e.g., ["guestkit.inspect", "disk.qcow2"])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_capabilities: Option<Vec<String>>,

    /// Required system features (e.g., ["lvm", "selinux"])
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_features: Option<Vec<String>>,

    /// Minimum worker version (semver)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub minimum_worker_version: Option<String>,

    /// Maximum disk size worker can handle (GB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub maximum_disk_size_gb: Option<u64>,

    /// Requires privileged execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub require_privileged: Option<bool>,

    /// Allowed worker pool names
    #[serde(skip_serializing_if = "Option::is_none")]
    pub allowed_worker_pools: Option<Vec<String>>,
}

/// Routing and scheduling hints
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Routing {
    /// Pin to specific worker (use sparingly)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_id: Option<String>,

    /// Target worker pool
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_pool: Option<String>,

    /// Scheduling preferences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub affinity: Option<HashMap<String, String>>,

    /// Scheduling anti-preferences
    #[serde(skip_serializing_if = "Option::is_none")]
    pub anti_affinity: Option<HashMap<String, Vec<String>>>,
}

/// Operation-specific payload
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Payload {
    /// Payload type (namespace.operation.version)
    #[serde(rename = "type")]
    pub payload_type: String,

    /// Operation-specific data (free-form JSON)
    pub data: serde_json::Value,
}

/// Observability metadata (tracing, correlation)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Observability {
    /// Distributed trace ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<String>,

    /// Span ID within trace
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span_id: Option<String>,

    /// Parent span ID
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent_span_id: Option<String>,

    /// Correlation ID for related jobs
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<String>,
}

/// Audit trail
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct Audit {
    /// Submitter identity
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_by: Option<String>,

    /// Submitter IP/hostname
    #[serde(skip_serializing_if = "Option::is_none")]
    pub submitted_from: Option<String>,

    /// Authorization details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authorization: Option<Authorization>,
}

/// Authorization details
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Authorization {
    /// Authorization method
    pub method: String,

    /// Subject/principal
    pub subject: String,
}

// ========================================
// Worker Capabilities
// ========================================

/// Worker capability advertisement
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerCapabilities {
    /// Worker ID
    pub worker_id: String,

    /// Worker version
    pub version: String,

    /// Hostname
    pub hostname: String,

    /// Registration timestamp
    pub registered_at: DateTime<Utc>,

    /// Capabilities
    pub capabilities: WorkerCapabilitySet,

    /// Resource information
    pub resources: WorkerResources,

    /// Worker configuration
    pub configuration: WorkerConfiguration,

    /// Worker status
    pub status: WorkerStatus,
}

/// Worker capability set
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct WorkerCapabilitySet {
    /// Supported operations
    pub operations: Vec<String>,

    /// Supported features
    pub features: Vec<String>,

    /// Supported disk formats
    pub disk_formats: Vec<String>,
}

/// Worker resource information
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerResources {
    /// Maximum concurrent jobs
    pub max_concurrent_jobs: u32,

    /// Maximum disk size (GB)
    pub max_disk_size_gb: u64,

    /// Available disk space (GB)
    pub available_disk_gb: u64,

    /// CPU cores
    pub cpu_cores: u32,

    /// Memory (GB)
    pub memory_gb: u64,
}

/// Worker configuration
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerConfiguration {
    /// Whether worker runs privileged
    pub privileged: bool,

    /// Worker pool name
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worker_pool: Option<String>,

    /// Data locality paths
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data_locality: Option<Vec<String>>,
}

/// Worker status
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkerStatus {
    /// Worker state
    pub state: WorkerState,

    /// Current number of jobs
    pub current_jobs: u32,

    /// Last heartbeat timestamp
    pub last_heartbeat: DateTime<Utc>,
}

/// Worker state
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum WorkerState {
    Ready,
    Busy,
    Draining,
    Offline,
}

// ========================================
// Job Results
// ========================================

/// Job execution result
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobResult {
    /// Job ID
    pub job_id: String,

    /// Result status
    pub status: JobStatus,

    /// Completion timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,

    /// Failure timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failed_at: Option<DateTime<Utc>>,

    /// Worker ID that executed the job
    pub worker_id: String,

    /// Execution summary
    pub execution_summary: ExecutionSummary,

    /// Output files and artifacts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub outputs: Option<JobOutputs>,

    /// Execution metrics
    #[serde(skip_serializing_if = "Option::is_none")]
    pub metrics: Option<ExecutionMetrics>,

    /// Error details (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JobExecutionError>,

    /// Observability metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observability: Option<Observability>,

    /// Handler result payload (doctor, migrate-plan, etc.)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

/// Job status
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    Pending,
    Assigned,
    Running,
    Completed,
    Failed,
    Cancelled,
    Timeout,
}

/// Execution summary
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExecutionSummary {
    /// Start timestamp
    pub started_at: DateTime<Utc>,

    /// Duration in seconds
    pub duration_seconds: u64,

    /// Attempt number
    pub attempt: u32,

    /// Idempotency key (if provided)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub idempotency_key: Option<String>,
}

/// Job outputs
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct JobOutputs {
    /// Primary output file
    #[serde(skip_serializing_if = "Option::is_none")]
    pub primary: Option<String>,

    /// Additional artifacts
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifacts: Option<Vec<String>>,
}

/// Execution metrics
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct ExecutionMetrics {
    /// Disk bytes read
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_read_bytes: Option<u64>,

    /// Disk bytes written
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disk_write_bytes: Option<u64>,

    /// Peak memory usage (MB)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub peak_memory_mb: Option<u64>,

    /// CPU seconds consumed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cpu_seconds: Option<u64>,
}

/// Job execution error
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct JobExecutionError {
    /// Error code
    pub code: String,

    /// Error message
    pub message: String,

    /// Execution phase where error occurred
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,

    /// Additional error details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,

    /// Whether error is recoverable
    pub recoverable: bool,

    /// Whether retry is recommended
    pub retry_recommended: bool,
}

// ========================================
// Progress Events
// ========================================

/// Progress event emitted during job execution
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ProgressEvent {
    /// Job ID
    pub job_id: String,

    /// Event timestamp
    pub timestamp: DateTime<Utc>,

    /// Event sequence number
    pub sequence: u64,

    /// Execution phase
    pub phase: String,

    /// Progress percentage (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub progress_percent: Option<u8>,

    /// Human-readable message
    pub message: String,

    /// Additional details
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<HashMap<String, serde_json::Value>>,

    /// Observability metadata
    #[serde(skip_serializing_if = "Option::is_none")]
    pub observability: Option<Observability>,
}

// ========================================
// Guestkit-specific payload types
// ========================================

/// Guestkit inspect payload (v1)
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GuestkitInspectPayload {
    pub image: ImageSpec,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub options: Option<InspectOptions>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<OutputSpec>,
}

/// Image specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImageSpec {
    /// Path to image file
    pub path: String,

    /// Image format (qcow2, vmdk, etc.)
    pub format: String,

    /// Checksum for verification (SHA256)
    ///
    /// Supports two formats:
    /// - `"sha256:hexhash"` - Explicit algorithm specification
    /// - `"hexhash"` - Defaults to SHA256
    ///
    /// Example: `"sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"`
    ///
    /// When provided, the worker will compute the SHA256 hash of the image file
    /// and verify it matches the expected value before processing. This protects
    /// against corrupted or tampered images.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,

    /// Image size in bytes
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size_bytes: Option<u64>,

    /// Whether to open read-only
    #[serde(default = "default_true")]
    pub read_only: bool,
}

fn default_true() -> bool {
    true
}

/// Inspect options
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
#[serde(default)]
pub struct InspectOptions {
    pub deep_scan: bool,
    pub include_packages: bool,
    pub include_services: bool,
    pub include_users: bool,
    pub include_network: bool,
    pub include_security: bool,
    pub include_storage: bool,
    pub include_databases: bool,
}

/// Output specification
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct OutputSpec {
    /// Output format (json, yaml, etc.)
    pub format: String,

    /// Destination path
    pub destination: String,

    /// Optional compression
    #[serde(skip_serializing_if = "Option::is_none")]
    pub compression: Option<String>,
}

// ========================================
// Convenience types
// ========================================

/// Alias for the main Job type (for backward compatibility)
pub type Job = JobDocument;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_document_serialization() {
        let job = JobDocument {
            schema: Some("https://guestkit.dev/schemas/job-v1.json".to_string()),
            version: "1.0".to_string(),
            job_id: "job-test-123".to_string(),
            created_at: Utc::now(),
            kind: "VMOperation".to_string(),
            operation: "guestkit.inspect".to_string(),
            metadata: None,
            execution: None,
            constraints: None,
            routing: None,
            payload: Payload {
                payload_type: "guestkit.inspect.v1".to_string(),
                data: serde_json::json!({"test": "data"}),
            },
            observability: None,
            audit: None,
        };

        let json = serde_json::to_string_pretty(&job).unwrap();
        let deserialized: JobDocument = serde_json::from_str(&json).unwrap();

        assert_eq!(job, deserialized);
    }

    #[test]
    fn test_execution_policy_defaults() {
        let policy = ExecutionPolicy::default();
        assert_eq!(policy.attempt, 1);
        assert_eq!(policy.max_attempts, 1);
        assert_eq!(policy.timeout_seconds, 3600);
        assert_eq!(policy.priority, 5);
        assert!(policy.cancellable);
    }
}
