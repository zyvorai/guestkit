// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fleet clustering and anomaly detection.

pub mod analyzer;
pub mod baseline;
pub mod quarantine;
pub mod report;
pub mod wave;

pub use analyzer::analyze_fleet;
pub use baseline::FleetBaseline;
pub use quarantine::{quarantine_fleet, QuarantineReport};
pub use report::FleetAnalysisReport;
pub use wave::{plan_waves, MigrationWave, VmRole, WaveMember, WavePlan};
