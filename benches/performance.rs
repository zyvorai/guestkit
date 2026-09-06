// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance benchmarks for guestkit operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use std::hint::black_box;
use std::time::Duration;

/// Benchmark appliance launch and shutdown
fn benchmark_appliance_lifecycle(c: &mut Criterion) {
    let mut group = c.benchmark_group("appliance");
    group.measurement_time(Duration::from_secs(20));
    group.sample_size(10); // Small sample due to slow operation

    group.bench_function("launch_and_shutdown", |b| {
        b.iter(|| {
            // Note: This requires actual guestfs implementation
            // For now, simulate the operation
            std::thread::sleep(Duration::from_millis(2500));
            black_box(());
        });
    });

    group.finish();
}

/// Benchmark OS inspection operations
fn benchmark_inspection(c: &mut Criterion) {
    let mut group = c.benchmark_group("inspection");
    group.measurement_time(Duration::from_secs(15));

    // Simulate inspection of different OS types
    for os_type in &["linux", "windows", "freebsd"] {
        group.bench_with_input(BenchmarkId::new("detect_os", os_type), os_type, |b, &os| {
            b.iter(|| {
                // Simulate OS detection
                std::thread::sleep(Duration::from_millis(500));
                black_box(os);
            });
        });
    }

    group.finish();
}

/// Benchmark cache operations (JSON vs Binary)
fn benchmark_cache(c: &mut Criterion) {
    use serde::{Deserialize, Serialize};

    #[derive(Serialize, Deserialize, Clone)]
    struct MockData {
        os_type: String,
        distro: String,
        version: i32,
        packages: Vec<String>,
    }

    let data = MockData {
        os_type: "linux".to_string(),
        distro: "ubuntu".to_string(),
        version: 2204,
        packages: (0..1000).map(|i| format!("package-{}", i)).collect(),
    };

    let mut group = c.benchmark_group("cache");

    // JSON serialization
    group.bench_function("json_serialize", |b| {
        b.iter(|| {
            let json = serde_json::to_string(&data).unwrap();
            black_box(json);
        });
    });

    group.bench_function("json_deserialize", |b| {
        let json = serde_json::to_string(&data).unwrap();
        b.iter(|| {
            let deserialized: MockData = serde_json::from_str(&json).unwrap();
            black_box(deserialized);
        });
    });

    // Binary serialization (bincode)
    group.bench_function("bincode_serialize", |b| {
        b.iter(|| {
            let bytes = bincode::serialize(&data).unwrap();
            black_box(bytes);
        });
    });

    group.bench_function("bincode_deserialize", |b| {
        let bytes = bincode::serialize(&data).unwrap();
        b.iter(|| {
            let deserialized: MockData = bincode::deserialize(&bytes).unwrap();
            black_box(deserialized);
        });
    });

    group.finish();
}

/// Benchmark package listing operations
fn benchmark_package_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("packages");
    group.measurement_time(Duration::from_secs(10));

    for count in &[100, 500, 1000] {
        group.bench_with_input(
            BenchmarkId::new("list_packages", count),
            count,
            |b, &count| {
                b.iter(|| {
                    // Simulate package listing
                    let packages: Vec<String> =
                        (0..count).map(|i| format!("package-{}", i)).collect();
                    black_box(packages);
                });
            },
        );
    }

    group.finish();
}

/// Benchmark file operations
fn benchmark_file_operations(c: &mut Criterion) {
    let mut group = c.benchmark_group("file_ops");

    group.bench_function("read_small_file", |b| {
        b.iter(|| {
            // Simulate reading 1KB file
            std::thread::sleep(Duration::from_millis(15));
            black_box(vec![0u8; 1024]);
        });
    });

    group.bench_function("read_medium_file", |b| {
        b.iter(|| {
            // Simulate reading 1MB file
            std::thread::sleep(Duration::from_millis(50));
            black_box(vec![0u8; 1024 * 1024]);
        });
    });

    group.finish();
}

/// Benchmark parallel processing
fn benchmark_parallel(c: &mut Criterion) {
    use rayon::prelude::*;

    let mut group = c.benchmark_group("parallel");

    let data: Vec<i32> = (0..10000).collect();

    group.bench_function("sequential", |b| {
        b.iter(|| {
            let result: Vec<i32> = data.iter().map(|&x| x * 2).collect();
            black_box(result);
        });
    });

    group.bench_function("parallel", |b| {
        b.iter(|| {
            let result: Vec<i32> = data.par_iter().map(|&x| x * 2).collect();
            black_box(result);
        });
    });

    group.finish();
}

/// Benchmark string operations (for optimization)
fn benchmark_string_ops(c: &mut Criterion) {
    let mut group = c.benchmark_group("strings");

    let strings: Vec<String> = (0..1000).map(|i| format!("package-name-{}", i)).collect();

    group.bench_function("clone_strings", |b| {
        b.iter(|| {
            let cloned: Vec<String> = strings.clone();
            black_box(cloned);
        });
    });

    // Using Arc for sharing
    use std::sync::Arc;

    group.bench_function("arc_strings", |b| {
        let arc_strings: Vec<Arc<String>> = strings.iter().map(|s| Arc::new(s.clone())).collect();

        b.iter(|| {
            let cloned: Vec<Arc<String>> = arc_strings.clone();
            black_box(cloned);
        });
    });

    group.finish();
}

