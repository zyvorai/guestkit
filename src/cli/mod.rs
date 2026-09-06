// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! CLI module for guestkit

#[cfg(not(target_os = "windows"))]
pub mod agent_sign;
#[cfg(not(target_os = "windows"))]
pub mod ai;
#[cfg(not(target_os = "windows"))]
pub mod batch;
#[cfg(not(target_os = "windows"))]
pub mod blueprint;
#[cfg(not(target_os = "windows"))]
pub mod cache;
#[cfg(not(target_os = "windows"))]
pub mod commands;
#[cfg(not(target_os = "windows"))]
pub mod commands_list;
#[cfg(not(target_os = "windows"))]
pub mod cost;
#[cfg(not(target_os = "windows"))]
pub mod cutover_cmd;
#[cfg(not(target_os = "windows"))]
pub mod dependencies;
#[cfg(not(target_os = "windows"))]
pub mod diff;
#[cfg(not(target_os = "windows"))]
pub mod domain_disks;
#[cfg(not(target_os = "windows"))]
pub mod entry;
#[cfg(not(target_os = "windows"))]
pub mod errors;
#[cfg(not(target_os = "windows"))]
pub mod exporters;
#[cfg(not(target_os = "windows"))]
pub mod firstboot;
#[cfg(not(target_os = "windows"))]
pub mod forensic_diff;
#[cfg(not(target_os = "windows"))]
pub mod formatters;
#[cfg(not(target_os = "windows"))]
pub mod gate;
#[cfg(not(target_os = "windows"))]
pub mod img;
#[cfg(not(target_os = "windows"))]
pub mod interactive;
#[cfg(not(target_os = "windows"))]
pub mod inventory;
#[cfg(not(target_os = "windows"))]
pub mod invocation;
#[cfg(not(target_os = "windows"))]
pub mod license;
pub mod migrate;
pub mod output;
#[cfg(not(target_os = "windows"))]
pub mod parallel;
pub mod plan;
#[cfg(not(target_os = "windows"))]
pub mod profiles;
#[cfg(not(target_os = "windows"))]
pub mod sbom_diff;
#[cfg(not(target_os = "windows"))]
pub mod shell;
#[cfg(not(target_os = "windows"))]
pub mod tui;
#[cfg(not(target_os = "windows"))]
pub mod validate;
#[cfg(not(target_os = "windows"))]
pub mod virtio_win;
#[cfg(not(target_os = "windows"))]
pub mod welcome;

#[cfg(not(target_os = "windows"))]
pub use batch::*;
#[cfg(not(target_os = "windows"))]
pub use interactive::*;

/// Run the CLI (`guestkit` / `guestctl`). Offline host tooling — Unix only.
#[cfg(not(target_os = "windows"))]
pub fn run() -> anyhow::Result<()> {
    entry::run()
}
