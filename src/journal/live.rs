// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live journal collection (sd-journal native on Linux, Event Log on Windows).

use guestkit_agent_protocol::JournalSlice;

#[cfg(not(target_os = "windows"))]
use crate::journal::sd_journal::{collect_journal_slice as collect_sd, BootSelector};

pub fn collect_journal_slice(unit: &str, limit: usize) -> JournalSlice {
    collect_journal_slice_boot(unit, limit, "current")
}

pub fn collect_journal_slice_boot(unit: &str, limit: usize, boot: &str) -> JournalSlice {
    #[cfg(target_os = "windows")]
    {
        let _ = boot;
        crate::journal::windows_event::collect_event_log_slice(unit, limit)
    }
    #[cfg(not(target_os = "windows"))]
    {
        collect_sd(unit, limit, BootSelector::parse(boot))
    }
}
