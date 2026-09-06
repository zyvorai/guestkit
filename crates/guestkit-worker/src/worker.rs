// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Main worker daemon

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::signal;
use crate::error::{WorkerError, WorkerResult};
use crate::executor::JobExecutor;
use crate::handler::HandlerRegistry;
use crate::result::ResultWriter;
use crate::transport::JobTransport;
use crate::capabilities::Capabilities;
use crate::metrics::MetricsRegistry;

/// Worker configuration
#[derive(Debug, Clone)]
pub struct WorkerConfig {
    /// Worker ID
    pub worker_id: String,

    /// Worker pool name
    pub worker_pool: Option<String>,

    /// Working directory
    pub work_dir: std::path::PathBuf,

    /// Result output directory
    pub result_dir: std::path::PathBuf,

    /// Maximum concurrent jobs
    pub max_concurrent_jobs: usize,

    /// Graceful shutdown timeout (seconds)
    pub shutdown_timeout_secs: u64,
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("worker-{}", ulid::Ulid::new()),
            worker_pool: Some("default".to_string()),
            work_dir: std::env::temp_dir().join("guestkit-worker"),
            result_dir: std::path::PathBuf::from("./results"),
            max_concurrent_jobs: 4,
            shutdown_timeout_secs: 30,
        }
    }
}

/// Main worker daemon
pub struct Worker {
    config: WorkerConfig,
    capabilities: Capabilities,
    registry: Arc<HandlerRegistry>,
    executor: Arc<JobExecutor>,
    transport: Box<dyn JobTransport>,
    running: Arc<AtomicBool>,
    metrics: Option<Arc<MetricsRegistry>>,
}

impl Worker {
    /// Create a new worker
    pub fn new(
        config: WorkerConfig,
        capabilities: Capabilities,
        registry: HandlerRegistry,
        transport: Box<dyn JobTransport>,
    ) -> WorkerResult<Self> {
        let registry = Arc::new(registry);
        let result_writer = Arc::new(ResultWriter::new(&config.result_dir));

        let executor = Arc::new(JobExecutor::new(
            &config.worker_id,
            registry.clone(),
            result_writer,
            &config.work_dir,
        ));

        Ok(Self {
            config,
            capabilities,
            registry,
            executor,
            transport,
            running: Arc::new(AtomicBool::new(false)),
            metrics: None,
        })
    }

    /// Set metrics registry
    pub fn with_metrics(&mut self, metrics: Arc<MetricsRegistry>) {
        // Update executor with metrics
        let result_writer = Arc::new(ResultWriter::new(&self.config.result_dir));
        let executor = JobExecutor::new(
            &self.config.worker_id,
            self.registry.clone(),
            result_writer,
            &self.config.work_dir,
        ).with_metrics(Arc::clone(&metrics));

        self.executor = Arc::new(executor);
        self.metrics = Some(metrics);
    }

    /// Start the worker
    pub async fn run(&mut self) -> WorkerResult<()> {
        log::info!("Starting worker {}", self.config.worker_id);
        log::info!("Worker pool: {:?}", self.config.worker_pool);
        log::info!("Max concurrent jobs: {}", self.config.max_concurrent_jobs);
        log::info!("Supported operations: {:?}", self.capabilities.operations);

        self.running.store(true, Ordering::SeqCst);

        // Setup shutdown handler
        let running = self.running.clone();
        tokio::spawn(async move {
            shutdown_signal().await;
            log::info!("Shutdown signal received");
            running.store(false, Ordering::SeqCst);
        });

        // Main event loop
        while self.running.load(Ordering::SeqCst) {
            // Fetch next job
            match self.transport.fetch_job().await {
                Ok(Some(job)) => {
                    log::info!("Received job: {}", job.job_id);
                    let job_id = job.job_id.clone();

                    match self.executor.execute(job).await {
                        Ok(_) => {
                            log::info!("Job {job_id} completed");
                            if let Err(e) = self.transport.ack_job(&job_id).await {
                                log::error!("Failed to ack job {job_id}: {e}");
                            }
                        }
                        Err(e) => {
                            log::error!("Job {job_id} failed: {e}");
                            if let Err(ack_err) = self.transport.nack_job(&job_id, &e.to_string()).await {
                                log::error!("Failed to nack job {job_id}: {ack_err}");
                            }
                        }
                    }
                }
                Ok(None) => {
                    // No jobs available, continue polling
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                }
                Err(e) => {
                    log::error!("Transport error: {}", e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                }
            }
        }

        log::info!("Worker shutting down");

        // TODO: Wait for in-flight jobs to complete (graceful shutdown)

        Ok(())
    }

    /// Get worker capabilities
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Get worker configuration
    pub fn config(&self) -> &WorkerConfig {
        &self.config
    }

    /// Shutdown the worker
    pub fn shutdown(&self) {
        self.running.store(false, Ordering::SeqCst);
    }
}

/// Wait for shutdown signal (SIGTERM, SIGINT, or Ctrl+C)
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transport::file::{FileTransport, FileTransportConfig};
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_worker_creation() {
        let temp_dir = TempDir::new().unwrap();

        let config = WorkerConfig {
            worker_id: "test-worker".to_string(),
            work_dir: temp_dir.path().to_path_buf(),
            result_dir: temp_dir.path().join("results"),
            ..Default::default()
        };

        let caps = Capabilities::new()
            .with_operation("guestkit.inspect");

        let registry = HandlerRegistry::new();

        let transport_config = FileTransportConfig {
            watch_dir: temp_dir.path().join("jobs"),
            done_dir: temp_dir.path().join("done"),
            failed_dir: temp_dir.path().join("failed"),
            poll_interval_secs: 1,
        };

        let transport = FileTransport::new(transport_config).await.unwrap();

        let worker = Worker::new(
            config,
            caps,
            registry,
            Box::new(transport),
        );

        assert!(worker.is_ok());
    }
}
