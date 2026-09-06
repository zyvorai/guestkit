// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTTP-based job transport (REST API)
//!
//! This transport receives jobs via REST API endpoints instead of filesystem watching.
//! Jobs are submitted to the API and queued in memory for the worker to process.

use async_trait::async_trait;
use guestkit_job_spec::JobDocument;
use std::collections::VecDeque;
use std::sync::Arc;
use tokio::sync::Mutex;

use crate::error::WorkerResult;
use crate::transport::JobTransport;
use crate::api::handlers::{JobSubmitter, JobStatusLookup};
use crate::api::types::JobStatusResponse;
use guestkit_job_spec::JobStatus;

/// HTTP transport configuration
#[derive(Debug, Clone)]
pub struct HttpTransportConfig {
    /// Maximum queue size
    pub max_queue_size: usize,
}

impl Default for HttpTransportConfig {
    fn default() -> Self {
        Self {
            max_queue_size: 1000,
        }
    }
}

/// HTTP-based job transport
///
/// Jobs are submitted via REST API and queued in memory.
/// The worker fetches jobs from the queue.
pub struct HttpTransport {
    /// Configuration
    _config: HttpTransportConfig,
    /// Job queue (pending jobs)
    queue: Arc<Mutex<VecDeque<JobDocument>>>,
    /// Job status tracking
    status_map: Arc<Mutex<std::collections::HashMap<String, JobStatusInfo>>>,
}

#[derive(Debug, Clone)]
struct JobStatusInfo {
    status: JobStatus,
    submitted_at: chrono::DateTime<chrono::Utc>,
    started_at: Option<chrono::DateTime<chrono::Utc>>,
    completed_at: Option<chrono::DateTime<chrono::Utc>>,
    error: Option<String>,
    result: Option<serde_json::Value>,
}

impl HttpTransport {
    /// Create a new HTTP transport
    pub fn new(config: HttpTransportConfig) -> Self {
        Self {
            _config: config,
            queue: Arc::new(Mutex::new(VecDeque::new())),
            status_map: Arc::new(Mutex::new(std::collections::HashMap::new())),
        }
    }

    /// Get a handle for job submission (used by API)
    pub fn get_submitter(&self) -> Arc<dyn JobSubmitter> {
        Arc::new(HttpJobSubmitter {
            queue: Arc::clone(&self.queue),
            status_map: Arc::clone(&self.status_map),
        })
    }

    /// Get a handle for job status lookup (used by API)
    pub fn get_status_lookup(&self) -> Arc<dyn JobStatusLookup> {
        Arc::new(HttpJobStatusLookup {
            status_map: Arc::clone(&self.status_map),
        })
    }
}

#[async_trait]
impl JobTransport for HttpTransport {
    async fn fetch_job(&mut self) -> WorkerResult<Option<JobDocument>> {
        let mut queue = self.queue.lock().await;

        if let Some(job) = queue.pop_front() {
            // Update status to assigned
            let mut status_map = self.status_map.lock().await;
            if let Some(info) = status_map.get_mut(&job.job_id) {
                info.status = JobStatus::Assigned;
                info.started_at = Some(chrono::Utc::now());
            }
            Ok(Some(job))
        } else {
            Ok(None)
        }
    }

    async fn ack_job(&mut self, job_id: &str) -> WorkerResult<()> {
        let mut status_map = self.status_map.lock().await;
        if let Some(info) = status_map.get_mut(job_id) {
            info.status = JobStatus::Completed;
            info.completed_at = Some(chrono::Utc::now());
        }
        Ok(())
    }

    async fn nack_job(&mut self, job_id: &str, reason: &str) -> WorkerResult<()> {
        let mut status_map = self.status_map.lock().await;
        if let Some(info) = status_map.get_mut(job_id) {
            info.status = JobStatus::Failed;
            info.completed_at = Some(chrono::Utc::now());
            info.error = Some(reason.to_string());
        }
        Ok(())
    }

    async fn health_check(&self) -> WorkerResult<bool> {
        Ok(true)
    }
}

/// Job submitter implementation for HTTP transport
struct HttpJobSubmitter {
    queue: Arc<Mutex<VecDeque<JobDocument>>>,
    status_map: Arc<Mutex<std::collections::HashMap<String, JobStatusInfo>>>,
}

