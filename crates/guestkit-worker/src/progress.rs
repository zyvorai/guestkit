// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Progress tracking and reporting

use guestkit_job_spec::ProgressEvent;
use chrono::Utc;
use tokio::sync::mpsc;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use crate::error::WorkerResult;

/// Progress event sender
pub type ProgressSender = mpsc::UnboundedSender<ProgressEvent>;

/// Progress event receiver
pub type ProgressReceiver = mpsc::UnboundedReceiver<ProgressEvent>;

/// Progress tracker for job execution
#[derive(Debug, Clone)]
pub struct ProgressTracker {
    job_id: String,
    sender: ProgressSender,
    sequence: Arc<AtomicU64>,
}

impl ProgressTracker {
    /// Create a new progress tracker
    pub fn new(job_id: impl Into<String>) -> (Self, ProgressReceiver) {
        let (tx, rx) = mpsc::unbounded_channel();

        let tracker = Self {
            job_id: job_id.into(),
            sender: tx,
            sequence: Arc::new(AtomicU64::new(0)),
        };

        (tracker, rx)
    }

    /// Report progress
    pub async fn report(
        &self,
        phase: impl Into<String>,
        progress_percent: Option<u8>,
        message: impl Into<String>,
    ) -> WorkerResult<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        let event = ProgressEvent {
            job_id: self.job_id.clone(),
            timestamp: Utc::now(),
            sequence,
            phase: phase.into(),
            progress_percent,
            message: message.into(),
            details: None,
            observability: None,
        };

        self.sender.send(event).map_err(|e| {
            crate::error::WorkerError::ExecutionError(format!("Failed to send progress: {}", e))
        })?;

        Ok(())
    }

    /// Report with custom details
    pub async fn report_with_details(
        &self,
        phase: impl Into<String>,
        progress_percent: Option<u8>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> WorkerResult<()> {
        let sequence = self.sequence.fetch_add(1, Ordering::SeqCst);

        let mut event = ProgressEvent {
            job_id: self.job_id.clone(),
            timestamp: Utc::now(),
            sequence,
            phase: phase.into(),
            progress_percent,
            message: message.into(),
            details: None,
            observability: None,
        };

        event.details = Some(
            serde_json::from_value(details)
                .unwrap_or_default()
        );

        self.sender.send(event).map_err(|e| {
            crate::error::WorkerError::ExecutionError(format!("Failed to send progress: {}", e))
        })?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_progress_tracker() {
        let (tracker, mut rx) = ProgressTracker::new("job-test-123");

        tracker
            .report("validation", Some(10), "Validating job")
            .await
            .unwrap();

        tracker
            .report("execution", Some(50), "Running operation")
            .await
            .unwrap();

        // Receive events
        let event1 = rx.recv().await.unwrap();
        assert_eq!(event1.phase, "validation");
        assert_eq!(event1.progress_percent, Some(10));

        let event2 = rx.recv().await.unwrap();
        assert_eq!(event2.phase, "execution");
        assert_eq!(event2.progress_percent, Some(50));
    }
}
