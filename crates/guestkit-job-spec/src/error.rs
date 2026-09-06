// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Error types for job specification

use thiserror::Error;

/// Job-related errors
#[derive(Error, Debug)]
pub enum JobError {
    #[error("Invalid job specification: {0}")]
    InvalidSpec(String),

    #[error("Validation error: {0}")]
    ValidationError(String),

    #[error("Unsupported protocol version: {0}")]
    UnsupportedVersion(String),

    #[error("Unsupported operation: {0}")]
    UnsupportedOperation(String),

    #[error("Missing required field: {0}")]
    MissingField(String),

    #[error("Invalid field value: {field} - {reason}")]
    InvalidField { field: String, reason: String },

    #[error("Serialization error: {0}")]
    SerializationError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("Invalid ULID: {0}")]
    InvalidUlid(String),

    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(String),

    #[error("Capability mismatch: required {required:?}, available {available:?}")]
    CapabilityMismatch {
        required: Vec<String>,
        available: Vec<String>,
    },
}

/// Result type alias for job operations
pub type JobResult<T> = Result<T, JobError>;
