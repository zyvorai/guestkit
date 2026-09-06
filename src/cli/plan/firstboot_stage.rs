// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline staging for `ServiceOperation` and `CommandExec`.
//!
//! - `systemctl enable` / `disable` → wants Symlink / FileDelete (immediate).
//! - `start` / `restart` and arbitrary commands → append to a first-boot oneshot
//!   that runs on next boot (`guestkit-firstboot-live.service`).

use anyhow::{Context, Result};

use crate::cli::plan::types::{CommandExec, ServiceOperation};

const SCRIPT: &str = "/usr/lib/guestkit/firstboot-live.sh";
const UNIT: &str = "/etc/systemd/system/guestkit-firstboot-live.service";
const WANTS: &str = "/etc/systemd/system/multi-user.target.wants/guestkit-firstboot-live.service";
const PENDING: &str = "/var/lib/guestkit/firstboot-live.cmds";

/// Apply a service op offline: enable/disable via systemd wants; start/restart staged.
pub fn stage_service_offline(
    g: &mut crate::guestfs::Guestfs,
    so: &ServiceOperation,
) -> Result<bool> {
    let unit = normalize_unit(&so.service);
    let mut did = false;

    if let Some(state) = so.state.as_deref() {
        let s = state.trim().to_ascii_lowercase();
        if s == "enabled" || s == "enable" {
            enable_unit(g, &unit)?;
            did = true;
        } else if s == "disabled" || s == "disable" {
            disable_unit(g, &unit)?;
            did = true;
        }
    }

    let mut cmds = Vec::new();
    if so.start {
        cmds.push(format!("systemctl start {unit}"));
    }
    if so.restart {
        cmds.push(format!("systemctl restart {unit}"));
    }
    if !cmds.is_empty() {
        append_firstboot_cmds(g, &cmds)?;
        did = true;
        eprintln!(
            "Staged first-boot service action(s) for {unit}: {}",
            cmds.join("; ")
        );
    }

    if !did {
        eprintln!(
            "Warning: ServiceOperation ({}) had no enable/disable/start/restart — nothing to apply",
            so.service
        );
        return Ok(false);
    }
    Ok(true)
}

/// Prefer chroot when it works; otherwise stage the command for first boot.
pub fn apply_or_stage_command(g: &mut crate::guestfs::Guestfs, ce: &CommandExec) -> Result<bool> {
    // Offline-safe systemctl enable/disable without chroot.
    if let Some(action) = parse_systemctl_enable_disable(&ce.command) {
        match action {
            SysctlAction::Enable(u) => {
                enable_unit(g, &u)?;
                eprintln!("Offline enable via wants symlink: {u}");
                return Ok(true);
            }
            SysctlAction::Disable(u) => {
                disable_unit(g, &u)?;
                eprintln!("Offline disable via wants removal: {u}");
                return Ok(true);
            }
        }
    }

    // Try immediate chroot (works for simple file tools; often fails for systemctl).
    let args = match shell_words(&ce.command) {
        Ok(a) if !a.is_empty() => a,
        _ => vec!["sh".into(), "-c".into(), ce.command.clone()],
    };
    let arg_refs: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
    match g.command(&arg_refs) {
        Ok(_) => Ok(true),
        Err(e) => {
            eprintln!(
                "Warning: chroot command failed ({e}); staging for first-boot: {}",
                ce.command
            );
            let line = if let Some(interp) = &ce.interpreter {
                format!("{interp} {}", shell_quote(&ce.command))
            } else {
                format!("sh -c {}", shell_quote(&ce.command))
            };
            append_firstboot_cmds(g, &[line])?;
            Ok(true)
        }
    }
}

/// Preview note for ServiceOperation.
pub fn service_preview_note(so: &ServiceOperation) -> String {
    let mut parts = Vec::new();
    if let Some(state) = &so.state {
        let s = state.to_ascii_lowercase();
        if s.contains("enable") {
            parts.push("offline: wants symlink");
        } else if s.contains("disable") {
            parts.push("offline: remove wants");
        }
    }
    if so.start || so.restart {
        parts.push("offline: first-boot systemctl start/restart");
    }
    if parts.is_empty() {
        "offline: no-op".into()
    } else {
        format!("offline: {}", parts.join("; "))
    }
}

/// Preview note for CommandExec.
pub fn command_preview_note(ce: &CommandExec) -> String {
    if parse_systemctl_enable_disable(&ce.command).is_some() {
        "offline: systemctl enable/disable → wants Symlink/FileDelete".into()
    } else {
        "offline: try chroot; else first-boot oneshot".into()
    }
}

fn enable_unit(g: &mut crate::guestfs::Guestfs, unit: &str) -> Result<()> {
    let wants_dir = "/etc/systemd/system/multi-user.target.wants";
    g.mkdir_p(wants_dir)
        .map_err(|e| anyhow::anyhow!("mkdir {wants_dir}: {e}"))?;
    let link = format!("{wants_dir}/{unit}");
    let target = format!("../{unit}");
    g.ln_sf(&target, &link)
        .or_else(|_| g.ln_sf(&format!("/etc/systemd/system/{unit}"), &link))
        .map_err(|e| anyhow::anyhow!("enable {unit}: {e}"))?;
    Ok(())
}

