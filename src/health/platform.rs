// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Platform-specific live guest health assembly.

use anyhow::Result;
use guestkit_agent_protocol::GuestHealth;

pub fn build_guest_health_live() -> Result<GuestHealth> {
    #[cfg(target_os = "windows")]
    {
        let hostname = std::env::var("COMPUTERNAME").unwrap_or_else(|_| "windows-host".into());
        Ok(crate::collectors::windows_live::build_windows_guest_health(
            &hostname,
        ))
    }
    #[cfg(not(target_os = "windows"))]
    {
        let evidence = crate::evidence::build_evidence_live()?;
        Ok(crate::health::build_guest_health(&evidence))
    }
}
