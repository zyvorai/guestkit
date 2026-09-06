// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Error types for guestkit

use std::io;
use thiserror::Error;

/// guestkit error types
#[derive(Error, Debug)]
pub enum Error {
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("Conversion error: {0}")]
    Conversion(String),

    #[error("Detection error: {0}")]
    Detection(String),

    #[error("Command execution failed: {0}")]
    CommandFailed(String),

    #[error("Invalid format: {0}")]
    InvalidFormat(String),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Unsupported operation: {0}")]
    Unsupported(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("FFI error: {0}")]
    Ffi(String),

    #[error("Invalid state: {0}")]
    InvalidState(String),

    #[error("Invalid operation: {0}")]
    InvalidOperation(String),

    #[error("Security violation: {0}")]
    SecurityViolation(String),

    #[error("Path validation failed: {0}")]
    PathValidation(String),

    #[error("Input validation failed: {0}")]
    InputValidation(String),

    #[error("Resource limit exceeded: {0}")]
    ResourceLimit(String),

    #[error("Unknown error: {0}")]
    Unknown(String),
}

/// Result type alias for guestkit operations
pub type Result<T> = std::result::Result<T, Error>;
