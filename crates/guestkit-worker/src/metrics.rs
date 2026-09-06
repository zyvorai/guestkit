// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Prometheus metrics collection and export
//!
//! This module provides comprehensive metrics for monitoring worker performance,
//! job execution, and resource utilization.

use prometheus_client::encoding::text::encode;
use prometheus_client::metrics::counter::Counter;
use prometheus_client::metrics::family::Family;
use prometheus_client::metrics::gauge::Gauge;
use prometheus_client::metrics::histogram::{exponential_buckets, Histogram};
use prometheus_client::registry::Registry;
use std::sync::Arc;
use std::sync::Mutex as StdMutex;

/// Labels for job metrics
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct JobLabels {
    /// Operation name (e.g., "guestkit.inspect")
    pub operation: String,
    /// Job status (completed, failed, cancelled, timeout)
    pub status: String,
}

/// Labels for handler metrics
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct HandlerLabels {
    /// Handler name
    pub handler: String,
    /// Execution status (success, error)
    pub status: String,
}

/// Labels for checksum verification metrics
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct ChecksumLabels {
    /// Verification status (success, failure, skipped)
    pub status: String,
}

/// Labels for worker pool metrics
#[derive(Clone, Debug, Hash, PartialEq, Eq, prometheus_client::encoding::EncodeLabelSet)]
pub struct WorkerLabels {
    /// Worker ID
    pub worker_id: String,
    /// Worker pool name
    pub pool: String,
}

/// Prometheus metrics registry for the worker
#[derive(Debug)]
pub struct MetricsRegistry {
    /// Prometheus registry
    registry: Arc<StdMutex<Registry>>,

    // Job metrics
    /// Total number of jobs processed
    pub jobs_total: Family<JobLabels, Counter>,
    /// Job execution duration in seconds
    pub jobs_duration_seconds: Family<JobLabels, Histogram>,
    /// Currently active jobs
    pub active_jobs: Gauge,
    /// Queue depth (pending jobs)
    pub queue_depth: Gauge,

    // Handler metrics
    /// Total handler executions
    pub handler_executions_total: Family<HandlerLabels, Counter>,
    /// Handler execution duration
    pub handler_duration_seconds: Family<HandlerLabels, Histogram>,

    // Checksum verification metrics
    /// Checksum verification attempts
    pub checksum_verifications_total: Family<ChecksumLabels, Counter>,

    // Resource metrics
    /// Disk bytes read
    pub disk_read_bytes_total: Counter,
    /// Disk bytes written
    pub disk_write_bytes_total: Counter,
}

impl MetricsRegistry {
    /// Create a new metrics registry
    pub fn new() -> Self {
        let mut registry = Registry::default();

        // Job metrics
        let jobs_total = Family::<JobLabels, Counter>::default();
        registry.register(
            "guestkit_worker_jobs_total",
            "Total number of jobs processed",
            jobs_total.clone(),
        );

        let jobs_duration_seconds = Family::<JobLabels, Histogram>::new_with_constructor(|| {
            // Buckets: 1s, 2s, 5s, 10s, 30s, 1m, 2m, 5m, 10m, 30m, 1h
            Histogram::new(exponential_buckets(1.0, 2.0, 12))
        });
        registry.register(
            "guestkit_worker_jobs_duration_seconds",
            "Job execution duration in seconds",
            jobs_duration_seconds.clone(),
        );

        let active_jobs = Gauge::default();
        registry.register(
            "guestkit_worker_active_jobs",
            "Currently active jobs",
            active_jobs.clone(),
        );

        let queue_depth = Gauge::default();
        registry.register(
            "guestkit_worker_queue_depth",
            "Queue depth (pending jobs)",
            queue_depth.clone(),
        );

        // Handler metrics
        let handler_executions_total = Family::<HandlerLabels, Counter>::default();
        registry.register(
            "guestkit_handler_executions_total",
            "Total handler executions",
            handler_executions_total.clone(),
        );

        let handler_duration_seconds = Family::<HandlerLabels, Histogram>::new_with_constructor(|| {
            Histogram::new(exponential_buckets(1.0, 2.0, 12))
        });
        registry.register(
            "guestkit_handler_duration_seconds",
            "Handler execution duration in seconds",
            handler_duration_seconds.clone(),
        );

        // Checksum verification metrics
        let checksum_verifications_total = Family::<ChecksumLabels, Counter>::default();
        registry.register(
            "guestkit_checksum_verifications_total",
            "Checksum verification attempts",
            checksum_verifications_total.clone(),
        );

        // Resource metrics
        let disk_read_bytes_total = Counter::default();
        registry.register(
            "guestkit_worker_disk_read_bytes_total",
            "Total disk bytes read",
            disk_read_bytes_total.clone(),
        );

        let disk_write_bytes_total = Counter::default();
        registry.register(
            "guestkit_worker_disk_write_bytes_total",
            "Total disk bytes written",
            disk_write_bytes_total.clone(),
        );

        Self {
            registry: Arc::new(StdMutex::new(registry)),
            jobs_total,
            jobs_duration_seconds,
            active_jobs,
            queue_depth,
            handler_executions_total,
            handler_duration_seconds,
            checksum_verifications_total,
            disk_read_bytes_total,
            disk_write_bytes_total,
        }
    }

