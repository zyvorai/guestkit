// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Serial console without `grubby`.
//!
//! Photon and many other guests do not ship `grubby`. Chrooting that
//! binary exits 127 and staging it for first boot fails the same way.
//! When `grubby` is absent, kernel args are written into the bootloader
//! files the guest already uses.

use anyhow::Result;

use crate::guestfs::Guestfs;

pub const SERIAL_ARGS: &str = "console=ttyS0,115200 console=tty0";

const GRUBBY_PATHS: &[&str] = &["/usr/sbin/grubby", "/usr/bin/grubby", "/sbin/grubby"];

const GRUB_CFG: &[&str] = &["/boot/grub2/grub.cfg", "/boot/grub/grub.cfg"];

/// True for the migration repair command that used to call `grubby` directly.
pub fn is_grubby_serial(command: &str) -> bool {
    let first = command.split_whitespace().next().unwrap_or("");
    let bin = first.rsplit('/').next().unwrap_or(first);
    bin == "grubby" && command.contains("console=")
}

/// Bootloaders we can update when `grubby` is not installed.
const FILE_MARKERS: &[(&str, &str)] = &[
    ("/boot/grub2/grub.cfg", "grub2"),
    ("/boot/grub/grub.cfg", "grub2"),
    ("/etc/default/grub", "grub2"),
    ("/boot/grub/menu.lst", "grub-legacy"),
    ("/boot/loader/loader.conf", "systemd-boot"),
    ("/boot/syslinux/syslinux.cfg", "syslinux"),
    ("/syslinux.cfg", "syslinux"),
    ("/boot/extlinux/extlinux.conf", "extlinux"),
    ("/etc/zipl.conf", "zipl"),
];

const DIR_MARKERS: &[(&str, &str)] = &[("/boot/loader/entries", "bls")];

const BIN_MARKERS: &[(&str, &str)] = &[
    ("/usr/sbin/grub2-mkconfig", "grub2"),
    ("/usr/bin/grub2-mkconfig", "grub2"),
    ("/usr/sbin/grub-mkconfig", "grub2"),
    ("/usr/bin/grub-mkconfig", "grub2"),
    ("/usr/sbin/update-grub", "grub2"),
    ("/usr/bin/update-grub", "grub2"),
    ("/usr/bin/bootctl", "systemd-boot"),
    ("/usr/sbin/zipl", "zipl"),
    ("/usr/bin/zipl", "zipl"),
    ("/usr/sbin/extlinux", "extlinux"),
    ("/usr/bin/extlinux", "extlinux"),
    ("/usr/sbin/syslinux", "syslinux"),
    ("/usr/bin/syslinux", "syslinux"),
];

/// Names of bootloaders present, in probe order. `files` and `dirs` are paths that exist.
pub fn classify_bootloaders(files: &[&str], dirs: &[&str]) -> Vec<&'static str> {
    let mut names = Vec::new();
    for (path, name) in FILE_MARKERS.iter().chain(BIN_MARKERS.iter()) {
        if files.contains(path) && !names.contains(name) {
            names.push(*name);
        }
    }
    for (path, name) in DIR_MARKERS {
        if dirs.contains(path) && !names.contains(name) {
            names.push(*name);
        }
    }
    names
}

fn detect_bootloaders(g: &mut Guestfs) -> Vec<&'static str> {
    let mut files = Vec::new();
    for (path, _) in FILE_MARKERS.iter().chain(BIN_MARKERS.iter()) {
        if g.is_file(path).unwrap_or(false) {
            files.push(*path);
        }
    }
    let mut dirs = Vec::new();
    for (path, _) in DIR_MARKERS {
        if g.is_dir(path).unwrap_or(false) {
            dirs.push(*path);
        }
    }
    classify_bootloaders(&files, &dirs)
}

