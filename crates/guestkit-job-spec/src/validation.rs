// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Job validation logic

use crate::error::{JobError, JobResult};
use crate::types::{JobDocument, Payload};
use crate::PROTOCOL_VERSION;

/// Job validator
pub struct JobValidator;

impl JobValidator {
    /// Validate a job document
    pub fn validate(job: &JobDocument) -> JobResult<()> {
        // Validate protocol version
        Self::validate_version(&job.version)?;

        // Validate job ID
        Self::validate_job_id(&job.job_id)?;

        // Validate kind
        Self::validate_kind(&job.kind)?;

        // Validate operation
        Self::validate_operation(&job.operation)?;

        // Validate payload
        Self::validate_payload(&job.payload)?;

        // Validate execution policy if present
        if let Some(ref execution) = job.execution {
            Self::validate_execution_policy(execution)?;
        }

        // Validate constraints if present
        if let Some(ref constraints) = job.constraints {
            Self::validate_constraints(constraints)?;
        }

        Ok(())
    }

    /// Validate protocol version
    fn validate_version(version: &str) -> JobResult<()> {
        if version != PROTOCOL_VERSION {
            return Err(JobError::UnsupportedVersion(version.to_string()));
        }
        Ok(())
    }

    /// Validate job ID
    fn validate_job_id(job_id: &str) -> JobResult<()> {
        if job_id.is_empty() {
            return Err(JobError::MissingField("job_id".to_string()));
        }

        if job_id.len() < 8 {
            return Err(JobError::InvalidField {
                field: "job_id".to_string(),
                reason: "must be at least 8 characters".to_string(),
            });
        }

        Ok(())
    }

    /// Validate job kind
    fn validate_kind(kind: &str) -> JobResult<()> {
        if kind != "VMOperation" {
            return Err(JobError::InvalidField {
                field: "kind".to_string(),
                reason: format!("must be 'VMOperation', got '{}'", kind),
            });
        }
        Ok(())
    }

    /// Validate operation name
    fn validate_operation(operation: &str) -> JobResult<()> {
        if operation.is_empty() {
            return Err(JobError::MissingField("operation".to_string()));
        }

        // Must be namespaced (contains at least one dot)
        if !operation.contains('.') {
            return Err(JobError::InvalidField {
                field: "operation".to_string(),
                reason: "must be namespaced (e.g., 'guestkit.inspect')".to_string(),
            });
        }

        Ok(())
    }

    /// Validate payload
    fn validate_payload(payload: &Payload) -> JobResult<()> {
        if payload.payload_type.is_empty() {
            return Err(JobError::MissingField("payload.type".to_string()));
        }

        // Payload type should match pattern: namespace.operation.version
        let parts: Vec<&str> = payload.payload_type.split('.').collect();
        if parts.len() < 3 {
            return Err(JobError::InvalidField {
                field: "payload.type".to_string(),
                reason: "must be namespaced with version (e.g., 'guestkit.inspect.v1')".to_string(),
            });
        }

        // Last part should start with 'v' followed by a number
        if let Some(version_part) = parts.last() {
            if !version_part.starts_with('v') {
                return Err(JobError::InvalidField {
                    field: "payload.type".to_string(),
                    reason: format!("version part must start with 'v', got '{}'", version_part),
                });
            }
        }

        Ok(())
    }

    /// Validate execution policy
    fn validate_execution_policy(policy: &crate::types::ExecutionPolicy) -> JobResult<()> {
        // Priority must be 1-10
        if policy.priority < 1 || policy.priority > 10 {
            return Err(JobError::InvalidField {
                field: "execution.priority".to_string(),
                reason: format!("must be 1-10, got {}", policy.priority),
            });
        }

        // Max attempts must be at least 1
        if policy.max_attempts < 1 {
            return Err(JobError::InvalidField {
                field: "execution.max_attempts".to_string(),
                reason: "must be at least 1".to_string(),
            });
        }

        // Attempt must be <= max_attempts
        if policy.attempt > policy.max_attempts {
            return Err(JobError::InvalidField {
                field: "execution.attempt".to_string(),
                reason: format!(
                    "attempt ({}) cannot exceed max_attempts ({})",
                    policy.attempt, policy.max_attempts
                ),
            });
        }

        // Timeout should be reasonable (warn if > 24 hours)
        const MAX_REASONABLE_TIMEOUT: u64 = 86400; // 24 hours
        if policy.timeout_seconds > MAX_REASONABLE_TIMEOUT {
            eprintln!(
                "Warning: timeout_seconds ({}) exceeds 24 hours",
                policy.timeout_seconds
            );
        }

        Ok(())
    }

