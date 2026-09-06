// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Parallel processing for batch VM inspection operations
//!
//! This module provides parallel batch inspection capabilities using rayon,
//! enabling efficient processing of multiple VM disks concurrently.
//!
//! Note: Currently unused but available for future parallel inspection features.
#![allow(dead_code)]
//!
//! # Performance
//!
//! - Sequential: Process one disk at a time
//! - Parallel: Process N disks concurrently (where N = number of CPU cores)
//! - Expected speedup: ~4x on 4-core systems, ~8x on 8-core systems
//!
//! # Examples
//!
//! ```no_run
//! use guestkit::cli::parallel::{ParallelInspector, InspectionConfig};
//!
//! let disks = vec!["vm1.qcow2", "vm2.qcow2", "vm3.qcow2"];
//! let config = InspectionConfig::default();
//!
//! let results = ParallelInspector::new(config)
//!     .inspect_batch(&disks)?;
//!
//! for result in results {
//!     println!("Inspected: {:?}", result);
//! }
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

use crate::core::{BinaryCache, CachedInspection, Error, Result};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// Configuration for parallel inspection operations
#[derive(Debug, Clone)]
pub struct InspectionConfig {
    /// Maximum number of parallel workers (0 = use all CPU cores)
    pub max_workers: usize,

    /// Enable caching for inspection results
    pub enable_cache: bool,

    /// Maximum time to wait for each inspection (seconds)
    pub timeout_secs: u64,

    /// Continue processing on error (don't fail fast)
    pub continue_on_error: bool,

    /// Enable verbose progress reporting
    pub verbose: bool,
}

impl Default for InspectionConfig {
    fn default() -> Self {
        Self {
            max_workers: 0, // Use all cores
            enable_cache: true,
            timeout_secs: 300, // 5 minutes per disk
            continue_on_error: true,
            verbose: false,
        }
    }
}

/// Result of a single inspection operation
#[derive(Debug, Clone)]
pub struct InspectionResult {
    /// Path to the inspected disk
    pub disk_path: PathBuf,

    /// Whether inspection was successful
    pub success: bool,

    /// Error message if inspection failed
    pub error: Option<String>,

    /// Time taken for inspection
    pub duration: Duration,

    /// OS type detected (if successful)
    pub os_type: Option<String>,

    /// OS product name (if successful)
    pub product_name: Option<String>,

    /// Whether result came from cache
    pub from_cache: bool,
}

impl InspectionResult {
    /// Create a successful result
    pub fn success(
        disk_path: PathBuf,
        duration: Duration,
        os_type: String,
        product_name: String,
        from_cache: bool,
    ) -> Self {
        Self {
            disk_path,
            success: true,
            error: None,
            duration,
            os_type: Some(os_type),
            product_name: Some(product_name),
            from_cache,
        }
    }

    /// Create a failed result
    pub fn failure(disk_path: PathBuf, duration: Duration, error: String) -> Self {
        Self {
            disk_path,
            success: false,
            error: Some(error),
            duration,
            os_type: None,
            product_name: None,
            from_cache: false,
        }
    }
}

/// Parallel batch inspector for VM disks
pub struct ParallelInspector {
    config: InspectionConfig,
}

impl ParallelInspector {
    /// Create a new parallel inspector with the given configuration
    pub fn new(config: InspectionConfig) -> Self {
        // Configure rayon thread pool
        if config.max_workers > 0 {
            if let Err(e) = rayon::ThreadPoolBuilder::new()
                .num_threads(config.max_workers)
                .build_global()
            {
                eprintln!("Warning: Failed to configure thread pool: {}", e);
            }
        }

        Self { config }
    }

    /// Inspect multiple disks in parallel
    ///
    /// This method processes all disks concurrently using rayon's parallel
    /// iterator. Results are returned in the same order as the input disks.
    ///
    /// # Arguments
    ///
    /// * `disk_paths` - Slice of disk paths to inspect
    ///
    /// # Returns
    ///
    /// Vector of inspection results, one per disk, in the same order as input.
    ///
    /// # Errors
    ///
    /// Returns error only if the entire operation fails. Individual disk
    /// inspection failures are captured in InspectionResult.
    pub fn inspect_batch<P: AsRef<Path> + Send + Sync>(
        &self,
        disk_paths: &[P],
    ) -> Result<Vec<InspectionResult>> {
        let start = Instant::now();

        if self.config.verbose {
            println!(
                "🚀 Starting parallel inspection of {} disks",
                disk_paths.len()
            );
            println!("👷 Workers: {}", self.num_workers());
        }

        // Convert paths to PathBuf for parallel processing
        let paths: Vec<PathBuf> = disk_paths
            .iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();

        // Process disks in parallel
        let results: Vec<InspectionResult> = paths
            .par_iter()
            .map(|path| self.inspect_single(path))
            .collect();

        let total_duration = start.elapsed();

        if self.config.verbose {
            self.print_summary(&results, total_duration);
        }

        Ok(results)
    }

