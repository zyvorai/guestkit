// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Package and patch management (spec §22).
//!
//! Inventory and update queries are read-only; installation mutates the
//! system and is policy-gated (off by default). Linux uses the native
//! package manager (apt/dnf/zypper); Windows reports installed KBs and
//! pending updates via PowerShell.

use anyhow::{bail, Result};
use serde_json::{json, Value};
use std::path::Path;
use std::process::Command;
use std::time::Duration;

const PROBE_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Manager {
    Apt,
    Dnf,
    Yum,
    Zypper,
    Windows,
    Unknown,
}

fn detect_manager() -> Manager {
    if cfg!(target_os = "windows") {
        return Manager::Windows;
    }
    if Path::new("/usr/bin/apt-get").exists() {
        Manager::Apt
    } else if Path::new("/usr/bin/dnf").exists() {
        Manager::Dnf
    } else if Path::new("/usr/bin/yum").exists() {
        Manager::Yum
    } else if Path::new("/usr/bin/zypper").exists() {
        Manager::Zypper
    } else {
        Manager::Unknown
    }
}

fn run(cmd: &str, args: &[&str]) -> Option<String> {
    let mut child = Command::new(cmd)
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::null())
        .spawn()
        .ok()?;
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => {
                let out = child.wait_with_output().ok()?;
                return Some(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(None) if std::time::Instant::now() > deadline => {
                let _ = child.kill();
                return None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(_) => return None,
        }
    }
}

fn ps(script: &str) -> Option<String> {
    run(
        "powershell",
        &[
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-Command",
            script,
        ],
    )
}

fn manager_name(m: Manager) -> &'static str {
    match m {
        Manager::Apt => "apt",
        Manager::Dnf => "dnf",
        Manager::Yum => "yum",
        Manager::Zypper => "zypper",
        Manager::Windows => "windows-update",
        Manager::Unknown => "unknown",
    }
}

