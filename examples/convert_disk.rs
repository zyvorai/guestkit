// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Convert disk image format

use guestkit::converters::DiskConverter;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let converter = DiskConverter::new();

    // Convert VMDK to qcow2
    let result = converter.convert(
        Path::new("/path/to/source.vmdk"),
        Path::new("/path/to/output.qcow2"),
        "qcow2",
        true, // compress
        true, // flatten
    )?;

    if result.success {
        println!("✓ Conversion successful!");
        println!(
            "  Source:  {} ({})",
            result.source_path.display(),
            result.source_format.as_str()
        );
        println!(
            "  Output:  {} ({})",
            result.output_path.display(),
            result.output_format.as_str()
        );
        println!("  Size:    {} bytes", result.output_size);
        println!("  Time:    {:.2}s", result.duration_secs);
    } else {
        eprintln!("✗ Conversion failed: {}", result.error.unwrap_or_default());
        std::process::exit(1);
    }

    Ok(())
}
