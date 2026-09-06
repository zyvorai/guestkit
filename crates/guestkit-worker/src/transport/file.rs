// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! File-based job transport
//!
//! Watches a directory for new job files and processes them.

use async_trait::async_trait;
use guestkit_job_spec::JobDocument;
use notify::{Watcher, RecursiveMode, Event, EventKind};
use notify::event::CreateKind;
use tokio::sync::mpsc;
use tokio::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;
use crate::error::{WorkerError, WorkerResult};
use super::JobTransport;

/// File-based transport configuration
#[derive(Debug, Clone)]
pub struct FileTransportConfig {
    /// Directory to watch for new jobs
    pub watch_dir: PathBuf,

    /// Directory to move completed jobs
    pub done_dir: PathBuf,

    /// Directory to move failed jobs
    pub failed_dir: PathBuf,

    /// Poll interval if filesystem watching is unavailable
    pub poll_interval_secs: u64,
}

impl Default for FileTransportConfig {
    fn default() -> Self {
        Self {
            watch_dir: PathBuf::from("./jobs"),
            done_dir: PathBuf::from("./jobs/done"),
            failed_dir: PathBuf::from("./jobs/failed"),
            poll_interval_secs: 5,
        }
    }
}

/// File-based job transport
pub struct FileTransport {
    config: Arc<FileTransportConfig>,
    job_queue: mpsc::UnboundedReceiver<PathBuf>,
    _watcher: Option<notify::RecommendedWatcher>,
}

impl FileTransport {
    /// Create a new file transport
    pub async fn new(config: FileTransportConfig) -> WorkerResult<Self> {
        let config = Arc::new(config);

        // Create directories
        fs::create_dir_all(&config.watch_dir).await?;
        fs::create_dir_all(&config.done_dir).await?;
        fs::create_dir_all(&config.failed_dir).await?;

        // Setup file watcher
        let (tx, rx) = mpsc::unbounded_channel();
        let watch_dir = config.watch_dir.clone();

        // Clone tx for watcher closure
        let tx_clone = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            if let Ok(event) = res {
                if matches!(event.kind, EventKind::Create(CreateKind::File)) {
                    for path in event.paths {
                        if path.extension().and_then(|s| s.to_str()) == Some("json") {
                            let _ = tx_clone.send(path);
                        }
                    }
                }
            }
        })?;

        watcher.watch(&watch_dir, RecursiveMode::NonRecursive)?;

        // Scan for existing files
        let mut entries = fs::read_dir(&config.watch_dir).await?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                let _ = tx.send(path);
            }
        }

        Ok(Self {
            config,
            job_queue: rx,
            _watcher: Some(watcher),
        })
    }

    /// Move job file to done directory
    async fn move_to_done(&self, job_id: &str) -> WorkerResult<()> {
        let source = self.config.watch_dir.join(format!("{}.json", job_id));
        let dest = self.config.done_dir.join(format!("{}.json", job_id));

        if source.exists() {
            fs::rename(&source, &dest).await?;
            log::debug!("Moved job {} to done directory", job_id);
        }

        Ok(())
    }

    /// Move job file to failed directory
    async fn move_to_failed(&self, job_id: &str, reason: &str) -> WorkerResult<()> {
        let source = self.config.watch_dir.join(format!("{}.json", job_id));
        let dest = self.config.failed_dir.join(format!("{}.json", job_id));

        if source.exists() {
            fs::rename(&source, &dest).await?;

            // Write failure reason
            let reason_file = self.config.failed_dir.join(format!("{}.reason.txt", job_id));
            fs::write(&reason_file, reason).await?;

            log::debug!("Moved job {} to failed directory: {}", job_id, reason);
        }

        Ok(())
    }

    /// Read and parse job file
    async fn read_job(&self, path: &Path) -> WorkerResult<JobDocument> {
        let contents = fs::read_to_string(path).await?;
        let job: JobDocument = serde_json::from_str(&contents)?;
        Ok(job)
    }
}

#[async_trait]
impl JobTransport for FileTransport {
    async fn fetch_job(&mut self) -> WorkerResult<Option<JobDocument>> {
        // Try to receive a job path from the queue
        match tokio::time::timeout(
            Duration::from_secs(self.config.poll_interval_secs),
            self.job_queue.recv()
        ).await {
            Ok(Some(path)) => {
                log::info!("Received job file: {}", path.display());
                let job = self.read_job(&path).await?;
                Ok(Some(job))
            }
            Ok(None) => {
                // Channel closed
                Ok(None)
            }
            Err(_) => {
                // Timeout - no jobs available
                Ok(None)
            }
        }
    }

    async fn ack_job(&mut self, job_id: &str) -> WorkerResult<()> {
        self.move_to_done(job_id).await
    }

    async fn nack_job(&mut self, job_id: &str, reason: &str) -> WorkerResult<()> {
        self.move_to_failed(job_id, reason).await
    }

    async fn health_check(&self) -> WorkerResult<bool> {
        // Check if directories are accessible
        Ok(self.config.watch_dir.exists()
            && self.config.done_dir.exists()
            && self.config.failed_dir.exists())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use guestkit_job_spec::builder::JobBuilder;

    #[tokio::test]
    async fn test_file_transport() {
        let temp_dir = TempDir::new().unwrap();

        let config = FileTransportConfig {
            watch_dir: temp_dir.path().join("jobs"),
            done_dir: temp_dir.path().join("done"),
            failed_dir: temp_dir.path().join("failed"),
            poll_interval_secs: 1,
        };

        let mut transport = FileTransport::new(config.clone()).await.unwrap();

        // Create a test job file
        let job = JobBuilder::new()
            .job_id("test-job-123")
            .operation("guestkit.inspect")
            .payload("guestkit.inspect.v1", serde_json::json!({}))
            .build()
            .unwrap();

        let job_file = config.watch_dir.join("test-job-123.json");
        fs::write(&job_file, serde_json::to_string_pretty(&job).unwrap())
            .await
            .unwrap();

        // Wait for file watcher to pick it up
        tokio::time::sleep(Duration::from_millis(500)).await;

        // Fetch the job
        let fetched = transport.fetch_job().await.unwrap();
        assert!(fetched.is_some());
        assert_eq!(fetched.unwrap().job_id, "test-job-123");

        // Acknowledge
        transport.ack_job("test-job-123").await.unwrap();

        // Verify moved to done
        assert!(config.done_dir.join("test-job-123.json").exists());
        assert!(!job_file.exists());
    }
}
