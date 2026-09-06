// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! D-Bus collectors for systemd and related services.

#[cfg(target_os = "linux")]
pub mod login1;
#[cfg(target_os = "linux")]
pub mod resolve1;
#[cfg(target_os = "linux")]
pub mod systemd1;
#[cfg(target_os = "linux")]
pub mod systemd_events;
#[cfg(target_os = "linux")]
pub mod timedate1;

#[cfg(target_os = "linux")]
pub use systemd1::collect_systemd_runtime;
#[cfg(target_os = "linux")]
pub use systemd1::{get_unit_by_pid, get_unit_detail, list_failed_units, restart_unit};

#[cfg(target_os = "linux")]
pub use login1::collect_login_state;
#[cfg(target_os = "linux")]
pub use resolve1::collect_dns_health;
#[cfg(target_os = "linux")]
pub use timedate1::collect_timedate_health;

use crate::evidence::snapshot::SystemdRuntimeInfo;
use guestkit_agent_protocol::{DnsHealth, LoginState, TimedateHealth};

/// Collect systemd runtime state (Linux D-Bus or empty on other platforms).
pub fn collect_systemd_runtime_safe() -> Option<SystemdRuntimeInfo> {
    #[cfg(target_os = "linux")]
    {
        collect_systemd_runtime()
    }
    #[cfg(not(target_os = "linux"))]
    {
        None
    }
}

pub fn collect_login_state_safe() -> LoginState {
    #[cfg(target_os = "linux")]
    {
        collect_login_state().unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        LoginState::default()
    }
}

pub fn collect_timedate_health_safe() -> TimedateHealth {
    #[cfg(target_os = "linux")]
    {
        collect_timedate_health().unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        TimedateHealth::default()
    }
}

pub fn collect_dns_health_safe() -> DnsHealth {
    #[cfg(target_os = "linux")]
    {
        collect_dns_health().unwrap_or_default()
    }
    #[cfg(not(target_os = "linux"))]
    {
        DnsHealth::default()
    }
}
