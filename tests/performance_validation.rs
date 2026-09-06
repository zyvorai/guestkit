// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance validation framework for guestkit
//!
//! This module provides a framework for validating performance improvements
//! and detecting regressions across different versions.

mod helpers;

use helpers::{MockDisk, MockDiskBuilder, MockDiskType};
use std::path::Path;
use std::time::Duration;
use tempfile::TempDir;

/// Performance test result
#[derive(Debug, Clone)]
pub struct PerfResult {
    pub test_name: String,
    pub duration: Duration,
    pub iterations: usize,
    pub avg_duration: Duration,
    pub min_duration: Duration,
    pub max_duration: Duration,
}

impl PerfResult {
    /// Create a new performance result
    pub fn new(test_name: impl Into<String>, durations: Vec<Duration>) -> Self {
        let iterations = durations.len();
        let total: Duration = durations.iter().sum();
        let avg = total / iterations as u32;
        let min = *durations.iter().min().unwrap();
        let max = *durations.iter().max().unwrap();

        Self {
            test_name: test_name.into(),
            duration: total,
            iterations,
            avg_duration: avg,
            min_duration: min,
            max_duration: max,
        }
    }

    /// Check if this result meets a target duration
    pub fn meets_target(&self, target: Duration) -> bool {
        self.avg_duration <= target
    }

    /// Calculate improvement percentage compared to baseline
    pub fn improvement_vs(&self, baseline: &PerfResult) -> f64 {
        let baseline_ms = baseline.avg_duration.as_millis() as f64;
        let current_ms = self.avg_duration.as_millis() as f64;
        ((baseline_ms - current_ms) / baseline_ms) * 100.0
    }
}

/// Performance test suite
#[derive(Default)]
pub struct PerfTestSuite {
    results: Vec<PerfResult>,
}

impl PerfTestSuite {
    /// Create a new performance test suite
    pub fn new() -> Self {
        Self::default()
    }

    /// Run a performance test with multiple iterations
    pub fn run_test<F>(&mut self, name: impl Into<String>, iterations: usize, mut test_fn: F)
    where
        F: FnMut() -> Duration,
    {
        let mut durations = Vec::with_capacity(iterations);

        for _ in 0..iterations {
            let duration = test_fn();
            durations.push(duration);
        }

        let result = PerfResult::new(name, durations);
        self.results.push(result);
    }

    /// Get all test results
    pub fn results(&self) -> &[PerfResult] {
        &self.results
    }

    /// Generate a performance report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# Performance Validation Report\n\n");
        report.push_str(&format!("Total tests: {}\n\n", self.results.len()));

        report.push_str("## Test Results\n\n");
        report.push_str("| Test Name | Iterations | Avg Duration | Min | Max |\n");
        report.push_str("|-----------|------------|--------------|-----|-----|\n");

        for result in &self.results {
            report.push_str(&format!(
                "| {} | {} | {:?} | {:?} | {:?} |\n",
                result.test_name,
                result.iterations,
                result.avg_duration,
                result.min_duration,
                result.max_duration
            ));
        }

        report
    }

    /// Compare against baseline and detect regressions
    pub fn compare_with_baseline(
        &self,
        baseline: &PerfTestSuite,
        threshold: f64,
    ) -> ComparisonReport {
        let mut improvements = Vec::new();
        let mut regressions = Vec::new();
        let mut unchanged = Vec::new();

        for result in &self.results {
            if let Some(baseline_result) = baseline
                .results
                .iter()
                .find(|r| r.test_name == result.test_name)
            {
                let improvement = result.improvement_vs(baseline_result);

                if improvement > threshold {
                    improvements.push((result.test_name.clone(), improvement));
                } else if improvement < -threshold {
                    regressions.push((result.test_name.clone(), improvement.abs()));
                } else {
                    unchanged.push(result.test_name.clone());
                }
            }
        }

        ComparisonReport {
            improvements,
            regressions,
            unchanged,
            threshold,
        }
    }
}

/// Comparison report between two test suites
#[derive(Debug)]
pub struct ComparisonReport {
    pub improvements: Vec<(String, f64)>,
    pub regressions: Vec<(String, f64)>,
    pub unchanged: Vec<String>,
    pub threshold: f64,
}