/// Set the serial console. Uses `grubby` only when that binary is in the guest.
/// Otherwise finds grub2, grub-legacy, systemd-boot, BLS, syslinux, extlinux, or zipl.
pub fn apply_serial_console(g: &mut Guestfs) -> Result<bool> {
    if guest_has_grubby(g) {
        match g.command(&[
            "grubby",
            "--update-kernel=ALL",
            "--args=console=ttyS0,115200 console=tty0",
        ]) {
            Ok(_) => {
                eprintln!("Serial console set with grubby");
                return Ok(true);
            }
            Err(e) => {
                eprintln!("grubby failed ({e}); looking for another bootloader");
            }
        }
    }

    let names = detect_bootloaders(g);
    if names.is_empty() {
        eprintln!("No grubby and no known bootloader (grub2, grub-legacy, systemd-boot, syslinux, extlinux, zipl)");
        return Ok(true);
    }

    let mut changed = 0usize;
    for name in &names {
        changed += apply_bootloader(g, name)?;
    }
    eprintln!(
        "Serial console written via {} ({changed} files). grubby is not in this guest.",
        names.join(", ")
    );
    Ok(true)
}

fn apply_bootloader(g: &mut Guestfs, name: &str) -> Result<usize> {
    let mut changed = 0usize;
    match name {
        "grub2" => {
            changed += patch(g, "/etc/default/grub", inject_grub_default)?;
            for path in GRUB_CFG {
                changed += patch(g, path, inject_grub_cfg)?;
            }
            changed += patch_efi_grub(g)?;
            try_grub_mkconfig(g);
        }
        "grub-legacy" => {
            changed += patch(g, "/boot/grub/menu.lst", inject_menu_lst)?;
        }
        "systemd-boot" | "bls" => {
            changed += patch(g, "/etc/kernel/cmdline", inject_plain_cmdline)?;
            changed += patch_bls_entries(g)?;
        }
        "syslinux" => {
            changed += patch(g, "/boot/syslinux/syslinux.cfg", inject_syslinux)?;
            changed += patch(g, "/syslinux.cfg", inject_syslinux)?;
        }
        "extlinux" => {
            changed += patch(g, "/boot/extlinux/extlinux.conf", inject_syslinux)?;
        }
        "zipl" => {
            changed += patch(g, "/etc/zipl.conf", inject_zipl)?;
        }
        _ => {}
    }
    Ok(changed)
}

fn patch_efi_grub(g: &mut Guestfs) -> Result<usize> {
    if !g.is_dir("/boot/efi/EFI").unwrap_or(false) {
        return Ok(0);
    }
    let vendors = g.ls("/boot/efi/EFI").unwrap_or_else(|_| Vec::new());
    let mut changed = 0usize;
    for vendor in vendors {
        let path = format!("/boot/efi/EFI/{vendor}/grub.cfg");
        changed += patch(g, &path, inject_grub_cfg)?;
    }
    Ok(changed)
}

fn patch_bls_entries(g: &mut Guestfs) -> Result<usize> {
    if !g.is_dir("/boot/loader/entries").unwrap_or(false) {
        return Ok(0);
    }
    let entries = g.ls("/boot/loader/entries").unwrap_or_else(|_| Vec::new());
    let mut changed = 0usize;
    for name in entries {
        if name.ends_with(".conf") {
            changed += patch(g, &format!("/boot/loader/entries/{name}"), inject_bls)?;
        }
    }
    Ok(changed)
}

fn try_grub_mkconfig(g: &mut Guestfs) {
    let tools = [
        ("/usr/sbin/grub2-mkconfig", Some("/boot/grub2/grub.cfg")),
        ("/usr/bin/grub2-mkconfig", Some("/boot/grub2/grub.cfg")),
        ("/usr/sbin/grub-mkconfig", Some("/boot/grub/grub.cfg")),
        ("/usr/bin/grub-mkconfig", Some("/boot/grub/grub.cfg")),
        ("/usr/sbin/update-grub", None),
        ("/usr/bin/update-grub", None),
    ];
    for (bin, out) in tools {
        if !g.is_file(bin).unwrap_or(false) {
            continue;
        }
        let result = if let Some(cfg) = out {
            g.command(&[bin, "-o", cfg])
        } else {
            g.command(&[bin])
        };
        match result {
            Ok(_) => eprintln!("Regenerated GRUB config with {}", bin.rsplit('/').next().unwrap_or(bin)),
            Err(e) => eprintln!(
                "{} failed ({e}); kernel lines were already written into the GRUB config",
                bin.rsplit('/').next().unwrap_or(bin)
            ),
        }
        return;
    }
}

