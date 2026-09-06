// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Performance benchmarks for guestkit
//!
//! Run with: cargo bench

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use guestkit::Guestfs;
use std::fs;
use std::hint::black_box;

fn setup_test_disk(path: &str, size_mb: u64) {
    let _ = fs::remove_file(path);
    let mut g = Guestfs::new().unwrap();
    g.disk_create(path, "raw", (size_mb * 1024 * 1024) as i64)
        .unwrap();
}

fn cleanup_disk(path: &str) {
    let _ = fs::remove_file(path);
}

fn bench_disk_creation(c: &mut Criterion) {
    let mut group = c.benchmark_group("disk_creation");

    for size in [10, 50, 100, 500].iter() {
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, &size| {
            b.iter(|| {
                let path = format!("/tmp/bench_disk_{}mb.img", size);
                let mut g = Guestfs::new().unwrap();
                g.disk_create(&path, "raw", black_box((size * 1024 * 1024) as i64))
                    .unwrap();
                cleanup_disk(&path);
            });
        });
    }

    group.finish();
}

fn bench_partition_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_partition.img";

    c.bench_function("partition_creation", |b| {
        setup_test_disk(disk_path, 100);

        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();

            g.part_init("/dev/sda", "gpt").unwrap();
            g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();

            g.shutdown().unwrap();
        });

        cleanup_disk(disk_path);
    });
}

fn bench_filesystem_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_filesystem.img";
    setup_test_disk(disk_path, 200);

    let mut g = Guestfs::new().unwrap();
    g.add_drive_ro(disk_path).unwrap();
    g.launch().unwrap();
    g.part_init("/dev/sda", "gpt").unwrap();
    g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();
    g.shutdown().unwrap();

    c.bench_function("mkfs_ext4", |b| {
        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();

            g.mkfs("ext4", "/dev/sda1").unwrap();

            g.shutdown().unwrap();
        });
    });

    cleanup_disk(disk_path);
}

fn bench_file_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_file_ops.img";
    setup_test_disk(disk_path, 100);

    // Setup filesystem
    let mut g = Guestfs::new().unwrap();
    g.add_drive_ro(disk_path).unwrap();
    g.launch().unwrap();
    g.part_init("/dev/sda", "gpt").unwrap();
    g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();
    g.mkfs("ext4", "/dev/sda1").unwrap();
    g.shutdown().unwrap();

    let mut group = c.benchmark_group("file_operations");

    // Benchmark file write
    for size in [1, 10, 100, 1000].iter() {
        let size_kb = *size;
        group.bench_with_input(
            BenchmarkId::new("write", format!("{}KB", size_kb)),
            &size_kb,
            |b, &size_kb| {
                let content = vec![b'A'; size_kb * 1024];

                b.iter(|| {
                    let mut g = Guestfs::new().unwrap();
                    g.add_drive_ro(disk_path).unwrap();
                    g.launch().unwrap();
                    g.mount("/dev/sda1", "/").unwrap();

                    g.write("/test.dat", &content).unwrap();

                    g.umount("/").unwrap();
                    g.shutdown().unwrap();
                });
            },
        );
    }

    // Benchmark file read
    {
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(disk_path).unwrap();
        g.launch().unwrap();
        g.mount("/dev/sda1", "/").unwrap();
        g.write("/read_test.txt", b"Test content for reading")
            .unwrap();
        g.umount("/").unwrap();
        g.shutdown().unwrap();
    }

    group.bench_function("read_small_file", |b| {
        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();
            g.mount("/dev/sda1", "/").unwrap();

            let _content = g.cat("/read_test.txt").unwrap();

            g.umount("/").unwrap();
            g.shutdown().unwrap();
        });
    });

    group.finish();
    cleanup_disk(disk_path);
}

fn bench_mount_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_mount.img";
    setup_test_disk(disk_path, 100);

    // Setup filesystem
    let mut g = Guestfs::new().unwrap();
    g.add_drive_ro(disk_path).unwrap();
    g.launch().unwrap();
    g.part_init("/dev/sda", "gpt").unwrap();
    g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();
    g.mkfs("ext4", "/dev/sda1").unwrap();
    g.shutdown().unwrap();

    c.bench_function("mount_unmount", |b| {
        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();

            g.mount("/dev/sda1", "/").unwrap();
            g.umount("/").unwrap();

            g.shutdown().unwrap();
        });
    });

    cleanup_disk(disk_path);
}