    /// Record a job completion
    pub fn record_job_completion(
        &self,
        operation: &str,
        status: &str,
        duration_seconds: f64,
    ) {
        let labels = JobLabels {
            operation: operation.to_string(),
            status: status.to_string(),
        };

        self.jobs_total.get_or_create(&labels).inc();
        self.jobs_duration_seconds.get_or_create(&labels).observe(duration_seconds);
    }

    /// Record handler execution
    pub fn record_handler_execution(
        &self,
        handler: &str,
        status: &str,
        duration_seconds: f64,
    ) {
        let labels = HandlerLabels {
            handler: handler.to_string(),
            status: status.to_string(),
        };

        self.handler_executions_total.get_or_create(&labels).inc();
        self.handler_duration_seconds.get_or_create(&labels).observe(duration_seconds);
    }

    /// Record checksum verification
    pub fn record_checksum_verification(&self, status: &str) {
        let labels = ChecksumLabels {
            status: status.to_string(),
        };
        self.checksum_verifications_total.get_or_create(&labels).inc();
    }

    /// Increment active jobs
    pub fn inc_active_jobs(&self) {
        self.active_jobs.inc();
    }

    /// Decrement active jobs
    pub fn dec_active_jobs(&self) {
        self.active_jobs.dec();
    }

    /// Set queue depth
    pub fn set_queue_depth(&self, depth: i64) {
        self.queue_depth.set(depth);
    }

    /// Record disk I/O
    pub fn record_disk_io(&self, read_bytes: u64, write_bytes: u64) {
        if read_bytes > 0 {
            self.disk_read_bytes_total.inc_by(read_bytes);
        }
        if write_bytes > 0 {
            self.disk_write_bytes_total.inc_by(write_bytes);
        }
    }

    /// Encode metrics in Prometheus text format
    pub fn encode(&self) -> String {
        let mut buffer = String::new();
        let registry = self.registry.lock().unwrap();
        encode(&mut buffer, &registry).unwrap();
        buffer
    }
}

impl Default for MetricsRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl Clone for MetricsRegistry {
    fn clone(&self) -> Self {
        Self {
            registry: Arc::clone(&self.registry),
            jobs_total: self.jobs_total.clone(),
            jobs_duration_seconds: self.jobs_duration_seconds.clone(),
            active_jobs: self.active_jobs.clone(),
            queue_depth: self.queue_depth.clone(),
            handler_executions_total: self.handler_executions_total.clone(),
            handler_duration_seconds: self.handler_duration_seconds.clone(),
            checksum_verifications_total: self.checksum_verifications_total.clone(),
            disk_read_bytes_total: self.disk_read_bytes_total.clone(),
            disk_write_bytes_total: self.disk_write_bytes_total.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_metrics_registry_creation() {
        let registry = MetricsRegistry::new();
        let encoded = registry.encode();

        // Verify metrics are registered
        assert!(encoded.contains("guestkit_worker_jobs_total"));
        assert!(encoded.contains("guestkit_worker_active_jobs"));
    }

    #[test]
    fn test_record_job_completion() {
        let registry = MetricsRegistry::new();

        registry.record_job_completion("guestkit.inspect", "completed", 42.5);
        registry.record_job_completion("guestkit.inspect", "completed", 38.2);
        registry.record_job_completion("guestkit.inspect", "failed", 10.0);

        let encoded = registry.encode();

        // Should have recorded the jobs
        assert!(encoded.contains("guestkit.inspect"));
        assert!(encoded.contains("completed"));
        assert!(encoded.contains("failed"));
    }

    #[test]
    fn test_active_jobs_tracking() {
        let registry = MetricsRegistry::new();

        registry.inc_active_jobs();
        registry.inc_active_jobs();
        registry.dec_active_jobs();

        let encoded = registry.encode();
        assert!(encoded.contains("guestkit_worker_active_jobs"));
    }

    #[test]
    fn test_checksum_verification_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_checksum_verification("success");
        registry.record_checksum_verification("success");
        registry.record_checksum_verification("failure");

        let encoded = registry.encode();
        assert!(encoded.contains("guestkit_checksum_verifications_total"));
    }

    #[test]
    fn test_handler_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_handler_execution("inspect", "success", 45.0);
        registry.record_handler_execution("profile", "error", 5.0);

        let encoded = registry.encode();
        assert!(encoded.contains("guestkit_handler_executions_total"));
        assert!(encoded.contains("inspect"));
        assert!(encoded.contains("profile"));
    }

    #[test]
    fn test_disk_io_metrics() {
        let registry = MetricsRegistry::new();

        registry.record_disk_io(1024, 512);
        registry.record_disk_io(2048, 0);

        let encoded = registry.encode();
        assert!(encoded.contains("guestkit_worker_disk_read_bytes_total"));
        assert!(encoded.contains("guestkit_worker_disk_write_bytes_total"));
    }
}
