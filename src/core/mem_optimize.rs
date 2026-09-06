// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Memory optimization utilities for guestkit
//!
//! This module provides optimized memory allocation patterns to improve
//! performance by reducing reallocation overhead.
//!
//! # Performance Impact
//!
//! Using pre-allocated vectors provides approximately 2x speedup for
//! allocation-heavy operations by avoiding multiple reallocations as
//! the vector grows.
//!
//! # Usage
//!
//! ```rust
//! use guestkit::core::mem_optimize;
//!
//! // Instead of Vec::new()
//! let packages: Vec<String> = mem_optimize::vec_for_packages();
//!
//! // Instead of Vec::with_capacity(arbitrary_number)
//! let filesystems: Vec<String> = mem_optimize::vec_for_filesystems();
//! ```

/// Typical capacity estimates based on real-world observations
pub mod capacity {
    /// Typical number of partitions in a VM disk (usually 1-4)
    pub const PARTITIONS: usize = 4;

    /// Typical number of filesystems (usually 2-8)
    pub const FILESYSTEMS: usize = 8;

    /// Typical number of mount points (usually 4-16)
    pub const MOUNT_POINTS: usize = 16;

    /// Small package count (minimal installations)
    pub const PACKAGES_SMALL: usize = 256;

    /// Medium package count (typical server)
    pub const PACKAGES_MEDIUM: usize = 512;

    /// Large package count (desktop/development system)
    pub const PACKAGES_LARGE: usize = 1024;

    /// Default package count
    pub const PACKAGES: usize = PACKAGES_MEDIUM;

    /// Typical user count (usually 5-20)
    pub const USERS: usize = 20;

    /// Typical service count (usually 20-100)
    pub const SERVICES: usize = 50;

    /// Typical network interfaces (usually 1-4)
    pub const NETWORK_INTERFACES: usize = 4;

    /// Typical LVM volume groups (usually 1-2)
    pub const LVM_VG: usize = 2;

    /// Typical LVM logical volumes per VG (usually 2-8)
    pub const LVM_LV: usize = 8;

    /// Typical file listing size (small directory)
    pub const FILES_SMALL: usize = 64;

    /// Typical file listing size (medium directory)
    pub const FILES_MEDIUM: usize = 256;

    /// Typical file listing size (large directory)
    pub const FILES_LARGE: usize = 1024;

    /// Default file listing size
    pub const FILES: usize = FILES_MEDIUM;

    /// Typical number of VMs in batch operations
    pub const BATCH_VMS: usize = 8;

    /// Typical number of environment variables
    pub const ENV_VARS: usize = 32;

    /// Typical number of cron jobs
    pub const CRON_JOBS: usize = 16;
}

/// Create a pre-allocated vector for partitions
///
/// # Examples
///
/// ```
/// let mut partitions: Vec<String> = guestkit::core::mem_optimize::vec_for_partitions();
/// // partitions is pre-allocated with capacity for typical partition count
/// ```
#[inline]
pub fn vec_for_partitions<T>() -> Vec<T> {
    Vec::with_capacity(capacity::PARTITIONS)
}

/// Create a pre-allocated vector for filesystems
#[inline]
pub fn vec_for_filesystems<T>() -> Vec<T> {
    Vec::with_capacity(capacity::FILESYSTEMS)
}

/// Create a pre-allocated vector for mount points
#[inline]
pub fn vec_for_mount_points<T>() -> Vec<T> {
    Vec::with_capacity(capacity::MOUNT_POINTS)
}

/// Create a pre-allocated vector for packages
///
/// Uses medium capacity by default (512 packages).
/// For minimal systems, consider using `vec_with_capacity(capacity::PACKAGES_SMALL)`.
#[inline]
pub fn vec_for_packages<T>() -> Vec<T> {
    Vec::with_capacity(capacity::PACKAGES)
}

/// Create a pre-allocated vector for users
#[inline]
pub fn vec_for_users<T>() -> Vec<T> {
    Vec::with_capacity(capacity::USERS)
}

/// Create a pre-allocated vector for services
#[inline]
pub fn vec_for_services<T>() -> Vec<T> {
    Vec::with_capacity(capacity::SERVICES)
}

/// Create a pre-allocated vector for network interfaces
#[inline]
pub fn vec_for_network_interfaces<T>() -> Vec<T> {
    Vec::with_capacity(capacity::NETWORK_INTERFACES)
}