#[async_trait::async_trait]
impl JobSubmitter for HttpJobSubmitter {
    async fn submit_job(&self, job: JobDocument) -> Result<String, String> {
        let job_id = job.job_id.clone();

        // Add to queue
        let mut queue = self.queue.lock().await;
        queue.push_back(job);

        // Add to status tracking
        let mut status_map = self.status_map.lock().await;
        status_map.insert(
            job_id.clone(),
            JobStatusInfo {
                status: JobStatus::Pending,
                submitted_at: chrono::Utc::now(),
                started_at: None,
                completed_at: None,
                error: None,
                result: None,
            },
        );

        Ok(job_id)
    }
}

/// Job status lookup implementation for HTTP transport
struct HttpJobStatusLookup {
    status_map: Arc<Mutex<std::collections::HashMap<String, JobStatusInfo>>>,
}

#[async_trait::async_trait]
impl JobStatusLookup for HttpJobStatusLookup {
    async fn get_status(&self, job_id: &str) -> Option<JobStatusResponse> {
        let status_map = self.status_map.lock().await;
        status_map.get(job_id).map(|info| JobStatusResponse {
            job_id: job_id.to_string(),
            status: info.status,
            submitted_at: Some(info.submitted_at),
            started_at: info.started_at,
            completed_at: info.completed_at,
            error: info.error.clone(),
        })
    }

    async fn list_jobs(&self) -> Vec<JobStatusResponse> {
        let status_map = self.status_map.lock().await;
        status_map
            .iter()
            .map(|(job_id, info)| JobStatusResponse {
                job_id: job_id.clone(),
                status: info.status,
                submitted_at: Some(info.submitted_at),
                started_at: info.started_at,
                completed_at: info.completed_at,
                error: info.error.clone(),
            })
            .collect()
    }

    async fn get_result(&self, job_id: &str) -> Option<serde_json::Value> {
        let status_map = self.status_map.lock().await;
        status_map.get(job_id).and_then(|info| info.result.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use guestkit_job_spec::builder::JobBuilder;

    #[tokio::test]
    async fn test_http_transport_submit_and_fetch() {
        let config = HttpTransportConfig::default();
        let mut transport = HttpTransport::new(config);

        // Submit a job
        let submitter = transport.get_submitter();
        let job = JobBuilder::new()
            .job_id("test-job-001")
            .operation("test.operation")
            .payload("test.operation.v1", serde_json::json!({}))
            .build()
            .unwrap();

        let result = submitter.submit_job(job).await;
        assert!(result.is_ok());

        // Fetch the job
        let fetched = transport.fetch_job().await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().job_id, "test-job-001");
    }

    #[tokio::test]
    async fn test_http_transport_status_lookup() {
        let config = HttpTransportConfig::default();
        let transport = HttpTransport::new(config);

        // Submit a job
        let submitter = transport.get_submitter();
        let job = JobBuilder::new()
            .job_id("test-job-002")
            .operation("test.operation")
            .payload("test.operation.v1", serde_json::json!({}))
            .build()
            .unwrap();

        submitter.submit_job(job).await.unwrap();

        // Look up status
        let lookup = transport.get_status_lookup();
        let status = lookup.get_status("test-job-002").await;
        assert!(status.is_some());
        assert_eq!(status.unwrap().status, JobStatus::Pending);
    }

    #[tokio::test]
    async fn test_http_transport_ack() {
        let config = HttpTransportConfig::default();
        let mut transport = HttpTransport::new(config);

        // Submit and fetch job
        let submitter = transport.get_submitter();
        let job = JobBuilder::new()
            .job_id("test-job-003")
            .operation("test.operation")
            .payload("test.operation.v1", serde_json::json!({}))
            .build()
            .unwrap();

        submitter.submit_job(job).await.unwrap();
        transport.fetch_job().await.unwrap();

        // Acknowledge job
        transport.ack_job("test-job-003").await.unwrap();

        // Check status
        let lookup = transport.get_status_lookup();
        let status = lookup.get_status("test-job-003").await;
        assert_eq!(status.unwrap().status, JobStatus::Completed);
    }
}
