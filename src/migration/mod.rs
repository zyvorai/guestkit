// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Migration readiness assessment and repair planning.
//!
//! Checks run against [`crate::evidence::EvidenceSnapshot`], so the engine
//! serves both offline images and the live in-guest agent. Boot probes are
//! reused from [`crate::boot`] via wrapper checks rather than re-run.

pub mod baseline;
pub mod checks;
pub mod readiness;
pub mod repair;
pub mod score;
/// Live cutover workflow — needs the in-guest agent runtime.
#[cfg(feature = "agent")]
pub mod workflow;

pub use readiness::{AssessContext, MigrationCheckResult, ReadinessCategory, RemediationHint};
pub use repair::{MigrationRepairPlanner, RepairOptions};
pub use score::{assess_migration, MigrationAssessment, MigrationSubScores, ReadinessLevel};
