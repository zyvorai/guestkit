// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

pub mod analyze;
pub mod live;
pub mod sd_journal;

#[cfg(all(target_os = "linux", feature = "journal-native"))]
pub mod sd_journal_native;

#[cfg(target_os = "windows")]
pub mod windows_event;