fn bench_checksum_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_checksum.img";
    setup_test_disk(disk_path, 100);

    // Setup filesystem with test file
    let mut g = Guestfs::new().unwrap();
    g.add_drive_ro(disk_path).unwrap();
    g.launch().unwrap();
    g.part_init("/dev/sda", "gpt").unwrap();
    g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();
    g.mkfs("ext4", "/dev/sda1").unwrap();
    g.mount("/dev/sda1", "/").unwrap();

    // Create test files of different sizes
    g.write("/small.dat", &vec![b'A'; 1024]).unwrap(); // 1KB
    g.write("/medium.dat", &vec![b'B'; 100 * 1024]).unwrap(); // 100KB
    g.write("/large.dat", &vec![b'C'; 1024 * 1024]).unwrap(); // 1MB

    g.umount("/").unwrap();
    g.shutdown().unwrap();

    let mut group = c.benchmark_group("checksum");

    for (name, file) in [
        ("small", "/small.dat"),
        ("medium", "/medium.dat"),
        ("large", "/large.dat"),
    ]
    .iter()
    {
        for algo in ["md5", "sha256"].iter() {
            group.bench_function(format!("{}_{}", algo, name), |b| {
                b.iter(|| {
                    let mut g = Guestfs::new().unwrap();
                    g.add_drive_ro(disk_path).unwrap();
                    g.launch().unwrap();
                    g.mount("/dev/sda1", "/").unwrap();

                    let _checksum = g.checksum(black_box(algo), black_box(file)).unwrap();

                    g.umount("/").unwrap();
                    g.shutdown().unwrap();
                });
            });
        }
    }

    group.finish();
    cleanup_disk(disk_path);
}

fn bench_archive_operations(c: &mut Criterion) {
    let disk_path = "/tmp/bench_archive.img";
    setup_test_disk(disk_path, 200);

    // Setup filesystem with test data
    let mut g = Guestfs::new().unwrap();
    g.add_drive_ro(disk_path).unwrap();
    g.launch().unwrap();
    g.part_init("/dev/sda", "gpt").unwrap();
    g.part_add("/dev/sda", "primary", 2048, -2048).unwrap();
    g.mkfs("ext4", "/dev/sda1").unwrap();
    g.mount("/dev/sda1", "/").unwrap();

    // Create test directory structure
    g.mkdir_p("/data").unwrap();
    for i in 0..10 {
        g.write(&format!("/data/file{}.txt", i), &vec![b'X'; 10 * 1024])
            .unwrap();
    }

    g.umount("/").unwrap();
    g.shutdown().unwrap();

    c.bench_function("tar_out", |b| {
        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();
            g.mount("/dev/sda1", "/").unwrap();

            g.tar_out("/data", "/tmp/archive.tar").unwrap();

            g.umount("/").unwrap();
            g.shutdown().unwrap();
        });
    });

    c.bench_function("tar_in", |b| {
        // First create an archive
        let mut g = Guestfs::new().unwrap();
        g.add_drive_ro(disk_path).unwrap();
        g.launch().unwrap();
        g.mount("/dev/sda1", "/").unwrap();
        g.tar_out("/data", "/tmp/test_archive.tar").unwrap();
        g.umount("/").unwrap();
        g.shutdown().unwrap();

        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();
            g.launch().unwrap();
            g.mount("/dev/sda1", "/").unwrap();

            g.tar_in("/tmp/test_archive.tar", "/restore").unwrap();

            g.umount("/").unwrap();
            g.shutdown().unwrap();
        });

        let _ = fs::remove_file("/tmp/test_archive.tar");
    });

    cleanup_disk(disk_path);
}

fn bench_launch_shutdown(c: &mut Criterion) {
    let disk_path = "/tmp/bench_launch.img";
    setup_test_disk(disk_path, 50);

    c.bench_function("launch_shutdown", |b| {
        b.iter(|| {
            let mut g = Guestfs::new().unwrap();
            g.add_drive_ro(disk_path).unwrap();

            g.launch().unwrap();
            g.shutdown().unwrap();
        });
    });

    cleanup_disk(disk_path);
}

criterion_group!(
    benches,
    bench_disk_creation,
    bench_partition_operations,
    bench_filesystem_operations,
    bench_file_operations,
    bench_mount_operations,
    bench_checksum_operations,
    bench_archive_operations,
    bench_launch_shutdown,
);

criterion_main!(benches);
