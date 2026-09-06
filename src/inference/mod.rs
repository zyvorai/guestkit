// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Deterministic root-cause inference engine.

pub mod engine;
pub mod report;

pub use engine::infer_root_cause;
pub use report::RootCauseReport;
