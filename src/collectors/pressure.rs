// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! PSI pressure from /proc/pressure.

use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PressureEvidence {
    pub cpu_some: Option<String>,
    pub memory_some: Option<String>,
    pub io_some: Option<String>,
}

pub fn collect_pressure() -> PressureEvidence {
    PressureEvidence {
        cpu_some: read_pressure("/proc/pressure/cpu"),
        memory_some: read_pressure("/proc/pressure/memory"),
        io_some: read_pressure("/proc/pressure/io"),
    }
}

fn read_pressure(path: &str) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|s| s.lines().next().unwrap_or("").trim().to_string())
        .filter(|s| !s.is_empty())
}
