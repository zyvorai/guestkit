// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live boot analysis via systemd-analyze.

use guestkit_agent_protocol::{BootAnalysis, BootUnitTiming};
use std::process::Command;

pub fn collect_boot_analysis() -> BootAnalysis {
    let time_out = Command::new("systemd-analyze").arg("time").output().ok();
    let blame_out = Command::new("systemd-analyze").arg("blame").output().ok();
    let chain_out = Command::new("systemd-analyze")
        .arg("critical-chain")
        .output()
        .ok();

    let mut analysis = BootAnalysis::default();

    if let Some(out) = time_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            if line.contains("kernel") {
                analysis.kernel_time_ms = parse_duration_ms(line);
            } else if line.contains("initrd") {
                analysis.initrd_time_ms = parse_duration_ms(line);
            } else if line.contains("userspace") {
                analysis.userspace_time_ms = parse_duration_ms(line);
            }
        }
        analysis.total_boot_time_ms =
            analysis.kernel_time_ms + analysis.initrd_time_ms + analysis.userspace_time_ms;
    }

    if let Some(out) = blame_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines().take(15) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                let time_ms = parse_time_token(parts[0]).unwrap_or(0);
                let name = parts[1..].join(" ");
                analysis.slow_units.push(BootUnitTiming { name, time_ms });
            }
        }
    }

    if let Some(out) = chain_out {
        let text = String::from_utf8_lossy(&out.stdout);
        for line in text.lines() {
            let trimmed = line.trim();
            if trimmed.contains(".service") || trimmed.contains(".target") {
                analysis.critical_chain.push(trimmed.to_string());
            }
        }
    }

    analysis
}

fn parse_duration_ms(line: &str) -> u64 {
    line.split_whitespace()
        .find_map(parse_time_token)
        .unwrap_or(0)
}

fn parse_time_token(token: &str) -> Option<u64> {
    if let Some(ms) = token.strip_suffix("ms") {
        ms.parse().ok()
    } else if let Some(s) = token.strip_suffix('s') {
        s.parse::<f64>().ok().map(|v| (v * 1000.0) as u64)
    } else {
        None
    }
}
