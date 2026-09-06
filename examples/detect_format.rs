// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Example: Detect disk image format

use guestkit::converters::DiskConverter;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    env_logger::init();

    let converter = DiskConverter::new();

    // Detect format
    let format = converter.detect_format(Path::new("/path/to/disk.img"))?;

    println!("Detected format: {}", format.as_str());

    // Get detailed info
    let info = converter.get_info(Path::new("/path/to/disk.img"))?;
    println!("\nDetailed info:");
    println!("{}", serde_json::to_string_pretty(&info)?);

    Ok(())
}