impl ComparisonReport {
    /// Check if there are any regressions
    pub fn has_regressions(&self) -> bool {
        !self.regressions.is_empty()
    }

    /// Get overall improvement percentage
    pub fn overall_improvement(&self) -> f64 {
        if self.improvements.is_empty() {
            return 0.0;
        }
        self.improvements.iter().map(|(_, pct)| pct).sum::<f64>() / self.improvements.len() as f64
    }

    /// Generate a comparison report
    pub fn generate_report(&self) -> String {
        let mut report = String::new();
        report.push_str("# Performance Comparison Report\n\n");

        report.push_str(&format!("Threshold: {:.1}%\n\n", self.threshold));

        if !self.improvements.is_empty() {
            report.push_str("## ✅ Improvements\n\n");
            for (test, pct) in &self.improvements {
                report.push_str(&format!("- {}: +{:.2}% faster\n", test, pct));
            }
            report.push('\n');
        }

        if !self.regressions.is_empty() {
            report.push_str("## ❌ Regressions\n\n");
            for (test, pct) in &self.regressions {
                report.push_str(&format!("- {}: -{:.2}% slower\n", test, pct));
            }
            report.push('\n');
        }

        if !self.unchanged.is_empty() {
            report.push_str("## ➡️ Unchanged\n\n");
            for test in &self.unchanged {
                report.push_str(&format!("- {}\n", test));
            }
            report.push('\n');
        }

        report.push_str(&format!(
            "\n**Overall improvement:** {:.2}%\n",
            self.overall_improvement()
        ));

        report
    }
}

/// Synthetic workload generator
pub struct WorkloadGenerator {
    temp_dir: TempDir,
}

impl WorkloadGenerator {
    /// Create a new workload generator
    pub fn new() -> std::io::Result<Self> {
        Ok(Self {
            temp_dir: TempDir::new()?,
        })
    }

    /// Generate a batch of mock disks for testing
    pub fn generate_disk_batch(
        &self,
        count: usize,
        disk_type: MockDiskType,
    ) -> std::io::Result<Vec<MockDisk>> {
        let mut disks = Vec::with_capacity(count);

        for i in 0..count {
            let path = self.temp_dir.path().join(format!("disk_{}.img", i));
            let disk = MockDiskBuilder::new()
                .disk_type(disk_type)
                .hostname(format!("vm-{}", i))
                .build(path)?;
            disks.push(disk);
        }

        Ok(disks)
    }

    /// Generate disks with varying characteristics
    pub fn generate_varied_batch(&self, count: usize) -> std::io::Result<Vec<MockDisk>> {
        let mut disks = Vec::with_capacity(count);
        let types = [
            MockDiskType::Minimal,
            MockDiskType::Small,
            MockDiskType::Medium,
        ];

        for i in 0..count {
            let disk_type = types[i % types.len()];
            let path = self.temp_dir.path().join(format!("varied_{}.img", i));
            let disk = MockDiskBuilder::new()
                .disk_type(disk_type)
                .hostname(format!("varied-vm-{}", i))
                .num_files(50 + (i * 10))
                .num_packages(100 + (i * 20))
                .build(path)?;
            disks.push(disk);
        }

        Ok(disks)
    }

