// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! VM Operations Job Protocol - Type definitions and validation
//!
//! This crate provides the type definitions for the VM Operations Job Protocol v1.
//! It supports serialization/deserialization and validation of job specifications.

pub mod builder;
pub mod error;
pub mod types;
pub mod validation;

// Re-export main types
pub use builder::JobBuilder;
pub use error::{JobError, JobResult};
pub use types::{
    Audit, Constraints, ExecutionMetrics, ExecutionPolicy, ExecutionSummary, Job, JobDocument,
    JobExecutionError, JobMetadata, JobOutputs, JobResult as JobResultType, JobStatus,
    Observability, Payload, ProgressEvent, Routing, WorkerCapabilities,
};
pub use validation::JobValidator;

/// Protocol version
pub const PROTOCOL_VERSION: &str = "1.0";

/// Operation namespaces
pub mod operations {
    /// Guestkit operations
    pub const GUESTKIT_INSPECT: &str = "guestkit.inspect";
    pub const GUESTKIT_PROFILE: &str = "guestkit.profile";
    pub const GUESTKIT_FIX: &str = "guestkit.fix";
    pub const GUESTKIT_CONVERT: &str = "guestkit.convert";
    pub const GUESTKIT_COMPARE: &str = "guestkit.compare";
    pub const GUESTKIT_AGENT_EVIDENCE: &str = "guestkit.agent.evidence";
    pub const GUESTKIT_AGENT_DOCTOR: &str = "guestkit.agent.doctor";
    pub const GUESTKIT_AGENT_CALL: &str = "guestkit.agent.call";
    pub const GUESTKIT_AGENT_FIX: &str = "guestkit.agent.fix";
    pub const GUESTKIT_DOCTOR: &str = "guestkit.doctor";
    pub const GUESTKIT_MIGRATE_PLAN: &str = "guestkit.migrate-plan";
    pub const GUESTKIT_REPAIR: &str = "guestkit.repair";
    pub const GUESTKIT_PASSPORT: &str = "guestkit.passport";

    /// hyper2kvm operations (future)
    pub const HYPER2KVM_CONVERT: &str = "hyper2kvm.convert";
    pub const HYPER2KVM_VALIDATE: &str = "hyper2kvm.validate";

    /// System operations
    pub const SYSTEM_HEALTH_CHECK: &str = "system.health-check";
    pub const SYSTEM_CAPABILITY_PROBE: &str = "system.capability-probe";
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version() {
        assert_eq!(PROTOCOL_VERSION, "1.0");
    }
}
