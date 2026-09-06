// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance benchmarks for GuestKit operations

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use guestkit::guestfs::Guestfs;
use std::hint::black_box;
use std::path::PathBuf;

// Test image paths (set via environment or use defaults)
fn get_test_image(name: &str) -> Option<PathBuf> {
    std::env::var(format!("GUESTKIT_TEST_{}", name.to_uppercase()))
        .ok()
        .map(PathBuf::from)
        .or_else(|| {
            let default_path = format!("test-images/{}.qcow2", name);
            if std::path::Path::new(&default_path).exists() {
                Some(PathBuf::from(default_path))
            } else {
                None
            }
        })
}

/// Benchmark: Create and launch Guestfs handle
fn bench_create_and_launch(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        c.bench_function("create_and_launch", |b| {
            b.iter(|| {
                let mut g = Guestfs::new().unwrap();
                g.add_drive_ro(&image).unwrap();
                g.launch().unwrap();
                black_box(&g);
                g.shutdown().ok();
            });
        });
    } else {
        println!("Skipping create_and_launch: No test image found");
    }
}

/// Benchmark: OS inspection across different distributions
fn bench_inspect_os(c: &mut Criterion) {
    let test_images = vec![
        ("ubuntu-22.04", get_test_image("ubuntu-22.04")),
        ("debian-12", get_test_image("debian-12")),
        ("fedora-38", get_test_image("fedora-38")),
    ];

    let mut group = c.benchmark_group("inspect_os");

    for (name, image_opt) in test_images {
        if let Some(image) = image_opt {
            group.bench_with_input(BenchmarkId::from_parameter(name), &image, |b, img| {
                b.iter(|| {
                    let mut g = Guestfs::new().unwrap();
                    g.add_drive_ro(img).unwrap();
                    g.launch().unwrap();
                    let roots = g.inspect_os().unwrap();
                    black_box(&roots);
                    g.shutdown().ok();
                });
            });
        }
    }

    group.finish();
}

/// Benchmark: Get OS metadata (type, distro, version, etc.)
fn bench_os_metadata(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        // Setup: Create and launch once
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(&image).unwrap();
        g.launch().unwrap();
        let roots = g.inspect_os().unwrap();
        let root = roots.first().unwrap();

        let mut group = c.benchmark_group("os_metadata");

        group.bench_function("inspect_get_type", |b| {
            b.iter(|| {
                let os_type = g.inspect_get_type(root).unwrap();
                black_box(&os_type);
            });
        });

        group.bench_function("inspect_get_distro", |b| {
            b.iter(|| {
                let distro = g.inspect_get_distro(root).unwrap();
                black_box(&distro);
            });
        });

        group.bench_function("inspect_get_hostname", |b| {
            b.iter(|| {
                let hostname = g.inspect_get_hostname(root).unwrap();
                black_box(&hostname);
            });
        });

        group.bench_function("inspect_get_mountpoints", |b| {
            b.iter(|| {
                let mountpoints = g.inspect_get_mountpoints(root).unwrap();
                black_box(&mountpoints);
            });
        });

        group.finish();
        g.shutdown().ok();
    }
}

/// Benchmark: Mount and unmount operations
fn bench_mount_operations(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        let mut group = c.benchmark_group("mount_operations");

        group.bench_function("mount_unmount", |b| {
            // Setup once
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(&image).unwrap();
            g.launch().unwrap();

            b.iter(|| {
                g.mount_ro("/dev/sda2", "/").unwrap();
                black_box(&g);
                g.umount("/").unwrap();
            });

            g.shutdown().ok();
        });

        group.finish();
    }
}