    /// Get the temp directory path
    pub fn temp_dir(&self) -> &Path {
        self.temp_dir.path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_perf_result_creation() {
        let durations = vec![
            Duration::from_millis(100),
            Duration::from_millis(110),
            Duration::from_millis(90),
        ];

        let result = PerfResult::new("test", durations);

        assert_eq!(result.test_name, "test");
        assert_eq!(result.iterations, 3);
        assert_eq!(result.min_duration, Duration::from_millis(90));
        assert_eq!(result.max_duration, Duration::from_millis(110));
    }

    #[test]
    fn test_perf_result_meets_target() {
        let durations = vec![Duration::from_millis(100)];
        let result = PerfResult::new("test", durations);

        assert!(result.meets_target(Duration::from_millis(150)));
        assert!(!result.meets_target(Duration::from_millis(50)));
    }

    #[test]
    fn test_perf_result_improvement() {
        let baseline = PerfResult::new("test", vec![Duration::from_millis(100)]);
        let improved = PerfResult::new("test", vec![Duration::from_millis(80)]);

        let improvement = improved.improvement_vs(&baseline);
        assert!((improvement - 20.0).abs() < 0.1);
    }

    #[test]
    fn test_perf_test_suite_basic() {
        let mut suite = PerfTestSuite::new();

        suite.run_test("test1", 3, || Duration::from_millis(100));
        suite.run_test("test2", 3, || Duration::from_millis(200));

        assert_eq!(suite.results().len(), 2);
        assert_eq!(suite.results()[0].test_name, "test1");
        assert_eq!(suite.results()[1].test_name, "test2");
    }

    #[test]
    fn test_perf_test_suite_report() {
        let mut suite = PerfTestSuite::new();
        suite.run_test("test", 2, || Duration::from_millis(100));

        let report = suite.generate_report();
        assert!(report.contains("Performance Validation Report"));
        assert!(report.contains("test"));
    }

    #[test]
    fn test_comparison_report_improvements() {
        let mut baseline = PerfTestSuite::new();
        baseline.run_test("slow_test", 1, || Duration::from_millis(100));

        let mut improved = PerfTestSuite::new();
        improved.run_test("slow_test", 1, || Duration::from_millis(80));

        let comparison = improved.compare_with_baseline(&baseline, 5.0);

        assert_eq!(comparison.improvements.len(), 1);
        assert_eq!(comparison.regressions.len(), 0);
    }

    #[test]
    fn test_comparison_report_regressions() {
        let mut baseline = PerfTestSuite::new();
        baseline.run_test("fast_test", 1, || Duration::from_millis(100));

        let mut regressed = PerfTestSuite::new();
        regressed.run_test("fast_test", 1, || Duration::from_millis(120));

        let comparison = regressed.compare_with_baseline(&baseline, 5.0);

        assert_eq!(comparison.improvements.len(), 0);
        assert_eq!(comparison.regressions.len(), 1);
        assert!(comparison.has_regressions());
    }

    #[test]
    fn test_workload_generator_creation() {
        let generator = WorkloadGenerator::new().unwrap();
        assert!(generator.temp_dir().exists());
    }

    #[test]
    fn test_workload_generator_disk_batch() {
        let generator = WorkloadGenerator::new().unwrap();
        let disks = generator
            .generate_disk_batch(3, MockDiskType::Minimal)
            .unwrap();

        assert_eq!(disks.len(), 3);
        for disk in &disks {
            assert!(disk.path().exists());
        }
    }

    #[test]
    fn test_workload_generator_varied_batch() {
        let generator = WorkloadGenerator::new().unwrap();
        let disks = generator.generate_varied_batch(5).unwrap();

        assert_eq!(disks.len(), 5);
        // Verify different characteristics
        assert_ne!(disks[0].disk_type(), disks[1].disk_type());
    }

    #[test]
    fn test_comparison_report_generation() {
        let mut baseline = PerfTestSuite::new();
        baseline.run_test("test1", 1, || Duration::from_millis(100));
        baseline.run_test("test2", 1, || Duration::from_millis(200));

        let mut improved = PerfTestSuite::new();
        improved.run_test("test1", 1, || Duration::from_millis(80));
        improved.run_test("test2", 1, || Duration::from_millis(250));

        let comparison = improved.compare_with_baseline(&baseline, 10.0);
        let report = comparison.generate_report();

        assert!(report.contains("Improvements"));
        assert!(report.contains("Regressions"));
        assert!(report.contains("test1"));
        assert!(report.contains("test2"));
    }

    #[test]
    fn test_overall_improvement_calculation() {
        let mut baseline = PerfTestSuite::new();
        baseline.run_test("test1", 1, || Duration::from_millis(100));
        baseline.run_test("test2", 1, || Duration::from_millis(100));

        let mut improved = PerfTestSuite::new();
        improved.run_test("test1", 1, || Duration::from_millis(80)); // 20% improvement
        improved.run_test("test2", 1, || Duration::from_millis(90)); // 10% improvement

        let comparison = improved.compare_with_baseline(&baseline, 5.0);
        let overall = comparison.overall_improvement();

        assert!((overall - 15.0).abs() < 0.1); // Average of 20% and 10%
    }
}