fn guest_has_grubby(g: &mut Guestfs) -> bool {
    GRUBBY_PATHS
        .iter()
        .any(|p| g.is_file(p).unwrap_or(false) || g.exists(p).unwrap_or(false))
}

fn patch(g: &mut Guestfs, path: &str, rewrite: fn(&str) -> String) -> Result<usize> {
    if !g.is_file(path).unwrap_or(false) {
        return Ok(0);
    }
    let old = match g.cat(path) {
        Ok(s) => s,
        Err(_) => return Ok(0),
    };
    let new = rewrite(&old);
    if new == old {
        return Ok(0);
    }
    g.write(path, new.as_bytes())
        .map_err(|e| anyhow::anyhow!("write {path}: {e}"))?;
    Ok(1)
}

pub fn inject_grub_default(text: &str) -> String {
    let mut lines: Vec<String> = text.lines().map(rewrite_grub_cmdline_line).collect();
    let has_cmdline = lines.iter().any(|l| {
        let t = l.trim_start();
        t.starts_with("GRUB_CMDLINE_LINUX=") || t.starts_with("GRUB_CMDLINE_LINUX_DEFAULT=")
    });
    if !has_cmdline {
        lines.push(format!("GRUB_CMDLINE_LINUX=\"{SERIAL_ARGS}\""));
    }
    let terminal = lines
        .iter()
        .position(|l| l.trim_start().starts_with("GRUB_TERMINAL_OUTPUT="));
    if let Some(i) = terminal {
        let ln = &lines[i];
        let t = ln.trim_start();
        if !t.contains("serial") {
            let indent = &ln[..ln.len() - t.len()];
            lines[i] = format!("{indent}GRUB_TERMINAL_OUTPUT=\"serial console\"");
        }
    } else {
        lines.push("GRUB_TERMINAL_OUTPUT=\"serial console\"".to_string());
    }
    if !lines
        .iter()
        .any(|l| l.trim_start().starts_with("GRUB_SERIAL_COMMAND="))
    {
        lines.push(
            "GRUB_SERIAL_COMMAND=\"serial --speed=115200 --unit=0 --word=8 --parity=no --stop=1\""
                .to_string(),
        );
    }
    let mut body = lines.join("\n");
    body.push('\n');
    body
}

fn rewrite_grub_cmdline_line(line: &str) -> String {
    let trimmed = line.trim_start();
    let key = if trimmed.starts_with("GRUB_CMDLINE_LINUX_DEFAULT=") {
        "GRUB_CMDLINE_LINUX_DEFAULT="
    } else if trimmed.starts_with("GRUB_CMDLINE_LINUX=") {
        "GRUB_CMDLINE_LINUX="
    } else {
        return line.to_string();
    };
    let indent = &line[..line.len() - trimmed.len()];
    let rest = trimmed[key.len()..].trim();
    let (quote, inner) = split_quotes(rest);
    if inner.contains("console=ttyS0") {
        return line.to_string();
    }
    let inner = append_args(inner);
    match quote {
        Some(q) => format!("{indent}{key}{q}{inner}{q}"),
        None => format!("{indent}{key}\"{inner}\""),
    }
}

pub fn inject_plain_cmdline(text: &str) -> String {
    let line = text.lines().next().unwrap_or("").trim();
    if line.contains("console=ttyS0") {
        return text.to_string();
    }
    let next = append_args(line);
    if text.ends_with('\n') || text.is_empty() {
        format!("{next}\n")
    } else {
        next
    }
}

pub fn inject_bls(text: &str) -> String {
    rewrite_lines(text, |ln| {
        let t = ln.trim_start();
        if t.starts_with("options ") && !t.contains("console=ttyS0") {
            format!("{ln} {SERIAL_ARGS}")
        } else {
            ln.to_string()
        }
    })
}

pub fn inject_grub_cfg(text: &str) -> String {
    rewrite_lines(text, |ln| {
        let t = ln.trim_start();
        let linux = t.starts_with("linux ")
            || t.starts_with("linux16 ")
            || t.starts_with("linuxefi ");
        if linux && !t.contains("console=ttyS0") {
            format!("{ln} {SERIAL_ARGS}")
        } else {
            ln.to_string()
        }
    })
}

pub fn inject_menu_lst(text: &str) -> String {
    rewrite_lines(text, |ln| append_if_prefix(ln, &["kernel ", "append "]))
}

