// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit operation handlers
//!
//! These handlers integrate with the guestkit core library to perform
//! actual VM operations.

pub mod agent;
pub mod convert;
pub mod doctor;
pub mod inspect;
pub mod migrate_plan;
pub mod passport;
pub mod profile;
pub mod repair;

pub use agent::{AgentCallHandler, AgentDoctorHandler, AgentEvidenceHandler, AgentFixHandler};
pub use convert::ConvertHandler;
pub use doctor::DoctorHandler;
pub use inspect::InspectHandler;
pub use migrate_plan::MigratePlanHandler;
pub use passport::PassportHandler;
pub use profile::ProfileHandler;
pub use repair::RepairHandler;
