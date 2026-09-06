// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Error types for worker operations

use thiserror::Error;

/// Worker-related errors
#[derive(Error, Debug)]
pub enum WorkerError {
    #[error("Job error: {0}")]
    JobError(#[from] guestkit_job_spec::JobError),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Execution error: {0}")]
    ExecutionError(String),

    #[error("Handler not found for operation: {0}")]
    HandlerNotFound(String),

    #[error("Capability mismatch: {0}")]
    CapabilityMismatch(String),

    #[error("Invalid state transition: {current} -> {target}")]
    InvalidStateTransition { current: String, target: String },

    #[error("Job timeout after {seconds} seconds")]
    Timeout { seconds: u64 },

    #[error("Worker shutdown requested")]
    ShutdownRequested,

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("File watch error: {0}")]
    WatchError(#[from] notify::Error),

    #[error("Job already exists with idempotency key: {0}")]
    DuplicateIdempotencyKey(String),

    #[error("Invalid configuration: {0}")]
    InvalidConfig(String),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Result type alias for worker operations
pub type WorkerResult<T> = Result<T, WorkerError>;