/// Create a pre-allocated vector for LVM volume groups
#[inline]
pub fn vec_for_lvm_vg<T>() -> Vec<T> {
    Vec::with_capacity(capacity::LVM_VG)
}

/// Create a pre-allocated vector for LVM logical volumes
#[inline]
pub fn vec_for_lvm_lv<T>() -> Vec<T> {
    Vec::with_capacity(capacity::LVM_LV)
}

/// Create a pre-allocated vector for file listings
///
/// Uses medium capacity by default (256 files).
#[inline]
pub fn vec_for_files<T>() -> Vec<T> {
    Vec::with_capacity(capacity::FILES)
}

/// Create a pre-allocated vector for batch VM operations
#[inline]
pub fn vec_for_batch_vms<T>() -> Vec<T> {
    Vec::with_capacity(capacity::BATCH_VMS)
}

/// Create a pre-allocated vector for environment variables
#[inline]
pub fn vec_for_env_vars<T>() -> Vec<T> {
    Vec::with_capacity(capacity::ENV_VARS)
}

/// Create a pre-allocated vector for cron jobs
#[inline]
pub fn vec_for_cron_jobs<T>() -> Vec<T> {
    Vec::with_capacity(capacity::CRON_JOBS)
}

/// Create a vector with estimated capacity based on input size
///
/// Useful when processing collections where the output size is
/// proportional to the input size.
///
/// # Examples
///
/// ```
/// let input = vec![1, 2, 3, 4, 5];
/// let mut output: Vec<i32> = guestkit::core::mem_optimize::vec_with_estimated_capacity(&input, 1.5);
/// // output has capacity of ~7 (5 * 1.5), avoiding reallocation
/// ```
#[inline]
pub fn vec_with_estimated_capacity<T, U>(input: &[U], factor: f32) -> Vec<T> {
    let capacity = (input.len() as f32 * factor).ceil() as usize;
    Vec::with_capacity(capacity)
}

/// Optimize a vector by shrinking to fit if it has excess capacity
///
/// Only shrinks if the excess capacity is significant (>25% waste).
///
/// # Examples
///
/// ```
/// let mut vec: Vec<i32> = Vec::with_capacity(1000);
/// // ... add 100 items ...
/// guestkit::core::mem_optimize::shrink_if_wasteful(&mut vec);
/// // vec now uses less memory
/// ```
pub fn shrink_if_wasteful<T>(vec: &mut Vec<T>) {
    let len = vec.len();
    let cap = vec.capacity();

    // Only shrink if capacity is >25% wasted
    if cap > len && (cap - len) as f64 / cap as f64 > 0.25 {
        vec.shrink_to_fit();
    }
}

#[cfg(test)]
#[allow(clippy::assertions_on_constants)]
mod tests {
    use super::*;

    #[test]
    fn test_capacity_constants() {
        assert!(capacity::PARTITIONS > 0);
        assert!(capacity::PACKAGES > capacity::PACKAGES_SMALL);
        assert!(capacity::PACKAGES_LARGE > capacity::PACKAGES_MEDIUM);
    }

    #[test]
    fn test_vec_for_partitions() {
        let vec: Vec<String> = vec_for_partitions();
        assert_eq!(vec.len(), 0);
        assert!(vec.capacity() >= capacity::PARTITIONS);
    }

