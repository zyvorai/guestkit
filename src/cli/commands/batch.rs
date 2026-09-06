// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Batch inspection commands
#![allow(clippy::too_many_arguments)]

use crate::cli::formatters::*;
use anyhow::Result;
use std::path::{Path, PathBuf};

use super::collect_inspection_data;

/// Batch inspect multiple images
pub fn inspect_batch(
    images: &[PathBuf],
    parallel: usize,
    verbose: bool,
    output_format: Option<OutputFormat>,
    use_cache: bool,
) -> Result<()> {
    use crate::cli::cache::InspectionCache;
    use std::sync::{Arc, Mutex};
    use std::thread;

    println!("=== Batch Inspection ===");
    println!("Images: {}", images.len());
    println!("Parallel workers: {}", parallel);
    println!();

    // Shared results vector
    type BatchResults = Vec<(String, Result<InspectionReport>)>;
    let results: Arc<Mutex<BatchResults>> = Arc::new(Mutex::new(Vec::new()));

    // Create work queue
    let work_queue: Arc<Mutex<Vec<PathBuf>>> = Arc::new(Mutex::new(images.to_vec()));

    // Progress tracking
    let total = images.len();
    let completed = Arc::new(Mutex::new(0usize));

    // Spawn worker threads
    let mut handles = vec![];

    for worker_id in 0..parallel {
        let work_queue = Arc::clone(&work_queue);
        let results = Arc::clone(&results);
        let completed = Arc::clone(&completed);

        let handle = thread::spawn(move || {
            loop {
                // Get next image from queue
                let image = {
                    let Ok(mut queue) = work_queue.lock() else {
                        eprintln!("[Worker {}] Failed to acquire work queue lock", worker_id);
                        break;
                    };
                    match queue.pop() {
                        Some(image) => image,
                        None => break,
                    }
                };

                if verbose {
                    eprintln!("[Worker {}] Processing: {}", worker_id, image.display());
                }

                // Try cache first if enabled
                let report_result = if use_cache {
                    if let Ok(cache) = InspectionCache::new() {
                        if let Ok(Some(cached)) = cache.get(&image) {
                            eprintln!("✓ [Worker {}] Cache hit: {}", worker_id, image.display());
                            Ok(cached)
                        } else {
                            inspect_single_image(&image, verbose, use_cache)
                        }
                    } else {
                        inspect_single_image(&image, verbose, use_cache)
                    }
                } else {
                    inspect_single_image(&image, verbose, use_cache)
                };

                // Store result
                if let Ok(mut res) = results.lock() {
                    res.push((image.to_string_lossy().to_string(), report_result));
                }

                // Update progress
                if let Ok(mut count) = completed.lock() {
                    *count += 1;
                    eprintln!("Progress: {}/{}", *count, total);
                }
            }
        });

        handles.push(handle);
    }

    // Wait for all workers to complete
    let mut panic_count = 0usize;
    for handle in handles {
        if let Err(e) = handle.join() {
            eprintln!("Worker thread panicked: {:?}", e);
            panic_count += 1;
        }
    }
    if panic_count > 0 {
        eprintln!(
            "\nWarning: {} worker thread(s) panicked during processing",
            panic_count
        );
    }

    println!("\n=== Results ===\n");

    // Print results
    let final_results = results
        .lock()
        .map_err(|e| anyhow::anyhow!("Failed to lock results: {}", e))?;
    let mut success_count = 0;
    let mut error_count = 0;

    for (image_path, result) in final_results.iter() {
        match result {
            Ok(report) => {
                success_count += 1;

                if let Some(format) = output_format {
                    // JSON/YAML output
                    let formatter = get_formatter(format, true)?;
                    let output = formatter.format(report)?;
                    println!("=== {} ===", image_path);
                    println!("{}", output);
                    println!();
                } else {
                    // Summary output
                    println!("✓ {}", image_path);
                    println!(
                        "  OS: {} {}",
                        report.os.distribution.as_deref().unwrap_or("Unknown"),
                        report
                            .os
                            .version
                            .as_ref()
                            .map(|v| format!("{}.{}", v.major, v.minor))
                            .unwrap_or_else(|| "N/A".to_string())
                    );
                    if let Some(hostname) = &report.os.hostname {
                        println!("  Hostname: {}", hostname);
                    }
                    if let Some(packages) = &report.packages {
                        println!("  Packages: {}", packages.count);
                    }
                    println!();
                }
            }
            Err(e) => {
                error_count += 1;
                println!("✗ {}", image_path);
                println!("  Error: {}", e);
                println!();
            }
        }
    }

    println!("=== Summary ===");
    println!("Total: {}", final_results.len());
    println!("Success: {}", success_count);
    println!("Errors: {}", error_count);

    Ok(())
}

/// Inspect a single image (helper for batch processing)
pub fn inspect_single_image(
    image: &Path,
    verbose: bool,
    use_cache: bool,
) -> Result<InspectionReport> {
    use crate::cli::cache::InspectionCache;

    let mut g = super::init_guestfs_ro(image, false)?;

    let roots = g.inspect_os()?;
    if roots.is_empty() {
        g.shutdown()?;
        return Err(anyhow::anyhow!("No operating system found"));
    }

    let mut report = collect_inspection_data(&mut g, &roots[0], verbose)?;
    report.image_path = Some(image.to_string_lossy().to_string());

    g.shutdown()?;

    // Store in cache if enabled
    if use_cache {
        if let Ok(cache) = InspectionCache::new() {
            let _ = cache.store(image, &report);
        }
    }

    Ok(report)
}
