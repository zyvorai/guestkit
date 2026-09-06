// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Remote Desktop enable/disable from inside the guest.

#[cfg(target_os = "windows")]
use serde_json::json;
use serde_json::Value;
use std::process::Command;

pub fn enable_rdp() -> Result<Value, String> {
    #[cfg(target_os = "windows")]
    {
        let ps = r#"
Set-ItemProperty -Path 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 0 -Force
Enable-NetFirewallRule -DisplayGroup 'Remote Desktop' -ErrorAction SilentlyContinue
"#;
        run_powershell(ps)?;
        return Ok(json!({ "enabled": true }));
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = Command::new("true").status();
        Err("enableRdp is only supported on Windows guests".into())
    }
}

pub fn disable_rdp() -> Result<Value, String> {
    #[cfg(target_os = "windows")]
    {
        let ps = r#"
Set-ItemProperty -Path 'HKLM:\System\CurrentControlSet\Control\Terminal Server' -Name fDenyTSConnections -Value 1 -Force
"#;
        run_powershell(ps)?;
        return Ok(json!({ "enabled": false }));
    }
    #[cfg(not(target_os = "windows"))]
    {
        Err("disableRdp is only supported on Windows guests".into())
    }
}

#[cfg(target_os = "windows")]
fn run_powershell(script: &str) -> Result<(), String> {
    let status = Command::new("powershell")
        .args([
            "-NoProfile",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ])
        .status()
        .map_err(|e| format!("powershell: {e}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("powershell exit {status}"))
    }
}
