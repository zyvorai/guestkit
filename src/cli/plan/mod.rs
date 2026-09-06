// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fix plan generation and application module
//!
//! This module provides offline patch & fix preview capabilities:
//! - Generate fix plans from security profiles
//! - Preview changes before applying
//! - Export plans as scripts (bash, ansible)
//! - Apply changes with safety checks
//! - Rollback capabilities

#![allow(unused_imports)]

pub mod apply;
#[cfg(not(target_os = "windows"))]
pub mod cloud_init;
#[cfg(not(target_os = "windows"))]
pub mod command;
#[cfg(not(target_os = "windows"))]
pub mod cutover_prep;
pub mod export;
#[cfg(not(target_os = "windows"))]
pub mod firstboot_stage;
#[cfg(not(target_os = "windows"))]
pub mod generator;
#[cfg(not(target_os = "windows"))]
pub mod linux_boot;
#[cfg(not(target_os = "windows"))]
pub mod package_fetch;
#[cfg(not(target_os = "windows"))]
pub mod package_stage;
#[cfg(not(target_os = "windows"))]
pub mod preview;
pub mod types;

#[cfg(feature = "agent")]
pub mod executor_live;
pub mod topo_sort;

pub use types::{
    FileEdit, FixPlan, Operation, OperationType, PackageInstall, PostApplyAction, Priority,
    RegistryEdit, SELinuxMode, ServiceOperation,
};

pub use apply::ApplyResult;
#[cfg(not(target_os = "windows"))]
pub use apply::PlanApplicator;
#[cfg(not(target_os = "windows"))]
pub use command::PlanCommand;
#[cfg(feature = "agent")]
pub use executor_live::LivePlanExecutor;
pub use export::PlanExporter;
#[cfg(not(target_os = "windows"))]
pub use generator::PlanGenerator;
#[cfg(not(target_os = "windows"))]
pub use preview::PlanPreview;