    /// Inspect a single disk (called by parallel workers)
    fn inspect_single(&self, disk_path: &Path) -> InspectionResult {
        let start = Instant::now();

        if self.config.verbose {
            println!("🔍 Inspecting: {}", disk_path.display());
        }

        // Inspect with cache integration
        match self.inspect_with_cache(disk_path) {
            Ok((os_type, product_name, from_cache)) => {
                let duration = start.elapsed();
                InspectionResult::success(
                    disk_path.to_path_buf(),
                    duration,
                    os_type,
                    product_name,
                    from_cache,
                )
            }
            Err(e) => {
                let duration = start.elapsed();
                InspectionResult::failure(disk_path.to_path_buf(), duration, e.to_string())
            }
        }
    }

    /// Inspect disk with cache integration
    fn inspect_with_cache(&self, disk_path: &Path) -> Result<(String, String, bool)> {
        // Validate disk exists
        if !disk_path.exists() {
            return Err(Error::NotFound(format!(
                "Disk not found: {}",
                disk_path.display()
            )));
        }

        // Generate cache key from disk path and modification time
        let cache_key = self.generate_cache_key(disk_path)?;

        // Check cache first if enabled
        if self.config.enable_cache {
            if let Ok(cache) = BinaryCache::new() {
                if let Ok(cached_data) = cache.load(&cache_key) {
                    if self.config.verbose {
                        println!("✅ Cache hit: {}", disk_path.display());
                    }
                    return Ok((
                        cached_data.os_info.os_type,
                        cached_data.os_info.product_name,
                        true, // from_cache = true
                    ));
                }
            }
        }

        // Cache miss - perform actual inspection
        if self.config.verbose {
            println!("🔍 Cache miss - inspecting: {}", disk_path.display());
        }

        let (os_type, product_name) = self.perform_inspection(disk_path)?;

        // Save to cache if enabled
        if self.config.enable_cache {
            if let Ok(cache) = BinaryCache::new() {
                let mut cached_inspection = CachedInspection::new(cache_key.clone());
                cached_inspection.os_info.os_type = os_type.clone();
                cached_inspection.os_info.product_name = product_name.clone();
                let _ = cache.save(&cache_key, &cached_inspection);
            }
        }

        Ok((os_type, product_name, false)) // from_cache = false
    }

    /// Generate cache key from disk path
    ///
    /// Uses SHA256 hash of: file path + file size + modification time
    /// This ensures cache invalidation when disk changes.
    fn generate_cache_key(&self, disk_path: &Path) -> Result<String> {
        let metadata = fs::metadata(disk_path)?;

        let mut hasher = Sha256::new();

        // Hash file path
        hasher.update(disk_path.to_string_lossy().as_bytes());

        // Hash file size
        hasher.update(metadata.len().to_string().as_bytes());

        // Hash modification time (if available)
        if let Ok(modified) = metadata.modified() {
            if let Ok(duration) = modified.duration_since(std::time::UNIX_EPOCH) {
                hasher.update(duration.as_secs().to_string().as_bytes());
            }
        }

        let hash = hasher.finalize();
        Ok(format!("{:x}", hash))
    }

    /// Perform actual disk inspection using Guestfs
    fn perform_inspection(&self, disk_path: &Path) -> Result<(String, String)> {
        let mut g = crate::guestfs::Guestfs::new()
            .map_err(|e| Error::CommandFailed(format!("Failed to create guestfs handle: {}", e)))?;

        g.add_drive_ro(disk_path).map_err(|e| {
            Error::CommandFailed(format!(
                "Failed to add drive {}: {}",
                disk_path.display(),
                e
            ))
        })?;

        g.launch()
            .map_err(|e| Error::CommandFailed(format!("Failed to launch guestfs: {}", e)))?;

        let roots = g.inspect_os().unwrap_or_default();
        if roots.is_empty() {
            let _ = g.shutdown();
            return Ok(("unknown".to_string(), "Unknown".to_string()));
        }

        let root = &roots[0];
        let os_type = g
            .inspect_get_type(root)
            .unwrap_or_else(|_| "unknown".to_string());
        let product_name = g
            .inspect_get_product_name(root)
            .unwrap_or_else(|_| "Unknown".to_string());

        let _ = g.shutdown();

        Ok((os_type, product_name))
    }

