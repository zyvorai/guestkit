// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Core utilities and types for guestkit

pub mod binary_cache;
pub mod diagnostics;
pub mod error;
pub mod mem_optimize;
pub mod progress;
pub mod retry;
pub mod systemd;
pub mod types;

pub use binary_cache::{BinaryCache, CacheStats, CachedInspection};
pub use diagnostics::DiagnosticError;
pub use error::{Error, Result};
pub use progress::{MultiProgressReporter, ProgressReporter};
pub use retry::{retry_with_backoff, RetryConfig};
pub use systemd::{
    BootTiming, JournalEntry, ServiceDependencies, ServiceInfo, ServiceState, ServiceTiming,
    SystemdAnalyzer,
};
pub use types::*;
