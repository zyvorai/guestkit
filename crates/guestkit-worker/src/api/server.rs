// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! REST API server

use axum::{
    routing::{get, post},
    Router,
};
use std::net::SocketAddr;
use tokio::task::JoinHandle;
use tower_http::trace::TraceLayer;

use super::handlers::{
    ApiState, submit_job, get_job_status, get_job_result,
    list_jobs, get_capabilities, health_check,
};

/// API server configuration
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Address to bind to (e.g., "0.0.0.0:8080")
    pub bind_addr: SocketAddr,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:8080".parse().unwrap(),
        }
    }
}

/// REST API server
pub struct ApiServer {
    config: ApiServerConfig,
    state: ApiState,
}

impl ApiServer {
    /// Create a new API server
    pub fn new(config: ApiServerConfig, state: ApiState) -> Self {
        Self { config, state }
    }

    /// Start the API server
    ///
    /// Returns a join handle that can be awaited or aborted
    pub async fn start(self) -> std::io::Result<JoinHandle<()>> {
        let app = Router::new()
            // Job management endpoints
            .route("/api/v1/jobs", post(submit_job))
            .route("/api/v1/jobs", get(list_jobs))
            .route("/api/v1/jobs/:id", get(get_job_status))
            .route("/api/v1/jobs/:id/result", get(get_job_result))
            // Worker endpoints
            .route("/api/v1/capabilities", get(get_capabilities))
            // Health check
            .route("/api/v1/health", get(health_check))
            .route("/health", get(health_check))
            // Add state and middleware
            .with_state(self.state)
            .layer(TraceLayer::new_for_http());

        log::info!("Starting REST API server on {}", self.config.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                log::error!("API server error: {}", e);
            }
        });

        Ok(handle)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::handlers;
    use crate::capabilities::Capabilities;
    use std::sync::Arc;

    struct MockJobSubmitter;
    #[async_trait::async_trait]
    impl handlers::JobSubmitter for MockJobSubmitter {
        async fn submit_job(&self, job: guestkit_job_spec::JobDocument) -> Result<String, String> {
            Ok(job.job_id)
        }
    }

    struct MockJobStatusLookup;
    #[async_trait::async_trait]
    impl handlers::JobStatusLookup for MockJobStatusLookup {
        async fn get_status(&self, _job_id: &str) -> Option<super::super::types::JobStatusResponse> {
            None
        }

        async fn list_jobs(&self) -> Vec<super::super::types::JobStatusResponse> {
            vec![]
        }

        async fn get_result(&self, _job_id: &str) -> Option<serde_json::Value> {
            None
        }
    }

    #[test]
    fn test_api_server_config() {
        let config = ApiServerConfig::default();
        assert_eq!(config.bind_addr.port(), 8080);
    }

    #[tokio::test]
    async fn test_api_server_creation() {
        let config = ApiServerConfig::default();
        let state = ApiState {
            worker_id: "test-worker".to_string(),
            capabilities: Capabilities::new(),
            job_submitter: Arc::new(MockJobSubmitter),
            job_status_lookup: Arc::new(MockJobStatusLookup),
        };

        let server = ApiServer::new(config, state);
        assert_eq!(server.config.bind_addr.port(), 8080);
    }
}