    /// Get the number of worker threads
    pub fn num_workers(&self) -> usize {
        if self.config.max_workers > 0 {
            self.config.max_workers
        } else {
            rayon::current_num_threads()
        }
    }

    /// Print summary of batch inspection results
    fn print_summary(&self, results: &[InspectionResult], total_duration: Duration) {
        let successful = results.iter().filter(|r| r.success).count();
        let failed = results.iter().filter(|r| !r.success).count();
        let from_cache = results.iter().filter(|r| r.from_cache).count();

        println!("\n📊 Batch Inspection Summary");
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━");
        println!("Total disks:      {}", results.len());
        println!("✅ Successful:    {}", successful);
        println!("❌ Failed:        {}", failed);
        println!("💾 From cache:    {}", from_cache);
        println!("⏱️  Total time:    {:?}", total_duration);
        let avg = if results.is_empty() {
            Duration::ZERO
        } else {
            total_duration / results.len() as u32
        };
        println!("⚡ Avg per disk:  {:?}", avg);
        println!("━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━\n");
    }
}

/// Batch inspection with progress tracking
pub struct ProgressiveInspector {
    config: InspectionConfig,
    progress: Arc<Mutex<usize>>,
}

impl ProgressiveInspector {
    /// Create a new progressive inspector
    pub fn new(config: InspectionConfig) -> Self {
        Self {
            config,
            progress: Arc::new(Mutex::new(0)),
        }
    }

    /// Inspect batch with progress updates
    pub fn inspect_batch_with_progress<P, F>(
        &self,
        disk_paths: &[P],
        progress_callback: F,
    ) -> Result<Vec<InspectionResult>>
    where
        P: AsRef<Path> + Send + Sync,
        F: FnMut(usize, usize) + Send + Sync,
    {
        let total = disk_paths.len();
        let paths: Vec<PathBuf> = disk_paths
            .iter()
            .map(|p| p.as_ref().to_path_buf())
            .collect();

        let progress = Arc::clone(&self.progress);
        let callback = Arc::new(Mutex::new(progress_callback));

        let results: Vec<InspectionResult> = paths
            .par_iter()
            .map(|path| {
                // Perform inspection
                let inspector = ParallelInspector::new(self.config.clone());
                let result = inspector.inspect_single(path);

                // Update progress
                let mut count = progress.lock().unwrap_or_else(|e| e.into_inner());
                *count += 1;
                let current = *count;
                drop(count);

                // Call progress callback
                let mut cb = callback.lock().unwrap_or_else(|e| e.into_inner());
                cb(current, total);
                drop(cb);

                result
            })
            .collect();

        Ok(results)
    }
}

impl Default for ParallelInspector {
    fn default() -> Self {
        Self::new(InspectionConfig::default())
    }
}

/// Helper function for simple batch inspection
pub fn inspect_batch<P: AsRef<Path> + Send + Sync>(
    disk_paths: &[P],
) -> Result<Vec<InspectionResult>> {
    ParallelInspector::default().inspect_batch(disk_paths)
}

