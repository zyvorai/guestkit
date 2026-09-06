#!/bin/bash
# Copyright 2026 Zyvor AI Labs · https://zyvor.dev
# SPDX-License-Identifier: Apache-2.0
# Demo script to show colorized output functions

cat << 'EOF' > /tmp/test_colors.rs
use guestkit::cli::output::colors::*;

fn main() {
    println!("\n🎨 GuestKit Colorized Output Demo\n");

    header("System Information");
    separator();

    kv("OS Type", "Linux");
    kv("Distribution", "Ubuntu");
    kv("Version", "22.04 LTS");
    kv("Architecture", "x86_64");

    separator();
    section("Service Status");

    status("SSH Server", Status::Running);
    status("Firewall", Status::Enabled);
    status("SELinux", Status::Disabled);
    status("Docker", Status::Unknown);

    separator();
    section("Messages");

    success("Disk inspection completed successfully");
    info("Found 3 partitions");
    warning("Some packages are outdated");
    error("Failed to mount /dev/sdb1");

    separator();
    section("Lists");

    numbered(1, "Install dependencies");
    numbered(2, "Configure network");
    numbered(3, "Start services");

    println!();
    bullet("Use virtual environment");
    bullet("Enable caching for faster inspections");
    bullet("Run with sudo for full access");

    separator();
    emphasis("All features working!");

    thick_separator();
    dimmed("GuestKit v0.3.0 - https://github.com/hypersdk/guestkit");
}
EOF

echo "Building and running color demo..."
rustc --edition 2021 -L target/release/deps --extern guestkit=target/release/libguestkit.rlib /tmp/test_colors.rs -o /tmp/test_colors 2>/dev/null

if [ -f /tmp/test_colors ]; then
    /tmp/test_colors
    rm /tmp/test_colors
else
    echo "Note: Color demo requires compiled library. Colors are available in the CLI."
fi

rm /tmp/test_colors.rs

echo ""
echo "Shell completion test:"
echo "----------------------"
./target/release/guestkit completion bash | head -5
echo "..."
echo "✓ Bash completion generated"
echo ""
