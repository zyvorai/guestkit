// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guest customization / desired-state (spec §14 subset).
//!
//! Concrete, validated, policy-gated identity operations: hostname,
//! timezone, and DNS resolvers. Each returns the prior value so the caller
//! can record an undo. Off by default (`actions.customization.enabled`).

use anyhow::{bail, Context, Result};
use serde_json::{json, Value};
use std::process::Command;

fn valid_hostname(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 253
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '.')
        && !name.starts_with('-')
        && !name.starts_with('.')
}

pub fn set_hostname(params: &Value) -> Result<Value> {
    let hostname = params
        .get("hostname")
        .and_then(Value::as_str)
        .context("missing required param: hostname")?;
    if !valid_hostname(hostname) {
        bail!("invalid hostname: {hostname}");
    }
    let previous = current_hostname();

    let status = if cfg!(target_os = "windows") {
        Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!("Rename-Computer -NewName '{hostname}' -Force"),
            ])
            .status()
    } else {
        // hostnamectl is the canonical path; falls back to `hostname` + file.
        Command::new("hostnamectl")
            .args(["set-hostname", hostname])
            .status()
    }?;
    if !status.success() {
        bail!("set hostname failed: {status}");
    }
    crate::agent::audit::audit("customization.hostname", "ok", hostname);
    Ok(json!({ "hostname": hostname, "previous": previous }))
}

fn current_hostname() -> Option<String> {
    Command::new("hostname")
        .output()
        .ok()
        .filter(|o| o.status.success())
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
}

pub fn set_timezone(params: &Value) -> Result<Value> {
    let tz = params
        .get("timezone")
        .and_then(Value::as_str)
        .context("missing required param: timezone")?;
    // Timezone names are path components; keep them strict.
    if tz.is_empty()
        || tz.contains("..")
        || !tz
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '/' | '_' | '-' | '+'))
    {
        bail!("invalid timezone: {tz}");
    }
    let previous = Command::new("readlink")
        .arg("/etc/localtime")
        .output()
        .ok()
        .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());

    let status = if cfg!(target_os = "windows") {
        Command::new("tzutil").args(["/s", tz]).status()
    } else {
        Command::new("timedatectl")
            .args(["set-timezone", tz])
            .status()
    }?;
    if !status.success() {
        bail!("set timezone failed: {status}");
    }
    crate::agent::audit::audit("customization.timezone", "ok", tz);
    Ok(json!({ "timezone": tz, "previous": previous }))
}

pub fn set_dns(params: &Value) -> Result<Value> {
    let servers: Vec<String> = params
        .get("servers")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if servers.is_empty() {
        bail!("missing required param: servers (non-empty array)");
    }
    for s in &servers {
        if s.parse::<std::net::IpAddr>().is_err() {
            bail!("invalid DNS server address: {s}");
        }
    }

    if cfg!(target_os = "windows") {
        // Apply to all up adapters.
        let list = servers
            .iter()
            .map(|s| format!("'{s}'"))
            .collect::<Vec<_>>()
            .join(",");
        let script = format!(
            "Get-NetAdapter | Where-Object Status -eq 'Up' | \
             Set-DnsClientServerAddress -ServerAddresses {list}"
        );
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()?;
        if !status.success() {
            bail!("set DNS failed: {status}");
        }
    } else {
        // Prefer resolvectl on systemd-resolved hosts; otherwise write
        // /etc/resolv.conf with a backup.
        let default_iface = default_interface();
        let resolvectl_ok = if let Some(iface) = &default_iface {
            let mut args = vec!["dns".to_string(), iface.clone()];
            args.extend(servers.iter().cloned());
            Command::new("resolvectl")
                .args(&args)
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        } else {
            false
        };
        if !resolvectl_ok {
            let path = std::path::Path::new("/etc/resolv.conf");
            if path.exists() {
                let _ = std::fs::copy(path, "/etc/resolv.conf.guestkit-bak");
            }
            let content: String = servers
                .iter()
                .map(|s| format!("nameserver {s}\n"))
                .collect();
            std::fs::write(path, content).context("write /etc/resolv.conf")?;
        }
    }
    crate::agent::audit::audit("customization.dns", "ok", &servers.join(","));
    Ok(json!({ "servers": servers }))
}

#[cfg(target_os = "linux")]
fn default_interface() -> Option<String> {
    let out = Command::new("ip")
        .args(["route", "show", "default"])
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    text.split_whitespace()
        .skip_while(|w| *w != "dev")
        .nth(1)
        .map(str::to_string)
}

#[cfg(not(target_os = "linux"))]
fn default_interface() -> Option<String> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hostname_validation() {
        assert!(valid_hostname("web-01.example.com"));
        assert!(!valid_hostname("bad host!"));
        assert!(!valid_hostname(""));
        assert!(!valid_hostname("-leading"));
    }

    #[test]
    fn dns_rejects_bad_address() {
        let err = set_dns(&json!({"servers": ["not-an-ip"]})).unwrap_err();
        assert!(err.to_string().contains("invalid DNS server"));
    }

    #[test]
    fn timezone_rejects_traversal() {
        let err = set_timezone(&json!({"timezone": "../../etc/passwd"})).unwrap_err();
        assert!(err.to_string().contains("invalid timezone"));
    }
}