/// Benchmark: List devices and partitions
fn bench_list_operations(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        // Setup once
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(&image).unwrap();
        g.launch().unwrap();

        let mut group = c.benchmark_group("list_operations");

        group.bench_function("list_devices", |b| {
            b.iter(|| {
                let devices = g.list_devices().unwrap();
                black_box(&devices);
            });
        });

        group.bench_function("list_partitions", |b| {
            b.iter(|| {
                let partitions = g.list_partitions().unwrap();
                black_box(&partitions);
            });
        });

        group.bench_function("list_filesystems", |b| {
            b.iter(|| {
                let filesystems = g.list_filesystems().unwrap();
                black_box(&filesystems);
            });
        });

        group.finish();
        g.shutdown().ok();
    }
}

/// Benchmark: File operations (read, ls, stat)
fn bench_file_operations(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        // Setup once
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(&image).unwrap();
        g.launch().unwrap();
        g.mount_ro("/dev/sda2", "/").unwrap();

        let mut group = c.benchmark_group("file_operations");

        group.bench_function("read_small_file", |b| {
            b.iter(|| {
                let content = g.read_file("/etc/hostname").unwrap();
                black_box(&content);
            });
        });

        group.bench_function("ls_directory", |b| {
            b.iter(|| {
                let entries = g.ls("/etc").unwrap();
                black_box(&entries);
            });
        });

        group.bench_function("stat_file", |b| {
            b.iter(|| {
                let stat = g.stat("/etc/passwd").unwrap();
                black_box(&stat);
            });
        });

        group.bench_function("is_file", |b| {
            b.iter(|| {
                let result = g.is_file("/etc/passwd").unwrap();
                black_box(&result);
            });
        });

        group.bench_function("is_dir", |b| {
            b.iter(|| {
                let result = g.is_dir("/etc").unwrap();
                black_box(&result);
            });
        });

        group.finish();
        g.umount_all().ok();
        g.shutdown().ok();
    }
}

/// Benchmark: Package listing (can be slow)
fn bench_package_operations(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        let mut group = c.benchmark_group("package_operations");
        group.sample_size(10); // Fewer samples for slow operations

        group.bench_function("list_applications", |b| {
            b.iter(|| {
                let mut g = Guestfs::new().unwrap();
                g.add_drive_ro(&image).unwrap();
                g.launch().unwrap();

                let roots = g.inspect_os().unwrap();
                let root = roots.first().unwrap();

                // Mount filesystems
                if let Ok(mountpoints) = g.inspect_get_mountpoints(root) {
                    let mounts: Vec<_> = mountpoints.iter().collect();
                    for (mount, device) in mounts.into_iter().rev() {
                        g.mount_ro(device, mount).ok();
                    }
                }

                let apps = g.inspect_list_applications(root).unwrap();
                black_box(&apps);

                g.umount_all().ok();
                g.shutdown().ok();
            });
        });

        group.finish();
    }
}

/// Benchmark: Filesystem information
fn bench_filesystem_info(c: &mut Criterion) {
    if let Some(image) = get_test_image("ubuntu-22.04") {
        // Setup once
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(&image).unwrap();
        g.launch().unwrap();

        let mut group = c.benchmark_group("filesystem_info");

        group.bench_function("vfs_type", |b| {
            b.iter(|| {
                let fstype = g.vfs_type("/dev/sda2").unwrap();
                black_box(&fstype);
            });
        });

        group.bench_function("vfs_label", |b| {
            b.iter(|| {
                let label = g.vfs_label("/dev/sda2").unwrap_or_default();
                black_box(&label);
            });
        });

        group.bench_function("vfs_uuid", |b| {
            b.iter(|| {
                let uuid = g.vfs_uuid("/dev/sda2").unwrap_or_default();
                black_box(&uuid);
            });
        });

        group.bench_function("blockdev_getsize64", |b| {
            b.iter(|| {
                let size = g.blockdev_getsize64("/dev/sda2").unwrap();
                black_box(&size);
            });
        });

        group.finish();
        g.shutdown().ok();
    }
}

criterion_group!(
    benches,
    bench_create_and_launch,
    bench_inspect_os,
    bench_os_metadata,
    bench_mount_operations,
    bench_list_operations,
    bench_file_operations,
    bench_package_operations,
    bench_filesystem_info,
);

criterion_main!(benches);