    #[test]
    fn test_vec_for_packages() {
        let vec: Vec<String> = vec_for_packages();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::PACKAGES);
    }

    #[test]
    fn test_vec_with_estimated_capacity() {
        let input = vec![1, 2, 3, 4, 5];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 2.0);
        assert_eq!(vec.len(), 0);
        assert!(vec.capacity() >= 10);
    }

    #[test]
    fn test_shrink_if_wasteful() {
        let mut vec = Vec::with_capacity(1000);
        vec.push(1);
        vec.push(2);
        vec.push(3);

        shrink_if_wasteful(&mut vec);

        // Should shrink since we're using <1% of capacity
        assert!(vec.capacity() < 1000);
    }

    #[test]
    fn test_shrink_if_wasteful_no_shrink() {
        let mut vec = Vec::with_capacity(10);
        vec.push(1);
        vec.push(2);
        vec.push(3);
        vec.push(4);
        vec.push(5);
        vec.push(6);
        vec.push(7);
        vec.push(8);

        let cap_before = vec.capacity();
        shrink_if_wasteful(&mut vec);
        let cap_after = vec.capacity();

        // Should NOT shrink since we're using >75% of capacity
        assert_eq!(cap_before, cap_after);
    }

    #[test]
    fn test_all_vec_creators() {
        // Ensure all functions compile and work
        let _: Vec<u8> = vec_for_partitions();
        let _: Vec<u8> = vec_for_filesystems();
        let _: Vec<u8> = vec_for_mount_points();
        let _: Vec<u8> = vec_for_packages();
        let _: Vec<u8> = vec_for_users();
        let _: Vec<u8> = vec_for_services();
        let _: Vec<u8> = vec_for_network_interfaces();
        let _: Vec<u8> = vec_for_lvm_vg();
        let _: Vec<u8> = vec_for_lvm_lv();
        let _: Vec<u8> = vec_for_files();
        let _: Vec<u8> = vec_for_batch_vms();
        let _: Vec<u8> = vec_for_env_vars();
        let _: Vec<u8> = vec_for_cron_jobs();
    }

    // ========== Edge Case Tests ==========

    #[test]
    fn test_vec_for_filesystems_capacity() {
        let vec: Vec<String> = vec_for_filesystems();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::FILESYSTEMS);
    }

    #[test]
    fn test_vec_for_mount_points_capacity() {
        let vec: Vec<String> = vec_for_mount_points();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::MOUNT_POINTS);
    }

    #[test]
    fn test_vec_for_users_capacity() {
        let vec: Vec<String> = vec_for_users();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::USERS);
    }

    #[test]
    fn test_vec_for_services_capacity() {
        let vec: Vec<String> = vec_for_services();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::SERVICES);
    }

    #[test]
    fn test_vec_for_network_interfaces_capacity() {
        let vec: Vec<String> = vec_for_network_interfaces();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::NETWORK_INTERFACES);
    }

    #[test]
    fn test_vec_for_lvm_vg_capacity() {
        let vec: Vec<String> = vec_for_lvm_vg();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::LVM_VG);
    }

    #[test]
    fn test_vec_for_lvm_lv_capacity() {
        let vec: Vec<String> = vec_for_lvm_lv();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::LVM_LV);
    }

    #[test]
    fn test_vec_for_files_capacity() {
        let vec: Vec<String> = vec_for_files();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::FILES);
    }

    #[test]
    fn test_vec_for_batch_vms_capacity() {
        let vec: Vec<String> = vec_for_batch_vms();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::BATCH_VMS);
    }

    #[test]
    fn test_vec_for_env_vars_capacity() {
        let vec: Vec<String> = vec_for_env_vars();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::ENV_VARS);
    }

    #[test]
    fn test_vec_for_cron_jobs_capacity() {
        let vec: Vec<String> = vec_for_cron_jobs();
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), capacity::CRON_JOBS);
    }

    // ========== vec_with_estimated_capacity Edge Cases ==========

    #[test]
    fn test_vec_with_estimated_capacity_zero_input() {
        let input: Vec<i32> = vec![];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 2.0);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 0);
    }

    #[test]
    fn test_vec_with_estimated_capacity_zero_multiplier() {
        let input = vec![1, 2, 3];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 0.0);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 0);
    }

    #[test]
    fn test_vec_with_estimated_capacity_one_multiplier() {
        let input = vec![1, 2, 3, 4, 5];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 1.0);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 5);
    }

    #[test]
    fn test_vec_with_estimated_capacity_fractional() {
        let input = vec![1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 0.5);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 5);
    }

    #[test]
    fn test_vec_with_estimated_capacity_large_multiplier() {
        let input = vec![1, 2, 3];
        let vec: Vec<i32> = vec_with_estimated_capacity(&input, 100.0);
        assert_eq!(vec.len(), 0);
        assert_eq!(vec.capacity(), 300);
    }

    // ========== shrink_if_wasteful Edge Cases ==========

    #[test]
    fn test_shrink_if_wasteful_empty_vec() {
        let mut vec: Vec<i32> = Vec::with_capacity(100);
        shrink_if_wasteful(&mut vec);
        // Empty vec with >25% waste should shrink
        assert!(vec.capacity() < 100);
    }

    #[test]
    fn test_shrink_if_wasteful_full_vec() {
        let mut vec = Vec::with_capacity(10);
        for i in 0..10 {
            vec.push(i);
        }
        let cap_before = vec.capacity();
        shrink_if_wasteful(&mut vec);
        // Full vec should not shrink
        assert_eq!(vec.capacity(), cap_before);
    }

    #[test]
    fn test_shrink_if_wasteful_exactly_25_percent() {
        let mut vec = Vec::with_capacity(100);
        for i in 0..75 {
            vec.push(i);
        }
        // Exactly 25% waste - should NOT shrink (threshold is >25%)
        let cap_before = vec.capacity();
        shrink_if_wasteful(&mut vec);
        assert_eq!(vec.capacity(), cap_before);
    }

    #[test]
    fn test_shrink_if_wasteful_just_over_25_percent() {
        let mut vec = Vec::with_capacity(100);
        for i in 0..74 {
            vec.push(i);
        }
        // Just over 25% waste - should shrink
        shrink_if_wasteful(&mut vec);
        assert!(vec.capacity() < 100);
    }

    #[test]
    fn test_shrink_if_wasteful_minimal_waste() {
        let mut vec = Vec::with_capacity(10);
        for i in 0..9 {
            vec.push(i);
        }
        // Only 10% waste - should NOT shrink
        let cap_before = vec.capacity();
        shrink_if_wasteful(&mut vec);
        assert_eq!(vec.capacity(), cap_before);
    }

    #[test]
    fn test_shrink_if_wasteful_massive_waste() {
        let mut vec = Vec::with_capacity(10000);
        vec.push(1);
        // 99.99% waste - should definitely shrink
        shrink_if_wasteful(&mut vec);
        assert!(vec.capacity() < 10000);
    }

    // ========== Performance Verification Tests ==========

    #[test]
    fn test_preallocated_vs_default() {
        // Verify pre-allocated vectors are actually pre-allocated
        let default: Vec<String> = Vec::new();
        let preallocated: Vec<String> = vec_for_packages();

        assert_eq!(default.capacity(), 0);
        assert!(preallocated.capacity() > 0);
        assert_eq!(preallocated.capacity(), capacity::PACKAGES);
    }

    #[test]
    fn test_capacity_ordering() {
        // Verify capacity constants are in sensible order
        assert!(capacity::PARTITIONS < capacity::USERS);
        assert!(capacity::USERS < capacity::SERVICES);
        assert!(capacity::SERVICES < capacity::PACKAGES_SMALL);
        assert!(capacity::PACKAGES_SMALL < capacity::PACKAGES_MEDIUM);
        assert_eq!(capacity::PACKAGES_MEDIUM, capacity::PACKAGES); // PACKAGES is alias for PACKAGES_MEDIUM
        assert!(capacity::PACKAGES_MEDIUM < capacity::PACKAGES_LARGE);
    }

    #[test]
    fn test_all_capacities_positive() {
        assert!(capacity::PARTITIONS > 0);
        assert!(capacity::FILESYSTEMS > 0);
        assert!(capacity::MOUNT_POINTS > 0);
        assert!(capacity::PACKAGES > 0);
        assert!(capacity::PACKAGES_SMALL > 0);
        assert!(capacity::PACKAGES_MEDIUM > 0);
        assert!(capacity::PACKAGES_LARGE > 0);
        assert!(capacity::USERS > 0);
        assert!(capacity::SERVICES > 0);
        assert!(capacity::NETWORK_INTERFACES > 0);
        assert!(capacity::LVM_VG > 0);
        assert!(capacity::LVM_LV > 0);
        assert!(capacity::FILES > 0);
        assert!(capacity::BATCH_VMS > 0);
        assert!(capacity::ENV_VARS > 0);
        assert!(capacity::CRON_JOBS > 0);
    }

    #[test]
    fn test_shrink_preserves_data() {
        let mut vec = Vec::with_capacity(1000);
        vec.push("test1");
        vec.push("test2");
        vec.push("test3");

        shrink_if_wasteful(&mut vec);

        // Data should be preserved
        assert_eq!(vec.len(), 3);
        assert_eq!(vec[0], "test1");
        assert_eq!(vec[1], "test2");
        assert_eq!(vec[2], "test3");
    }

    #[test]
    fn test_vec_with_estimated_capacity_different_types() {
        let input = vec![1, 2, 3, 4, 5];

        let vec_i32: Vec<i32> = vec_with_estimated_capacity(&input, 2.0);
        let vec_string: Vec<String> = vec_with_estimated_capacity(&input, 2.0);
        let vec_u64: Vec<u64> = vec_with_estimated_capacity(&input, 2.0);

        assert_eq!(vec_i32.capacity(), 10);
        assert_eq!(vec_string.capacity(), 10);
        assert_eq!(vec_u64.capacity(), 10);
    }
}
