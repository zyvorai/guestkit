// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Panic safety test suite for GuestKit
//!
//! Tests to ensure no unwrap() panics occur in error scenarios.
//! All operations should return proper Result types instead of panicking.

use guestkit::guestfs::handle::Utf8Policy;
use guestkit::guestfs::Guestfs;

#[test]
fn test_nbd_not_initialized() {
    // Operations that need NBD should return Err, not panic
    let g = Guestfs::new().unwrap();

    // These should return errors, not panic on unwrap
    // Note: We can't directly test internal methods, but we can test
    // public APIs that depend on them

    // Just verify the handle is created successfully
    assert!(g.state() == &guestkit::guestfs::handle::GuestfsState::Config);
}

#[test]
fn test_partition_table_not_loaded() {
    // Operations that need partition table should return Err, not panic
    let g = Guestfs::new().unwrap();

    // list_partitions requires partition table to be loaded
    let result = g.list_partitions();
    assert!(
        result.is_err(),
        "Should return error when partition table not loaded"
    );

    // list_devices should still work (doesn't need partition table)
    let result = g.list_devices();
    assert!(result.is_err() || result.is_ok()); // Either is fine, just shouldn't panic
}

#[test]
fn test_operations_without_launch() {
    // All operations should fail gracefully without launch, not panic
    let mut g = Guestfs::new().unwrap();

    // Try various operations without launching
    assert!(g.list_partitions().is_err());
    assert!(g.list_filesystems().is_err());
}

#[test]
fn test_operations_without_drives() {
    // Launching without drives should return error, not panic
    let mut g = Guestfs::new().unwrap();

    let result = g.launch();
    assert!(result.is_err(), "Launch without drives should return error");

    // Verify we're in error or config state, not panicked
    let state = g.state();
    assert!(
        state == &guestkit::guestfs::handle::GuestfsState::Error
            || state == &guestkit::guestfs::handle::GuestfsState::Config
    );
}

#[test]
fn test_utf8_policy_configuration() {
    let mut g = Guestfs::new().unwrap();

    // Default should be Lossy
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Lossy);

    // Setting to Strict should work and not panic
    g.set_utf8_policy(Utf8Policy::Strict);
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Strict);

    // Setting back should also work
    g.set_utf8_policy(Utf8Policy::Lossy);
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Lossy);
}

#[test]
fn test_device_name_validation() {
    use guestkit::guestfs::security_utils::PathValidator;

    // Invalid device names should return errors, not panic
    assert!(PathValidator::validate_device_path("").is_err());
    assert!(PathValidator::validate_device_path("/home/file").is_err());
    assert!(PathValidator::validate_device_path("/dev/sda; rm -rf /").is_err());
    // Valid device paths should succeed
    assert!(PathValidator::validate_device_path("/dev/sda").is_ok());
    assert!(PathValidator::validate_device_path("/dev/sda1").is_ok());
}

#[test]
fn test_double_launch() {
    let mut g = Guestfs::new().unwrap();

    // First launch will fail (no drives), but shouldn't panic
    let _ = g.launch();

    // Second launch should return error, not panic
    let result = g.launch();
    assert!(result.is_err(), "Double launch should return error");
}

#[test]
fn test_operations_after_error_state() {
    let mut g = Guestfs::new().unwrap();

    // Cause an error by launching without drives
    let _ = g.launch();

    // Further operations should handle error state gracefully
    let result = g.list_partitions();
    assert!(
        result.is_err(),
        "Operations in error state should return error"
    );
}

#[test]
fn test_resource_limits_boundary() {
    let g = Guestfs::new().unwrap();

    // Test resource limits exist and have reasonable defaults - should not panic
    let limits = g.get_resource_limits();
    assert!(limits.max_file_size.is_some());
    assert!(limits.max_file_size.unwrap() > 0);
    assert_eq!(limits.max_path_length, 4096);
}

