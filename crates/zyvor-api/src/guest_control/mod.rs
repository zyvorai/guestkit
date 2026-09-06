// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Zyvor Guest Control Fabric — transport-independent guest control.

pub mod capabilities;
pub mod doctor;
pub mod envelope;
pub mod install;
pub mod polling;
pub mod routes;
pub mod transport;

pub use capabilities::{
    ControlState, GuestCapabilityContract, GuestCapabilitySupports, GuestTransport,
    TransportAttempt,
};
pub use doctor::{run_agent_doctor, AgentDoctorReport, DoctorNode};
pub use envelope::GuestControlEnvelope;
pub use install::{install_agent_strategy, InstallAgentRequest, InstallStrategy};
pub use transport::{pull_method, probe_guest_context, GuestContext, PullResult};
