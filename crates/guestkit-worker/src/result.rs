// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Job result persistence

use guestkit_job_spec::{
    JobResultType, JobStatus, ExecutionSummary, JobOutputs, JobExecutionError,
};
use chrono::Utc;
use std::path::Path;
use tokio::fs;
use crate::error::WorkerResult;

/// Result writer
pub struct ResultWriter {
    output_dir: std::path::PathBuf,
}

impl ResultWriter {
    /// Create a new result writer
    pub fn new(output_dir: impl Into<std::path::PathBuf>) -> Self {
        Self {
            output_dir: output_dir.into(),
        }
    }

    /// Write successful result
    pub async fn write_success(
        &self,
        job_id: &str,
        worker_id: &str,
        started_at: chrono::DateTime<Utc>,
        attempt: u32,
        idempotency_key: Option<String>,
        output_file: Option<String>,
        artifacts: Vec<String>,
        data: Option<serde_json::Value>,
    ) -> WorkerResult<String> {
        let duration = (Utc::now() - started_at).num_seconds() as u64;

        let result = JobResultType {
            job_id: job_id.to_string(),
            status: JobStatus::Completed,
            completed_at: Some(Utc::now()),
            failed_at: None,
            worker_id: worker_id.to_string(),
            execution_summary: ExecutionSummary {
                started_at,
                duration_seconds: duration,
                attempt,
                idempotency_key,
            },
            outputs: Some(JobOutputs {
                primary: output_file,
                artifacts: if artifacts.is_empty() {
                    None
                } else {
                    Some(artifacts)
                },
            }),
            metrics: None,
            error: None,
            observability: None,
            data,
        };

        self.write_result(&result).await
    }

    /// Write failure result
    pub async fn write_failure(
        &self,
        job_id: &str,
        worker_id: &str,
        started_at: chrono::DateTime<Utc>,
        attempt: u32,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
        phase: Option<String>,
        recoverable: bool,
    ) -> WorkerResult<String> {
        let duration = (Utc::now() - started_at).num_seconds() as u64;

        let result = JobResultType {
            job_id: job_id.to_string(),
            status: JobStatus::Failed,
            completed_at: None,
            failed_at: Some(Utc::now()),
            worker_id: worker_id.to_string(),
            execution_summary: ExecutionSummary {
                started_at,
                duration_seconds: duration,
                attempt,
                idempotency_key: None,
            },
            outputs: None,
            metrics: None,
            error: Some(JobExecutionError {
                code: error_code.into(),
                message: error_message.into(),
                phase,
                details: None,
                recoverable,
                retry_recommended: recoverable,
            }),
            observability: None,
            data: None,
        };

        self.write_result(&result).await
    }

    /// Write result to file
    async fn write_result(&self, result: &JobResultType) -> WorkerResult<String> {
        fs::create_dir_all(&self.output_dir).await?;

        let filename = format!("{}-result.json", result.job_id);
        let path = self.output_dir.join(&filename);

        let json = serde_json::to_string_pretty(result)?;
        fs::write(&path, json).await?;

        log::info!("Wrote result to {}", path.display());

        Ok(path.to_string_lossy().to_string())
    }

    /// Read result from file
    pub async fn read_result(&self, job_id: &str) -> WorkerResult<JobResultType> {
        let filename = format!("{}-result.json", job_id);
        let path = self.output_dir.join(&filename);

        let json = fs::read_to_string(&path).await?;
        let result = serde_json::from_str(&json)?;

        Ok(result)
    }

    /// Check if result exists
    pub async fn result_exists(&self, job_id: &str) -> bool {
        let filename = format!("{}-result.json", job_id);
        let path = self.output_dir.join(&filename);

        path.exists()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_write_success_result() {
        let temp_dir = TempDir::new().unwrap();
        let writer = ResultWriter::new(temp_dir.path());

        let started_at = Utc::now();

        let path = writer
            .write_success(
                "job-test-123",
                "worker-01",
                started_at,
                1,
                Some("idempotency-key".to_string()),
                Some("/output/result.json".to_string()),
                vec!["/output/log.txt".to_string()],
                None,
            )
            .await
            .unwrap();

        assert!(Path::new(&path).exists());

        // Read back
        let result = writer.read_result("job-test-123").await.unwrap();
        assert_eq!(result.status, JobStatus::Completed);
        assert_eq!(result.job_id, "job-test-123");
    }

    #[tokio::test]
    async fn test_write_failure_result() {
        let temp_dir = TempDir::new().unwrap();
        let writer = ResultWriter::new(temp_dir.path());

        let started_at = Utc::now();

        let path = writer
            .write_failure(
                "job-test-456",
                "worker-01",
                started_at,
                1,
                "VALIDATION_ERROR",
                "Job validation failed",
                Some("validation".to_string()),
                false,
            )
            .await
            .unwrap();

        assert!(Path::new(&path).exists());

        // Read back
        let result = writer.read_result("job-test-456").await.unwrap();
        assert_eq!(result.status, JobStatus::Failed);
        assert!(result.error.is_some());
    }
}