/// Benchmark memory allocation patterns
fn benchmark_memory(c: &mut Criterion) {
    let mut group = c.benchmark_group("memory");

    group.bench_function("vec_push", |b| {
        b.iter(|| {
            let mut vec = Vec::new();
            for i in 0..1000 {
                vec.push(i);
            }
            black_box(vec);
        });
    });

    group.bench_function("vec_with_capacity", |b| {
        b.iter(|| {
            let mut vec = Vec::with_capacity(1000);
            for i in 0..1000 {
                vec.push(i);
            }
            black_box(vec);
        });
    });

    group.finish();
}

/// Benchmark hash operations (for cache keys)
fn benchmark_hashing(c: &mut Criterion) {
    use sha2::{Digest, Sha256};

    let mut group = c.benchmark_group("hashing");

    let data = vec![0u8; 1024 * 1024]; // 1MB

    group.bench_function("sha256_1mb", |b| {
        b.iter(|| {
            let mut hasher = Sha256::new();
            hasher.update(&data);
            let hash = hasher.finalize();
            black_box(hash);
        });
    });

    group.finish();
}

/// Benchmark batch inspection with different worker counts
fn benchmark_batch_inspection(c: &mut Criterion) {
    use std::fs;
    use tempfile::TempDir;

    let mut group = c.benchmark_group("batch_inspection");
    group.measurement_time(Duration::from_secs(10));

    // Create temporary disk files for testing
    let temp_dir = TempDir::new().unwrap();
    let disks: Vec<_> = (0..8)
        .map(|i| {
            let path = temp_dir.path().join(format!("disk{}.img", i));
            fs::write(&path, vec![0u8; 1024 * 1024]).unwrap(); // 1MB fake disk
            path
        })
        .collect();

    // Benchmark sequential processing (1 worker)
    group.bench_function("sequential_8_disks", |b| {
        b.iter(|| {
            for disk in &disks {
                // Simulate inspection workload
                std::thread::sleep(Duration::from_millis(50));
                black_box(disk);
            }
        });
    });

    // Benchmark parallel processing (4 workers)
    group.bench_function("parallel_4_workers_8_disks", |b| {
        use rayon::prelude::*;

        b.iter(|| {
            disks.par_iter().for_each(|disk| {
                // Simulate inspection workload
                std::thread::sleep(Duration::from_millis(50));
                black_box(disk);
            });
        });
    });

    // Benchmark parallel processing (8 workers)
    group.bench_function("parallel_8_workers_8_disks", |b| {
        use rayon::prelude::*;

        b.iter(|| {
            disks.par_iter().for_each(|disk| {
                // Simulate inspection workload
                std::thread::sleep(Duration::from_millis(50));
                black_box(disk);
            });
        });
    });

    group.finish();
}

/// Benchmark cache hit vs cache miss performance
fn benchmark_cache_performance(c: &mut Criterion) {
    use guestkit::core::BinaryCache;
    use tempfile::TempDir;

    let mut group = c.benchmark_group("cache_performance");

    let temp_dir = TempDir::new().unwrap();
    let cache = BinaryCache::with_dir(temp_dir.path().to_path_buf()).unwrap();

    // Create large mock data (simulating full VM inspection)
    let large_data = guestkit::core::CachedInspection::new("test-key".to_string());

    // Save to cache first
    cache.save("large-inspection", &large_data).unwrap();

    // Benchmark cache hit (fast path)
    group.bench_function("cache_hit", |b| {
        b.iter(|| {
            let loaded = cache.load("large-inspection").unwrap();
            black_box(loaded);
        });
    });

    // Benchmark cache miss + save (slow path)
    group.bench_function("cache_miss_and_save", |b| {
        let mut counter = 0;
        b.iter(|| {
            let key = format!("cache-miss-{}", counter);
            counter += 1;

            // Simulate inspection work
            let data = guestkit::core::CachedInspection::new(key.clone());

            // Save to cache
            cache.save(&key, &data).unwrap();
            black_box(data);
        });
    });

    // Benchmark cache statistics calculation
    group.bench_function("cache_stats", |b| {
        b.iter(|| {
            let stats = cache.stats().unwrap();
            black_box(stats);
        });
    });

    group.finish();
}

criterion_group!(
    benches,
    benchmark_appliance_lifecycle,
    benchmark_inspection,
    benchmark_cache,
    benchmark_package_operations,
    benchmark_file_operations,
    benchmark_parallel,
    benchmark_string_ops,
    benchmark_memory,
    benchmark_hashing,
    benchmark_batch_inspection,
    benchmark_cache_performance,
);

criterion_main!(benches);