    /// Validate constraints
    fn validate_constraints(constraints: &crate::types::Constraints) -> JobResult<()> {
        // Validate semver if minimum_worker_version is provided
        if let Some(ref version) = constraints.minimum_worker_version {
            if version.is_empty() {
                return Err(JobError::InvalidField {
                    field: "constraints.minimum_worker_version".to_string(),
                    reason: "cannot be empty".to_string(),
                });
            }
        }

        // Validate disk size is reasonable
        if let Some(disk_size) = constraints.maximum_disk_size_gb {
            const MAX_REASONABLE_DISK_SIZE: u64 = 100_000; // 100TB
            if disk_size > MAX_REASONABLE_DISK_SIZE {
                eprintln!(
                    "Warning: maximum_disk_size_gb ({}) exceeds 100TB",
                    disk_size
                );
            }
        }

        Ok(())
    }

    /// Check if worker capabilities match job requirements
    pub fn check_capabilities(required: &[String], available: &[String]) -> JobResult<()> {
        let missing: Vec<String> = required
            .iter()
            .filter(|req| !available.contains(req))
            .cloned()
            .collect();

        if !missing.is_empty() {
            return Err(JobError::CapabilityMismatch {
                required: required.to_vec(),
                available: available.to_vec(),
            });
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{JobDocument, Payload};
    use chrono::Utc;

    fn create_minimal_valid_job() -> JobDocument {
        JobDocument {
            schema: None,
            version: "1.0".to_string(),
            job_id: "job-12345678".to_string(),
            created_at: Utc::now(),
            kind: "VMOperation".to_string(),
            operation: "guestkit.inspect".to_string(),
            metadata: None,
            execution: None,
            constraints: None,
            routing: None,
            payload: Payload {
                payload_type: "guestkit.inspect.v1".to_string(),
                data: serde_json::json!({}),
            },
            observability: None,
            audit: None,
        }
    }

    #[test]
    fn test_validate_valid_job() {
        let job = create_minimal_valid_job();
        assert!(JobValidator::validate(&job).is_ok());
    }

    #[test]
    fn test_validate_invalid_version() {
        let mut job = create_minimal_valid_job();
        job.version = "2.0".to_string();

        let result = JobValidator::validate(&job);
        assert!(matches!(result, Err(JobError::UnsupportedVersion(_))));
    }

    #[test]
    fn test_validate_short_job_id() {
        let mut job = create_minimal_valid_job();
        job.job_id = "short".to_string();

        let result = JobValidator::validate(&job);
        assert!(matches!(result, Err(JobError::InvalidField { .. })));
    }

    #[test]
    fn test_validate_invalid_kind() {
        let mut job = create_minimal_valid_job();
        job.kind = "InvalidKind".to_string();

        let result = JobValidator::validate(&job);
        assert!(matches!(result, Err(JobError::InvalidField { .. })));
    }

    #[test]
    fn test_validate_non_namespaced_operation() {
        let mut job = create_minimal_valid_job();
        job.operation = "inspect".to_string();

        let result = JobValidator::validate(&job);
        assert!(matches!(result, Err(JobError::InvalidField { .. })));
    }

    #[test]
    fn test_validate_invalid_payload_type() {
        let mut job = create_minimal_valid_job();
        job.payload.payload_type = "invalid".to_string();

        let result = JobValidator::validate(&job);
        assert!(matches!(result, Err(JobError::InvalidField { .. })));
    }

    #[test]
    fn test_check_capabilities_match() {
        let required = vec!["lvm".to_string(), "nbd".to_string()];
        let available = vec!["lvm".to_string(), "nbd".to_string(), "selinux".to_string()];

        assert!(JobValidator::check_capabilities(&required, &available).is_ok());
    }

    #[test]
    fn test_check_capabilities_missing() {
        let required = vec!["lvm".to_string(), "windows-registry".to_string()];
        let available = vec!["lvm".to_string(), "nbd".to_string()];

        let result = JobValidator::check_capabilities(&required, &available);
        assert!(matches!(result, Err(JobError::CapabilityMismatch { .. })));
    }
}