fn disable_unit(g: &mut crate::guestfs::Guestfs, unit: &str) -> Result<()> {
    let link = format!("/etc/systemd/system/multi-user.target.wants/{unit}");
    if g.exists(&link).unwrap_or(false) {
        g.rm(&link)
            .map_err(|e| anyhow::anyhow!("disable remove {link}: {e}"))?;
    }
    Ok(())
}

fn append_firstboot_cmds(g: &mut crate::guestfs::Guestfs, cmds: &[String]) -> Result<()> {
    g.mkdir_p("/var/lib/guestkit")
        .map_err(|e| anyhow::anyhow!("mkdir /var/lib/guestkit: {e}"))?;
    g.mkdir_p("/usr/lib/guestkit")
        .map_err(|e| anyhow::anyhow!("mkdir /usr/lib/guestkit: {e}"))?;
    g.mkdir_p("/etc/systemd/system")
        .map_err(|e| anyhow::anyhow!("mkdir systemd: {e}"))?;
    g.mkdir_p("/etc/systemd/system/multi-user.target.wants")
        .map_err(|e| anyhow::anyhow!("mkdir wants: {e}"))?;

    let mut existing = String::new();
    if let Ok(bytes) = g.read_file(PENDING) {
        existing = String::from_utf8_lossy(&bytes).into_owned();
    }
    if !existing.is_empty() && !existing.ends_with('\n') {
        existing.push('\n');
    }
    for c in cmds {
        existing.push_str(c);
        existing.push('\n');
    }
    g.write(PENDING, existing.as_bytes())
        .with_context(|| format!("write {PENDING}"))?;

    g.write(SCRIPT, firstboot_script().as_bytes())
        .with_context(|| format!("write {SCRIPT}"))?;
    let _ = g.chmod(0o755, SCRIPT);

    g.write(UNIT, firstboot_unit().as_bytes())
        .with_context(|| format!("write {UNIT}"))?;

    g.ln_sf("../guestkit-firstboot-live.service", WANTS)
        .or_else(|_| g.ln_sf("/etc/systemd/system/guestkit-firstboot-live.service", WANTS))
        .map_err(|e| anyhow::anyhow!("enable firstboot-live unit: {e}"))?;
    Ok(())
}

fn firstboot_script() -> String {
    format!(
        r#"#!/bin/bash
set -euo pipefail
PENDING={PENDING}
if [[ ! -f "$PENDING" ]]; then
  exit 0
fi
while IFS= read -r line || [[ -n "$line" ]]; do
  [[ -z "$line" || "$line" =~ ^# ]] && continue
  eval "$line" || true
done < "$PENDING"
rm -f "$PENDING"
systemctl disable guestkit-firstboot-live.service >/dev/null 2>&1 || true
rm -f {WANTS}
"#
    )
}

fn firstboot_unit() -> String {
    format!(
        r#"[Unit]
Description=GuestKit first-boot staged live commands
After=local-fs.target network-online.target
Wants=network-online.target
ConditionPathExists={PENDING}

[Service]
Type=oneshot
ExecStart={SCRIPT}
RemainAfterExit=yes

[Install]
WantedBy=multi-user.target
"#
    )
}

fn normalize_unit(name: &str) -> String {
    let n = name.trim();
    if n.ends_with(".service") || n.ends_with(".socket") || n.ends_with(".timer") {
        n.to_string()
    } else {
        format!("{n}.service")
    }
}

enum SysctlAction {
    Enable(String),
    Disable(String),
}

fn parse_systemctl_enable_disable(cmd: &str) -> Option<SysctlAction> {
    let parts: Vec<&str> = cmd.split_whitespace().collect();
    // systemctl [--…] enable|disable UNIT
    let is_systemctl = parts.first().map(|s| *s == "systemctl").unwrap_or(false)
        || (parts.len() >= 2 && parts[0].ends_with("systemctl"));
    let mut i = if is_systemctl { 1usize } else { return None };
    while i < parts.len() && parts[i].starts_with('-') {
        i += 1;
    }
    if i + 1 >= parts.len() {
        return None;
    }
    let action = parts[i];
    let unit = normalize_unit(parts[i + 1]);
    match action {
        "enable" => Some(SysctlAction::Enable(unit)),
        "disable" => Some(SysctlAction::Disable(unit)),
        _ => None,
    }
}

fn shell_words(s: &str) -> Result<Vec<String>> {
    // Minimal split: reuse apply's style — whitespace with simple quotes.
    let mut out = Vec::new();
    let mut cur = String::new();
    let mut in_single = false;
    let mut in_double = false;
    for ch in s.chars() {
        match ch {
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            c if c.is_whitespace() && !in_single && !in_double => {
                if !cur.is_empty() {
                    out.push(std::mem::take(&mut cur));
                }
            }
            c => cur.push(c),
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    Ok(out)
}

fn shell_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_enable_disable() {
        match parse_systemctl_enable_disable("systemctl enable firewalld").unwrap() {
            SysctlAction::Enable(u) => assert_eq!(u, "firewalld.service"),
            _ => panic!(),
        }
        match parse_systemctl_enable_disable("systemctl --no-reload disable ssh").unwrap() {
            SysctlAction::Disable(u) => assert_eq!(u, "ssh.service"),
            _ => panic!(),
        }
        assert!(parse_systemctl_enable_disable("echo hi").is_none());
    }

    #[test]
    fn normalize_adds_service() {
        assert_eq!(normalize_unit("sshd"), "sshd.service");
        assert_eq!(normalize_unit("ssh.service"), "ssh.service");
    }
}