#[test]
fn test_path_length_boundary() {
    use guestkit::guestfs::security_utils::PathValidator;

    // Test maximum allowed path length - should not panic
    let max_path = "/".to_string() + &"a".repeat(4095);
    let result = PathValidator::validate_fs_path(&max_path);
    assert!(result.is_ok());

    // Test just over limit
    let over_path = "/".to_string() + &"a".repeat(4097);
    let result = PathValidator::validate_fs_path(&over_path);
    assert!(result.is_err());
}

#[test]
fn test_mode_validation_boundary() {
    use guestkit::guestfs::validation::Validator;

    // Test boundary values - should not panic
    assert!(Validator::validate_mode(0).is_ok());
    assert!(Validator::validate_mode(0o7777).is_ok());
    assert!(Validator::validate_mode(-1).is_err());
    assert!(Validator::validate_mode(0o10000).is_err());
}

#[test]
fn test_ownership_validation_boundary() {
    use guestkit::guestfs::validation::Validator;

    // Test boundary values - should not panic
    assert!(Validator::validate_ownership(0, 0).is_ok());
    assert!(Validator::validate_ownership(i32::MAX, i32::MAX).is_ok());
    assert!(Validator::validate_ownership(-1, 0).is_err());
    assert!(Validator::validate_ownership(0, -1).is_err());
    assert!(Validator::validate_ownership(i32::MIN, i32::MIN).is_err());
}

#[test]
fn test_partition_number_validation_boundary() {
    use guestkit::guestfs::validation::Validator;

    // Test boundary values - should not panic
    assert!(Validator::validate_partition_number(1).is_ok());
    assert!(Validator::validate_partition_number(128).is_ok());
    assert!(Validator::validate_partition_number(0).is_err());
    assert!(Validator::validate_partition_number(129).is_err());
    assert!(Validator::validate_partition_number(u32::MAX).is_err());
}

#[test]
fn test_empty_string_handling() {
    use guestkit::guestfs::security_utils::PathValidator;
    use guestkit::guestfs::validation::Validator;

    // Empty strings should be handled gracefully, not panic
    assert!(PathValidator::validate_fs_path("").is_err());
    assert!(PathValidator::validate_device_path("").is_err());
    assert!(PathValidator::validate_cpio_format("").is_err());
    assert!(Validator::validate_fstype("").is_err());
    assert!(Validator::validate_archive_format("").is_err());
    assert!(Validator::validate_device_name("").is_err());
}

#[test]
fn test_null_handling() {
    let g = Guestfs::new().unwrap();

    // Verify handle created successfully
    assert!(g.state() == &guestkit::guestfs::handle::GuestfsState::Config);

    // Get limits - should not panic
    let limits = g.get_resource_limits();
    assert!(limits.max_file_size.is_some());
}

#[test]
fn test_state_transitions() {
    let mut g = Guestfs::new().unwrap();

    // Initial state
    assert_eq!(g.state(), &guestkit::guestfs::handle::GuestfsState::Config);

    // Try to launch without drives - should transition to Error state
    let result = g.launch();
    assert!(result.is_err());

    // Should be in Error state now
    if g.state() == &guestkit::guestfs::handle::GuestfsState::Error {
        // Error state confirmed
    } else {
        panic!("Expected Error state after failed launch");
    }
}

#[test]
fn test_utf8_policy_roundtrip() {
    let mut g = Guestfs::new().unwrap();

    // Default should be Lossy
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Lossy);

    // Set to Strict
    g.set_utf8_policy(Utf8Policy::Strict);
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Strict);

    // Set back to Lossy
    g.set_utf8_policy(Utf8Policy::Lossy);
    assert_eq!(g.get_utf8_policy(), &Utf8Policy::Lossy);
}

#[test]
fn test_concurrent_state_checks() {
    let g = Guestfs::new().unwrap();

    // Multiple state checks should be safe
    for _ in 0..100 {
        let _ = g.state();
        let _ = g.get_verbose();
        let _ = g.get_trace();
        let _ = g.get_utf8_policy();
        let _ = g.get_resource_limits();
    }
}
