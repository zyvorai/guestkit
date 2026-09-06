// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! User and access inventory (spec §13, read-only surface).
//!
//! Enumerates local accounts and highlights access risks (empty passwords,
//! dormant privileged accounts, root SSH login, sudoers members). Never
//! reads or exposes password hashes.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LocalUser {
    pub name: String,
    pub uid: u32,
    pub gid: u32,
    pub home: String,
    pub shell: String,
    pub is_system: bool,
    /// True when the account can log in (has a real shell, not nologin/false).
    pub can_login: bool,
}

#[cfg(target_os = "linux")]
pub fn inventory() -> Value {
    use std::fs;

    let passwd = fs::read_to_string("/etc/passwd").unwrap_or_default();
    let shadow = fs::read_to_string("/etc/shadow").ok();
    let mut users = Vec::new();
    let mut empty_password = Vec::new();
    let mut login_shells = 0usize;

    for line in passwd.lines() {
        let f: Vec<&str> = line.split(':').collect();
        if f.len() < 7 {
            continue;
        }
        let name = f[0].to_string();
        let uid: u32 = f[2].parse().unwrap_or(0);
        let shell = f[6].to_string();
        let can_login =
            !shell.ends_with("nologin") && !shell.ends_with("/false") && !shell.is_empty();
        if can_login {
            login_shells += 1;
        }
        users.push(LocalUser {
            name: name.clone(),
            uid,
            gid: f[3].parse().unwrap_or(0),
            home: f[4].to_string(),
            shell,
            is_system: uid < 1000 && uid != 0,
            can_login,
        });

        // Empty-password detection via /etc/shadow (field 2 empty).
        if let Some(shadow) = &shadow {
            if let Some(entry) = shadow.lines().find(|l| l.starts_with(&format!("{name}:"))) {
                let sf: Vec<&str> = entry.split(':').collect();
                if sf.len() > 1 && sf[1].is_empty() {
                    empty_password.push(name.clone());
                }
            }
        }
    }

    // Active sessions.
    let sessions: Vec<String> = std::process::Command::new("who")
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();

    // sudo group membership (wheel/sudo).
    let group = fs::read_to_string("/etc/group").unwrap_or_default();
    let sudoers: Vec<String> = group
        .lines()
        .filter(|l| l.starts_with("sudo:") || l.starts_with("wheel:"))
        .filter_map(|l| l.split(':').nth(3))
        .flat_map(|members| {
            members
                .split(',')
                .filter(|m| !m.is_empty())
                .map(str::to_string)
        })
        .collect();

    // Root SSH login policy.
    let root_ssh_login = fs::read_to_string("/etc/ssh/sshd_config")
        .ok()
        .and_then(|c| {
            c.lines()
                .map(str::trim)
                .filter(|l| !l.starts_with('#'))
                .filter_map(|l| l.strip_prefix("PermitRootLogin"))
                .map(|v| v.trim().to_lowercase())
                .next()
        })
        .unwrap_or_else(|| "default".into());

    // authorized_keys count per human user (dormant-key surface).
    let mut authorized_keys = 0usize;
    for user in users.iter().filter(|u| u.uid >= 1000 || u.uid == 0) {
        let ak = format!("{}/.ssh/authorized_keys", user.home);
        if let Ok(content) = fs::read_to_string(&ak) {
            authorized_keys += content
                .lines()
                .filter(|l| l.starts_with("ssh-") || l.starts_with("ecdsa-"))
                .count();
        }
    }

    json!({
        "user_count": users.len(),
        "login_capable": login_shells,
        "users": users,
        "active_sessions": sessions,
        "sudoers": sudoers,
        "empty_password_accounts": empty_password,
        "root_ssh_login": root_ssh_login,
        "authorized_keys_total": authorized_keys,
    })
}

#[cfg(target_os = "windows")]
pub fn inventory() -> Value {
    let ps = |script: &str| -> Option<String> {
        std::process::Command::new("powershell")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string())
    };
    let users: Vec<String> =
        ps("Get-LocalUser -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name")
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default();
    let admins: Vec<String> = ps(
        "Get-LocalGroupMember -Group Administrators -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name",
    )
    .map(|s| s.lines().map(str::to_string).collect())
    .unwrap_or_default();
    let rdp_users: Vec<String> = ps(
        "Get-LocalGroupMember -Group 'Remote Desktop Users' -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Name",
    )
    .map(|s| s.lines().map(str::to_string).collect())
    .unwrap_or_default();
    json!({
        "user_count": users.len(),
        "users": users,
        "local_administrators": admins,
        "rdp_users": rdp_users,
    })
}

#[cfg(not(any(target_os = "linux", target_os = "windows")))]
pub fn inventory() -> Value {
    json!({ "user_count": 0, "users": [] })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn inventory_reads_passwd() {
        let inv = inventory();
        // Every Linux host has at least root.
        assert!(inv["user_count"].as_u64().unwrap_or(0) >= 1);
        assert!(inv["users"]
            .as_array()
            .unwrap()
            .iter()
            .any(|u| u["name"] == "root"));
    }
}