pub fn inventory() -> Value {
    let m = detect_manager();
    let count = match m {
        Manager::Apt => run("dpkg-query", &["-f", "${binary:Package}\n", "-W"])
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
        Manager::Dnf | Manager::Yum => run("rpm", &["-qa"])
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
        Manager::Zypper => run("rpm", &["-qa"])
            .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
            .unwrap_or(0),
        Manager::Windows => ps("(Get-HotFix -ErrorAction SilentlyContinue | Measure-Object).Count")
            .and_then(|s| s.trim().parse().ok())
            .unwrap_or(0),
        Manager::Unknown => 0,
    };

    // Installed kernels (Linux) for migration/reboot reasoning.
    let kernels: Vec<String> = match m {
        Manager::Apt => run(
            "dpkg-query",
            &["-f", "${binary:Package}\n", "-W", "linux-image-*"],
        )
        .map(|s| {
            s.lines()
                .filter(|l| l.starts_with("linux-image-") && l.chars().any(|c| c.is_ascii_digit()))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default(),
        Manager::Dnf | Manager::Yum | Manager::Zypper => run("rpm", &["-q", "kernel"])
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default(),
        _ => Vec::new(),
    };

    let running_kernel = run("uname", &["-r"]).map(|s| s.trim().to_string());

    json!({
        "manager": manager_name(m),
        "installed_count": count,
        "kernels": kernels,
        "running_kernel": running_kernel,
    })
}

pub fn updates() -> Value {
    let m = detect_manager();
    let (available, security, reboot_required) = match m {
        Manager::Apt => apt_updates(),
        Manager::Dnf | Manager::Yum => dnf_updates(m),
        Manager::Zypper => zypper_updates(),
        Manager::Windows => windows_updates(),
        Manager::Unknown => (Vec::new(), 0, false),
    };
    json!({
        "manager": manager_name(m),
        "available_count": available.len(),
        "security_count": security,
        "reboot_required": reboot_required,
        "packages": available.iter().take(200).collect::<Vec<_>>(),
    })
}

fn apt_updates() -> (Vec<String>, usize, bool) {
    // `apt-get -s upgrade` simulates without touching the system.
    let sim = run("apt-get", &["-s", "upgrade"]).unwrap_or_default();
    let available: Vec<String> = sim
        .lines()
        .filter(|l| l.starts_with("Inst "))
        .filter_map(|l| l.split_whitespace().nth(1).map(str::to_string))
        .collect();
    let security = sim
        .lines()
        .filter(|l| l.starts_with("Inst ") && l.to_lowercase().contains("security"))
        .count();
    let reboot = Path::new("/var/run/reboot-required").exists()
        || Path::new("/run/reboot-required").exists();
    (available, security, reboot)
}

fn dnf_updates(m: Manager) -> (Vec<String>, usize, bool) {
    let tool = if m == Manager::Dnf { "dnf" } else { "yum" };
    // check-update exits 100 when updates exist; capture stdout regardless.
    let out = run(tool, &["-q", "check-update"]).unwrap_or_default();
    let available: Vec<String> = out
        .lines()
        .filter(|l| !l.trim().is_empty() && !l.starts_with(' '))
        .filter_map(|l| l.split_whitespace().next())
        .filter(|name| name.contains('.'))
        .map(str::to_string)
        .collect();
    let security = run(tool, &["-q", "updateinfo", "list", "security"])
        .map(|s| s.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0);
    // needs-restarting -r returns 1 when a reboot is required.
    let reboot = Command::new("needs-restarting")
        .arg("-r")
        .status()
        .map(|s| !s.success())
        .unwrap_or(false);
    (available, security, reboot)
}

fn zypper_updates() -> (Vec<String>, usize, bool) {
    let out = run("zypper", &["--quiet", "list-updates"]).unwrap_or_default();
    let available: Vec<String> = out
        .lines()
        .filter(|l| l.starts_with("v |") || l.contains(" | "))
        .filter_map(|l| l.split('|').nth(2).map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty() && s != "Name")
        .collect();
    let security = run(
        "zypper",
        &["--quiet", "list-patches", "--category", "security"],
    )
    .map(|s| s.lines().filter(|l| l.contains("security")).count())
    .unwrap_or(0);
    let reboot = Path::new("/var/run/reboot-required").exists();
    (available, security, reboot)
}

fn windows_updates() -> (Vec<String>, usize, bool) {
    // Query the Windows Update session for pending updates.
    let script = "$s=(New-Object -ComObject Microsoft.Update.Session); \
        $r=$s.CreateUpdateSearcher().Search('IsInstalled=0 and IsHidden=0'); \
        $r.Updates | ForEach-Object { $_.Title }";
    let available: Vec<String> = ps(script)
        .map(|s| {
            s.lines()
                .filter(|l| !l.trim().is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let security = available
        .iter()
        .filter(|t| t.to_lowercase().contains("security"))
        .count();
    let reboot = crate::collectors::windows_live::collect_pending_reboot().0;
    (available, security, reboot)
}

/// Install specific packages. Policy-gated (off by default) and validated:
/// only package names, no shell metacharacters.
pub fn install(params: &Value) -> Result<Value> {
    let packages: Vec<String> = params
        .get("packages")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if packages.is_empty() {
        bail!("missing required param: packages (non-empty array)");
    }
    for pkg in &packages {
        if !pkg
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.' | '+' | ':'))
        {
            bail!("invalid package name: {pkg}");
        }
    }
    let m = detect_manager();
    let refs: Vec<&str> = packages.iter().map(String::as_str).collect();
    let status = match m {
        Manager::Apt => {
            let mut args = vec!["install", "-y"];
            args.extend_from_slice(&refs);
            Command::new("apt-get")
                .args(&args)
                .env("DEBIAN_FRONTEND", "noninteractive")
                .status()
        }
        Manager::Dnf => Command::new("dnf")
            .arg("install")
            .arg("-y")
            .args(&refs)
            .status(),
        Manager::Yum => Command::new("yum")
            .arg("install")
            .arg("-y")
            .args(&refs)
            .status(),
        Manager::Zypper => Command::new("zypper")
            .args(["install", "-y"])
            .args(&refs)
            .status(),
        _ => bail!("package installation not supported on {}", manager_name(m)),
    }?;
    if !status.success() {
        bail!("package install failed: {status}");
    }
    crate::agent::audit::audit("packages.install", "ok", &packages.join(","));
    Ok(json!({ "installed": packages, "manager": manager_name(m) }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_rejects_shell_metachars() {
        let err = install(&json!({"packages": ["nginx; rm -rf /"]})).unwrap_err();
        assert!(err.to_string().contains("invalid package name"));
    }

    #[test]
    fn install_requires_packages() {
        assert!(install(&json!({}))
            .unwrap_err()
            .to_string()
            .contains("packages"));
    }

    #[test]
    fn inventory_is_well_formed() {
        let inv = inventory();
        assert!(inv.get("manager").is_some());
        assert!(inv.get("installed_count").is_some());
    }
}
