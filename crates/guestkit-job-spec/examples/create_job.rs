// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Creating and validating job documents

use guestkit_job_spec::builder::{inspect_job, JobBuilder};
use guestkit_job_spec::JobValidator;

fn main() -> anyhow::Result<()> {
    println!("=== Creating Job Documents ===\n");

    // Example 1: Minimal job using builder
    println!("1. Minimal Inspection Job:");
    let minimal_job = JobBuilder::new()
        .generate_job_id()
        .operation("guestkit.inspect")
        .payload(
            "guestkit.inspect.v1",
            serde_json::json!({
                "image": {
                    "path": "/vms/test.qcow2",
                    "format": "qcow2"
                }
            }),
        )
        .build()?;

    println!("{}\n", serde_json::to_string_pretty(&minimal_job)?);

    // Example 2: Full-featured job using helper and builder
    println!("2. Full-Featured Inspection Job:");
    let full_job = inspect_job("/vms/production-web-01.qcow2")
        .name("weekly-security-scan")
        .namespace("production")
        .label("environment", "prod")
        .label("team", "platform")
        .annotation("ticket", "INC-12345")
        .priority(8)
        .timeout_seconds(7200)
        .max_attempts(3)
        .idempotency_key("weekly-scan-2026-w04")
        .require_capability("guestkit.inspect")
        .require_capability("disk.qcow2")
        .require_feature("lvm")
        .require_feature("selinux")
        .worker_pool("pool-prod")
        .trace_id("550e8400-e29b-41d4-a716-446655440001")
        .submitted_by("api-service")
        .build()?;

    println!("{}\n", serde_json::to_string_pretty(&full_job)?);

    // Example 3: Validate jobs
    println!("3. Validating Jobs:");
    match JobValidator::validate(&minimal_job) {
        Ok(_) => println!("✓ Minimal job is valid"),
        Err(e) => println!("✗ Minimal job validation failed: {}", e),
    }

    match JobValidator::validate(&full_job) {
        Ok(_) => println!("✓ Full job is valid"),
        Err(e) => println!("✗ Full job validation failed: {}", e),
    }

    // Example 4: Check capability matching
    println!("\n4. Capability Matching:");
    let required = vec!["guestkit.inspect".to_string(), "disk.qcow2".to_string()];
    let available = vec![
        "guestkit.inspect".to_string(),
        "guestkit.profile".to_string(),
        "disk.qcow2".to_string(),
        "disk.vmdk".to_string(),
    ];

    match JobValidator::check_capabilities(&required, &available) {
        Ok(_) => println!("✓ Worker has required capabilities"),
        Err(e) => println!("✗ Capability check failed: {}", e),
    }

    // Example with missing capability
    let required_with_missing = vec![
        "guestkit.inspect".to_string(),
        "windows-registry".to_string(),
    ];

    match JobValidator::check_capabilities(&required_with_missing, &available) {
        Ok(_) => println!("✓ Worker has required capabilities"),
        Err(e) => println!("✗ Expected failure: {}", e),
    }

    // Example 5: Serialization roundtrip
    println!("\n5. Serialization Roundtrip:");
    let json = serde_json::to_string_pretty(&full_job)?;
    let deserialized = serde_json::from_str(&json)?;

    if full_job == deserialized {
        println!("✓ Serialization roundtrip successful");
    } else {
        println!("✗ Serialization roundtrip failed");
    }

    Ok(())
}
