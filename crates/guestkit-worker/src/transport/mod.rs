// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Job transport layer - pluggable job sources

use async_trait::async_trait;
use guestkit_job_spec::JobDocument;
use crate::error::WorkerResult;

pub mod file;
pub mod http;
pub mod redis;

pub use file::FileTransport;
pub use http::HttpTransport;
pub use redis::{enqueue_job, RedisTransport, RedisTransportConfig, CONSUMER_GROUP, JOBS_STREAM};

/// Job transport trait - defines how jobs are received and acknowledged
#[async_trait]
pub trait JobTransport: Send + Sync {
    /// Fetch next available job
    async fn fetch_job(&mut self) -> WorkerResult<Option<JobDocument>>;

    /// Acknowledge job completion (success)
    async fn ack_job(&mut self, job_id: &str) -> WorkerResult<()>;

    /// Negative acknowledgement (failure/retry)
    async fn nack_job(&mut self, job_id: &str, reason: &str) -> WorkerResult<()>;

    /// Check transport health
    async fn health_check(&self) -> WorkerResult<bool> {
        Ok(true)
    }
}