pub fn inject_syslinux(text: &str) -> String {
    rewrite_lines(text, |ln| {
        let t = ln.trim_start();
        if t.len() >= 7 && t[..7].eq_ignore_ascii_case("append ") && !t.contains("console=ttyS0") {
            format!("{ln} {SERIAL_ARGS}")
        } else {
            ln.to_string()
        }
    })
}

pub fn inject_zipl(text: &str) -> String {
    rewrite_lines(text, |ln| {
        let t = ln.trim_start();
        let key = t
            .split_once('=')
            .map(|(k, _)| k.trim())
            .unwrap_or("");
        if !key.eq_ignore_ascii_case("parameters") || t.contains("console=ttyS0") {
            return ln.to_string();
        }
        let indent = &ln[..ln.len() - t.len()];
        let value = t.split_once('=').map(|(_, v)| v.trim()).unwrap_or("");
        let (quote, inner) = split_quotes(value);
        let inner = append_args(inner);
        match quote {
            Some(q) => format!("{indent}parameters={q}{inner}{q}"),
            None if value.is_empty() => format!("{indent}parameters=\"{SERIAL_ARGS}\""),
            None => format!("{indent}parameters=\"{inner}\""),
        }
    })
}

fn append_if_prefix(line: &str, prefixes: &[&str]) -> String {
    let t = line.trim_start();
    if prefixes.iter().any(|p| t.starts_with(p)) && !t.contains("console=ttyS0") {
        format!("{line} {SERIAL_ARGS}")
    } else {
        line.to_string()
    }
}

fn rewrite_lines(text: &str, f: impl Fn(&str) -> String) -> String {
    if text.is_empty() {
        return String::new();
    }
    let mut out = text.lines().map(f).collect::<Vec<_>>().join("\n");
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn append_args(cmdline: &str) -> String {
    let t = cmdline.trim();
    if t.is_empty() {
        SERIAL_ARGS.to_string()
    } else if t.contains("console=ttyS0") {
        t.to_string()
    } else {
        format!("{t} {SERIAL_ARGS}")
    }
}

fn split_quotes(value: &str) -> (Option<char>, &str) {
    let b = value.as_bytes();
    if b.len() >= 2 {
        let q = b[0] as char;
        if (q == '"' || q == '\'') && value.ends_with(q) {
            return (Some(q), &value[1..value.len() - 1]);
        }
    }
    (None, value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_grubby_serial_command() {
        assert!(is_grubby_serial(
            "grubby --update-kernel=ALL --args='console=ttyS0,115200 console=tty0'"
        ));
        assert!(!is_grubby_serial("grub2-mkconfig -o /boot/grub2/grub.cfg"));
    }

    #[test]
    fn default_grub_gains_console_inside_quotes() {
        let src = "GRUB_CMDLINE_LINUX=\"quiet\"\n";
        let out = inject_grub_default(src);
        assert!(out.contains("GRUB_CMDLINE_LINUX=\"quiet console=ttyS0,115200 console=tty0\""));
        assert!(out.contains("GRUB_TERMINAL_OUTPUT=\"serial console\""));
        assert!(out.contains("GRUB_SERIAL_COMMAND="));
        assert_eq!(inject_grub_default(&out), out);
    }

    #[test]
    fn finds_the_bootloader_that_is_installed() {
        let names = classify_bootloaders(
            &["/boot/grub2/grub.cfg", "/usr/sbin/grub2-mkconfig"],
            &[],
        );
        assert_eq!(names, vec!["grub2"]);

        let names = classify_bootloaders(
            &["/boot/syslinux/syslinux.cfg"],
            &["/boot/loader/entries"],
        );
        assert_eq!(names, vec!["syslinux", "bls"]);
    }

    #[test]
    fn other_bootloaders_gain_console_args() {
        assert!(inject_menu_lst("kernel /vmlinuz root=/dev/sda1\n").contains("console=ttyS0"));
        assert!(inject_syslinux("APPEND quiet\n").contains("console=ttyS0"));
        let zipl = inject_zipl("[Linux]\nparameters=\"root=/dev/dasda1\"\n");
        assert!(zipl.contains("parameters=\"root=/dev/dasda1 console=ttyS0,115200 console=tty0\""));
    }
}