/// Helper function for batch inspection with custom workers
pub fn inspect_batch_with_workers<P: AsRef<Path> + Send + Sync>(
    disk_paths: &[P],
    num_workers: usize,
) -> Result<Vec<InspectionResult>> {
    let config = InspectionConfig {
        max_workers: num_workers,
        ..Default::default()
    };
    ParallelInspector::new(config).inspect_batch(disk_paths)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn test_inspection_config_default() {
        let config = InspectionConfig::default();
        assert_eq!(config.max_workers, 0);
        assert!(config.enable_cache);
        assert_eq!(config.timeout_secs, 300);
        assert!(config.continue_on_error);
        assert!(!config.verbose);
    }

    #[test]
    fn test_inspection_result_success() {
        let result = InspectionResult::success(
            PathBuf::from("test.qcow2"),
            Duration::from_secs(5),
            "linux".to_string(),
            "Ubuntu 22.04".to_string(),
            false,
        );

        assert!(result.success);
        assert!(result.error.is_none());
        assert_eq!(result.os_type, Some("linux".to_string()));
        assert!(!result.from_cache);
    }

    #[test]
    fn test_inspection_result_failure() {
        let result = InspectionResult::failure(
            PathBuf::from("test.qcow2"),
            Duration::from_secs(1),
            "Disk not found".to_string(),
        );

        assert!(!result.success);
        assert!(result.error.is_some());
        assert!(result.os_type.is_none());
    }

    #[test]
    fn test_parallel_inspector_creation() {
        let config = InspectionConfig::default();
        let inspector = ParallelInspector::new(config);
        assert!(inspector.num_workers() > 0);
    }

    #[test]
    fn test_parallel_inspector_workers() {
        let config = InspectionConfig {
            max_workers: 4,
            ..Default::default()
        };
        let inspector = ParallelInspector::new(config);
        assert_eq!(inspector.num_workers(), 4);
    }

    #[test]
    fn test_inspect_batch_nonexistent() {
        let inspector = ParallelInspector::default();
        let disks = vec!["/nonexistent/disk1.qcow2", "/nonexistent/disk2.qcow2"];

        let results = inspector.inspect_batch(&disks).unwrap();
        assert_eq!(results.len(), 2);
        assert!(!results[0].success);
        assert!(!results[1].success);
    }

    #[test]
    fn test_inspect_batch_with_temp_files() {
        let temp_dir = TempDir::new().unwrap();
        let disk1 = temp_dir.path().join("disk1.img");
        let disk2 = temp_dir.path().join("disk2.img");

        fs::write(&disk1, b"fake disk data").unwrap();
        fs::write(&disk2, b"fake disk data").unwrap();

        let inspector = ParallelInspector::default();
        let disks = vec![&disk1, &disk2];

        let results = inspector.inspect_batch(&disks).unwrap();
        assert_eq!(results.len(), 2);
        // Non-VM files will fail guestfs inspection
        assert!(!results[0].success);
        assert!(!results[1].success);
    }

    #[test]
    fn test_helper_inspect_batch() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("disk.img");
        fs::write(&disk, b"fake").unwrap();

        let results = inspect_batch(&[&disk]).unwrap();
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn test_helper_inspect_batch_with_workers() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("disk.img");
        fs::write(&disk, b"fake").unwrap();

        let results = inspect_batch_with_workers(&[&disk], 2).unwrap();
        assert_eq!(results.len(), 1);
    }

    // ========== Cache Key Generation Tests ==========

    #[test]
    fn test_cache_key_generation() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("test.img");
        fs::write(&disk, b"test data").unwrap();

        let inspector = ParallelInspector::default();
        let key = inspector.generate_cache_key(&disk).unwrap();

        // Should be a 64-character hex string (SHA256)
        assert_eq!(key.len(), 64);
        assert!(key.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_cache_key_deterministic() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("test.img");
        fs::write(&disk, b"test data").unwrap();

        let inspector = ParallelInspector::default();
        let key1 = inspector.generate_cache_key(&disk).unwrap();
        let key2 = inspector.generate_cache_key(&disk).unwrap();

        // Same file should produce same cache key
        assert_eq!(key1, key2);
    }

    #[test]
    fn test_cache_key_different_files() {
        let temp_dir = TempDir::new().unwrap();
        let disk1 = temp_dir.path().join("test1.img");
        let disk2 = temp_dir.path().join("test2.img");

        fs::write(&disk1, b"data1").unwrap();
        fs::write(&disk2, b"data2").unwrap();

        let inspector = ParallelInspector::default();
        let key1 = inspector.generate_cache_key(&disk1).unwrap();
        let key2 = inspector.generate_cache_key(&disk2).unwrap();

        // Different files should produce different keys
        assert_ne!(key1, key2);
    }

    #[test]
    fn test_cache_key_invalidation_on_modification() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("test.img");

        fs::write(&disk, b"original").unwrap();
        let inspector = ParallelInspector::default();
        let key1 = inspector.generate_cache_key(&disk).unwrap();

        // Modify file with different size so the hash changes
        // (mtime granularity may be too coarse for same-size writes)
        fs::write(&disk, b"modified content with different length").unwrap();
        let key2 = inspector.generate_cache_key(&disk).unwrap();

        // Modified file should have different key (different size)
        assert_ne!(key1, key2);
    }

    // ========== Worker Configuration Tests ==========

    #[test]
    fn test_zero_workers_uses_all_cores() {
        let config = InspectionConfig {
            max_workers: 0,
            ..Default::default()
        };
        let inspector = ParallelInspector::new(config);

        // Should use all available cores
        assert!(inspector.num_workers() > 0);
        assert!(inspector.num_workers() <= num_cpus::get());
    }

    #[test]
    fn test_single_worker() {
        let config = InspectionConfig {
            max_workers: 1,
            ..Default::default()
        };
        let inspector = ParallelInspector::new(config);
        assert_eq!(inspector.num_workers(), 1);
    }

    #[test]
    fn test_many_workers() {
        let config = InspectionConfig {
            max_workers: 16,
            ..Default::default()
        };
        let inspector = ParallelInspector::new(config);
        assert_eq!(inspector.num_workers(), 16);
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_inspect_empty_batch() {
        let inspector = ParallelInspector::default();
        let disks: Vec<PathBuf> = vec![];

        let results = inspector.inspect_batch(&disks).unwrap();
        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_inspect_single_disk() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("single.img");
        fs::write(&disk, b"data").unwrap();

        let inspector = ParallelInspector::default();
        let results = inspector.inspect_batch(&[&disk]).unwrap();

        assert_eq!(results.len(), 1);
        // Non-VM dummy files fail guestfs inspection
        assert!(!results[0].success);
    }

    #[test]
    fn test_inspect_mixed_success_failure() {
        let temp_dir = TempDir::new().unwrap();
        let disk1 = temp_dir.path().join("exists.img");
        let disk2 = PathBuf::from("/nonexistent/missing.img");
        let disk3 = temp_dir.path().join("exists2.img");

        fs::write(&disk1, b"data1").unwrap();
        fs::write(&disk3, b"data3").unwrap();

        let inspector = ParallelInspector::default();
        let results = inspector.inspect_batch(&[&disk1, &disk2, &disk3]).unwrap();

        assert_eq!(results.len(), 3);
        // All fail: disk1/disk3 are dummy data, disk2 doesn't exist
        assert!(!results[0].success);
        assert!(!results[1].success);
        assert!(!results[2].success);
    }

    #[test]
    fn test_inspect_large_batch() {
        let temp_dir = TempDir::new().unwrap();
        let mut disks = Vec::new();

        // Create 20 test disks
        for i in 0..20 {
            let disk = temp_dir.path().join(format!("disk{}.img", i));
            fs::write(&disk, format!("data{}", i)).unwrap();
            disks.push(disk);
        }

        let inspector = ParallelInspector::default();
        let disk_refs: Vec<&PathBuf> = disks.iter().collect();
        let results = inspector.inspect_batch(&disk_refs).unwrap();

        assert_eq!(results.len(), 20);
        // All dummy files fail guestfs inspection
        assert!(results.iter().all(|r| !r.success));
    }

    // ========== Configuration Tests ==========

    #[test]
    fn test_config_cache_disabled() {
        let config = InspectionConfig {
            enable_cache: false,
            ..Default::default()
        };
        assert!(!config.enable_cache);
    }

    #[test]
    fn test_config_verbose_mode() {
        let config = InspectionConfig {
            verbose: true,
            ..Default::default()
        };
        assert!(config.verbose);
    }

    #[test]
    fn test_config_custom_timeout() {
        let config = InspectionConfig {
            timeout_secs: 600,
            ..Default::default()
        };
        assert_eq!(config.timeout_secs, 600);
    }

    #[test]
    fn test_config_fail_fast() {
        let config = InspectionConfig {
            continue_on_error: false,
            ..Default::default()
        };
        assert!(!config.continue_on_error);
    }

    // ========== InspectionResult Tests ==========

    #[test]
    fn test_result_success_fields() {
        let result = InspectionResult::success(
            PathBuf::from("/path/to/disk.qcow2"),
            Duration::from_millis(1500),
            "windows".to_string(),
            "Windows 10 Pro".to_string(),
            true,
        );

        assert_eq!(result.disk_path, PathBuf::from("/path/to/disk.qcow2"));
        assert!(result.success);
        assert_eq!(result.error, None);
        assert_eq!(result.duration, Duration::from_millis(1500));
        assert_eq!(result.os_type, Some("windows".to_string()));
        assert_eq!(result.product_name, Some("Windows 10 Pro".to_string()));
        assert!(result.from_cache);
    }

    #[test]
    fn test_result_failure_fields() {
        let result = InspectionResult::failure(
            PathBuf::from("/path/to/bad.img"),
            Duration::from_millis(500),
            "Permission denied".to_string(),
        );

        assert_eq!(result.disk_path, PathBuf::from("/path/to/bad.img"));
        assert!(!result.success);
        assert_eq!(result.error, Some("Permission denied".to_string()));
        assert_eq!(result.duration, Duration::from_millis(500));
        assert_eq!(result.os_type, None);
        assert_eq!(result.product_name, None);
        assert!(!result.from_cache);
    }

    #[test]
    fn test_result_duration_tracking() {
        let short = InspectionResult::success(
            PathBuf::from("test.img"),
            Duration::from_millis(100),
            "linux".to_string(),
            "Debian".to_string(),
            true,
        );

        let long = InspectionResult::failure(
            PathBuf::from("test2.img"),
            Duration::from_secs(5),
            "Timeout".to_string(),
        );

        assert!(short.duration < long.duration);
    }

    // ========== Progressive Inspector Tests ==========

    #[test]
    fn test_progressive_inspector_creation() {
        let config = InspectionConfig::default();
        let _inspector = ProgressiveInspector::new(config);
    }

    #[test]
    fn test_progressive_inspector_with_progress() {
        let temp_dir = TempDir::new().unwrap();
        let disk1 = temp_dir.path().join("disk1.img");
        let disk2 = temp_dir.path().join("disk2.img");

        fs::write(&disk1, b"data1").unwrap();
        fs::write(&disk2, b"data2").unwrap();

        let config = InspectionConfig::default();
        let inspector = ProgressiveInspector::new(config);

        let mut progress_calls = 0;
        let results = inspector
            .inspect_batch_with_progress(&[&disk1, &disk2], |_current, _total| {
                progress_calls += 1;
            })
            .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(progress_calls, 2);
    }

    // ========== Error Handling Tests ==========

    #[test]
    fn test_inspect_nonexistent_file() {
        let inspector = ParallelInspector::default();
        let disk = PathBuf::from("/definitely/does/not/exist.img");

        let results = inspector.inspect_batch(&[&disk]).unwrap();
        assert_eq!(results.len(), 1);
        assert!(!results[0].success);
        assert!(results[0].error.is_some());
    }

    #[test]
    fn test_inspect_all_failures() {
        let inspector = ParallelInspector::default();
        let disks = [
            PathBuf::from("/nonexistent1.img"),
            PathBuf::from("/nonexistent2.img"),
            PathBuf::from("/nonexistent3.img"),
        ];

        let disk_refs: Vec<&PathBuf> = disks.iter().collect();
        let results = inspector.inspect_batch(&disk_refs).unwrap();

        assert_eq!(results.len(), 3);
        assert!(results.iter().all(|r| !r.success));
        assert!(results.iter().all(|r| r.error.is_some()));
    }

    #[test]
    fn test_cache_key_nonexistent_file() {
        let inspector = ParallelInspector::default();
        let disk = PathBuf::from("/nonexistent.img");

        let result = inspector.generate_cache_key(&disk);
        assert!(result.is_err());
    }

    // ========== Integration Tests ==========

    #[test]
    fn test_full_workflow_no_cache() {
        let temp_dir = TempDir::new().unwrap();
        let disk = temp_dir.path().join("workflow.img");
        fs::write(&disk, b"test").unwrap();

        let config = InspectionConfig {
            enable_cache: false,
            max_workers: 2,
            verbose: false,
            ..Default::default()
        };

        let inspector = ParallelInspector::new(config);
        let results = inspector.inspect_batch(&[&disk]).unwrap();

        assert_eq!(results.len(), 1);
        // Dummy file fails guestfs inspection
        assert!(!results[0].success);
    }

    #[test]
    fn test_result_ordering_preserved() {
        let temp_dir = TempDir::new().unwrap();
        let disk1 = temp_dir.path().join("first.img");
        let disk2 = temp_dir.path().join("second.img");
        let disk3 = temp_dir.path().join("third.img");

        fs::write(&disk1, b"1").unwrap();
        fs::write(&disk2, b"2").unwrap();
        fs::write(&disk3, b"3").unwrap();

        let inspector = ParallelInspector::default();
        let results = inspector.inspect_batch(&[&disk1, &disk2, &disk3]).unwrap();

        assert_eq!(results.len(), 3);
        assert_eq!(results[0].disk_path, disk1);
        assert_eq!(results[1].disk_path, disk2);
        assert_eq!(results[2].disk_path, disk3);
    }
}
