// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Live guest runtime metrics for VMRogue monitoring and Copilot.

#[cfg(feature = "agent")]
pub mod live;

#[cfg(feature = "agent")]
pub use live::{collect_metrics_live, GuestMetricsSnapshot};
