// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! API request handlers

use axum::{
    extract::{Path, State},
    Json,
};
use chrono::Utc;
use guestkit_job_spec::{JobDocument, JobValidator, JobStatus};
use std::sync::Arc;

use super::types::{
    ApiError, ApiResponse, JobSubmitRequest, JobSubmitResponse,
    JobStatusResponse, JobListResponse, CapabilitiesResponse,
};
use crate::capabilities::Capabilities;

/// Shared API state
#[derive(Clone)]
pub struct ApiState {
    /// Worker ID
    pub worker_id: String,
    /// Worker capabilities
    pub capabilities: Capabilities,
    /// Job submission callback
    pub job_submitter: Arc<dyn JobSubmitter>,
    /// Job status lookup callback
    pub job_status_lookup: Arc<dyn JobStatusLookup>,
}

/// Trait for submitting jobs
#[async_trait::async_trait]
pub trait JobSubmitter: Send + Sync {
    async fn submit_job(&self, job: JobDocument) -> Result<String, String>;
}

/// Trait for looking up job status
#[async_trait::async_trait]
pub trait JobStatusLookup: Send + Sync {
    async fn get_status(&self, job_id: &str) -> Option<JobStatusResponse>;
    async fn list_jobs(&self) -> Vec<JobStatusResponse>;
    async fn get_result(&self, job_id: &str) -> Option<serde_json::Value>;
}

/// POST /api/v1/jobs - Submit a new job
pub async fn submit_job(
    State(state): State<ApiState>,
    Json(request): Json<JobSubmitRequest>,
) -> Result<Json<ApiResponse<JobSubmitResponse>>, ApiError> {
    let mut job = request.job;

    // Validate job
    if let Err(e) = JobValidator::validate(&job) {
        return Err(ApiError::validation_error(format!("Job validation failed: {}", e)));
    }

    // Ensure job has created_at
    if job.created_at.timestamp() == 0 {
        job.created_at = Utc::now();
    }

    let job_id = job.job_id.clone();

    // Submit job
    match state.job_submitter.submit_job(job).await {
        Ok(_) => {
            let response = JobSubmitResponse {
                job_id: job_id.clone(),
                status: "submitted".to_string(),
                message: format!("Job {} submitted successfully", job_id),
            };
            Ok(Json(ApiResponse::success(response)))
        }
        Err(e) => Err(ApiError::internal_error(format!("Failed to submit job: {}", e))),
    }
}

/// GET /api/v1/jobs/:id - Get job status
pub async fn get_job_status(
    State(state): State<ApiState>,
    Path(job_id): Path<String>,
) -> Result<Json<ApiResponse<JobStatusResponse>>, ApiError> {
    match state.job_status_lookup.get_status(&job_id).await {
        Some(status) => Ok(Json(ApiResponse::success(status))),
        None => Err(ApiError::not_found(format!("Job {} not found", job_id))),
    }
}

/// GET /api/v1/jobs/:id/result - Get job result
pub async fn get_job_result(
    State(state): State<ApiState>,
    Path(job_id): Path<String>,
) -> Result<Json<ApiResponse<serde_json::Value>>, ApiError> {
    match state.job_status_lookup.get_result(&job_id).await {
        Some(result) => Ok(Json(ApiResponse::success(result))),
        None => Err(ApiError::not_found(format!("Result for job {} not found", job_id))),
    }
}

/// GET /api/v1/jobs - List all jobs
pub async fn list_jobs(
    State(state): State<ApiState>,
) -> Json<ApiResponse<JobListResponse>> {
    let jobs = state.job_status_lookup.list_jobs().await;
    let total = jobs.len();

    Json(ApiResponse::success(JobListResponse { jobs, total }))
}

/// GET /api/v1/capabilities - Get worker capabilities
pub async fn get_capabilities(
    State(state): State<ApiState>,
) -> Json<ApiResponse<CapabilitiesResponse>> {
    let response = CapabilitiesResponse {
        worker_id: state.worker_id.clone(),
        operations: state.capabilities.operations.clone(),
        features: state.capabilities.features.clone(),
        disk_formats: state.capabilities.disk_formats.clone(),
        max_concurrent_jobs: state.capabilities.max_concurrent_jobs,
        max_disk_size_gb: state.capabilities.max_disk_size_gb,
    };

    Json(ApiResponse::success(response))
}

/// GET /api/v1/health - Health check
pub async fn health_check() -> Json<ApiResponse<serde_json::Value>> {
    Json(ApiResponse::success(serde_json::json!({
        "status": "healthy",
        "timestamp": Utc::now().to_rfc3339(),
    })))
}

#[cfg(test)]
mod tests {
    use super::*;
    use guestkit_job_spec::builder::JobBuilder;

    struct MockJobSubmitter;
    #[async_trait::async_trait]
    impl JobSubmitter for MockJobSubmitter {
        async fn submit_job(&self, job: JobDocument) -> Result<String, String> {
            Ok(job.job_id)
        }
    }

    struct MockJobStatusLookup;
    #[async_trait::async_trait]
    impl JobStatusLookup for MockJobStatusLookup {
        async fn get_status(&self, job_id: &str) -> Option<JobStatusResponse> {
            Some(JobStatusResponse {
                job_id: job_id.to_string(),
                status: JobStatus::Pending,
                submitted_at: Some(Utc::now()),
                started_at: None,
                completed_at: None,
                error: None,
            })
        }

        async fn list_jobs(&self) -> Vec<JobStatusResponse> {
            vec![]
        }

        async fn get_result(&self, _job_id: &str) -> Option<serde_json::Value> {
            Some(serde_json::json!({"result": "test"}))
        }
    }

    fn create_test_state() -> ApiState {
        ApiState {
            worker_id: "test-worker".to_string(),
            capabilities: Capabilities::new(),
            job_submitter: Arc::new(MockJobSubmitter),
            job_status_lookup: Arc::new(MockJobStatusLookup),
        }
    }

    #[tokio::test]
    async fn test_submit_job() {
        let state = create_test_state();

        let job = JobBuilder::new()
            .job_id("test-job-001")
            .operation("test.operation")
            .payload("test.operation.v1", serde_json::json!({}))
            .build()
            .unwrap();

        let request = JobSubmitRequest { job };

        let result = submit_job(
            State(state),
            Json(request),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_get_job_status() {
        let state = create_test_state();

        let result = get_job_status(
            State(state),
            Path("test-job-001".to_string()),
        ).await;

        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_health_check() {
        let result = health_check().await;
        assert!(result.0.success);
    }
}
