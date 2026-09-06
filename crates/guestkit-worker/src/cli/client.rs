// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTTP client for guestkit-worker REST API

use anyhow::{Result, Context};
use guestkit_job_spec::JobDocument;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// API response wrapper
#[derive(Debug, Deserialize, Serialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

/// Job submission request
#[derive(Debug, Serialize)]
pub struct JobSubmitRequest {
    #[serde(flatten)]
    pub job: JobDocument,
}

/// Job submission response
#[derive(Debug, Deserialize, Serialize)]
pub struct JobSubmitResponse {
    pub job_id: String,
    pub status: String,
    pub message: String,
}

/// Job status response
#[derive(Debug, Deserialize, Serialize)]
pub struct JobStatusResponse {
    pub job_id: String,
    pub status: String,
    pub operation: String,
    pub submitted_at: String,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub worker_id: Option<String>,
    pub error: Option<String>,
}

/// Job list response
#[derive(Debug, Deserialize, Serialize)]
pub struct JobListResponse {
    pub jobs: Vec<JobStatusResponse>,
    pub total: usize,
}

/// Worker capabilities response
#[derive(Debug, Deserialize, Serialize)]
pub struct CapabilitiesResponse {
    pub worker_id: String,
    pub pool: Option<String>,
    pub operations: Vec<String>,
    pub features: Vec<String>,
    pub disk_formats: Vec<String>,
}

/// Health check response
#[derive(Debug, Deserialize, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub uptime_seconds: u64,
}

/// HTTP client for worker REST API
pub struct WorkerClient {
    base_url: String,
    client: reqwest::Client,
}

impl WorkerClient {
    /// Create a new client
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    /// Submit a job
    pub async fn submit_job(&self, job: JobDocument) -> Result<JobSubmitResponse> {
        let url = format!("{}/api/v1/jobs", self.base_url);

        let response = self.client
            .post(&url)
            .json(&JobSubmitRequest { job })
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<JobSubmitResponse> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }

    /// Get job status
    pub async fn get_job_status(&self, job_id: &str) -> Result<JobStatusResponse> {
        let url = format!("{}/api/v1/jobs/{}", self.base_url, job_id);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<JobStatusResponse> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }

    /// Get job result
    pub async fn get_job_result(&self, job_id: &str) -> Result<serde_json::Value> {
        let url = format!("{}/api/v1/jobs/{}/result", self.base_url, job_id);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<serde_json::Value> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> Result<JobListResponse> {
        let url = format!("{}/api/v1/jobs", self.base_url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<JobListResponse> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }

    /// Get worker capabilities
    pub async fn get_capabilities(&self) -> Result<CapabilitiesResponse> {
        let url = format!("{}/api/v1/capabilities", self.base_url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<CapabilitiesResponse> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }

    /// Health check
    pub async fn health_check(&self) -> Result<HealthResponse> {
        let url = format!("{}/api/v1/health", self.base_url);

        let response = self.client
            .get(&url)
            .send()
            .await
            .context("Failed to send request")?;

        if !response.status().is_success() {
            let error_text = response.text().await.unwrap_or_default();
            anyhow::bail!("API error: {}", error_text);
        }

        let api_response: ApiResponse<HealthResponse> = response
            .json()
            .await
            .context("Failed to parse response")?;

        Ok(api_response.data)
    }
}
