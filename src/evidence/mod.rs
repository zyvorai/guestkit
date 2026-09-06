// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Normalized evidence snapshot — digital twin primitive for migration assurance.

#[cfg(not(target_os = "windows"))]
pub mod builder;
pub mod collectors;
pub mod snapshot;

#[cfg(feature = "agent")]
pub mod live;

#[cfg(not(target_os = "windows"))]
pub use builder::{build_evidence, EvidenceBuilder};
#[cfg(feature = "agent")]
pub use live::{build_agent_status_live, build_evidence_live, AgentStatus};
pub use snapshot::{
    BootEvidence, CloudInitEvidence, EvidenceSnapshot, KubevirtEvidence, NetworkEvidence,
    NetworkProbeEvidence, OsEvidence, PackageEvidence, SecurityEvidence, SnapshotReadinessEvidence,
    StorageEvidence, SystemdInfo, SystemdJob, SystemdProblemHint, SystemdProblemSeverity,
    SystemdRuntimeInfo, SystemdRuntimeUnit, SystemdUnit, SystemdUnitState, VirtioDiskEntry,
    VmToolsEvidence, WindowsAppEntry, WindowsEvidence, WindowsServiceEntry, WindowsStartType,
    SCHEMA_VERSION,
};
