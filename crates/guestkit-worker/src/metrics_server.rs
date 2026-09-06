// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! HTTP server for Prometheus metrics endpoint
//!
//! Provides a simple HTTP server that exposes worker metrics at /metrics

use crate::metrics::MetricsRegistry;
use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::get,
    Router,
};
use std::net::SocketAddr;
use std::sync::Arc;
use tokio::task::JoinHandle;

/// Metrics server configuration
#[derive(Debug, Clone)]
pub struct MetricsServerConfig {
    /// Address to bind to (e.g., "0.0.0.0:9090")
    pub bind_addr: SocketAddr,
}

impl Default for MetricsServerConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:9090".parse().unwrap(),
        }
    }
}

/// Metrics HTTP server
pub struct MetricsServer {
    config: MetricsServerConfig,
    metrics: Arc<MetricsRegistry>,
}

impl MetricsServer {
    /// Create a new metrics server
    pub fn new(config: MetricsServerConfig, metrics: Arc<MetricsRegistry>) -> Self {
        Self { config, metrics }
    }

    /// Start the metrics server
    ///
    /// Returns a join handle that can be awaited or aborted
    pub async fn start(self) -> std::io::Result<JoinHandle<()>> {
        let app = Router::new()
            .route("/metrics", get(metrics_handler))
            .route("/health", get(health_handler))
            .with_state(self.metrics);

        log::info!("Starting metrics server on {}", self.config.bind_addr);

        let listener = tokio::net::TcpListener::bind(self.config.bind_addr).await?;

        let handle = tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                log::error!("Metrics server error: {}", e);
            }
        });

        Ok(handle)
    }
}

/// Handler for /metrics endpoint
async fn metrics_handler(
    State(metrics): State<Arc<MetricsRegistry>>,
) -> Response {
    let body = metrics.encode();
    (
        StatusCode::OK,
        [("content-type", "text/plain; version=0.0.4")],
        body,
    )
        .into_response()
}

/// Handler for /health endpoint
async fn health_handler() -> Response {
    (
        StatusCode::OK,
        [("content-type", "application/json")],
        r#"{"status":"healthy"}"#,
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_server_config() {
        let config = MetricsServerConfig::default();
        assert_eq!(config.bind_addr.port(), 9090);
    }

    #[tokio::test]
    async fn test_metrics_handler() {
        let metrics = Arc::new(MetricsRegistry::new());

        // Record some metrics
        metrics.record_job_completion("test.op", "completed", 1.5);

        let response = metrics_handler(State(metrics)).await;
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_handler() {
        let response = health_handler().await;
        assert_eq!(response.status(), StatusCode::OK);
    }
}
