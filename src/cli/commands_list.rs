// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Grouped listing of CLI subcommands (`commands` subcommand).

use colored::Colorize;

/// (category title, command names)
const GROUPS: &[(&str, &[&str])] = &[
    (
        "Inspect & report",
        &[
            "inspect",
            "inspect-batch",
            "doctor",
            "migrate-plan",
            "migrate-assess",
            "migrate-repair",
            "passport",
            "diff",
            "forensic-diff",
            "compare",
            "inventory",
            "sbom",
            "sbom-diff",
            "cve",
            "licenses",
        ],
    ),
    (
        "Files & disk",
        &[
            "list",
            "extract",
            "inject",
            "search",
            "grep",
            "cat",
            "checksum",
            "du",
            "find-large",
            "tree",
            "archive",
            "convert",
            "info",
            "img",
            "domain-disks",
            "virtio-win",
            "firstboot",
            "fsck",
            "df",
            "filesystems",
            "packages",
            "snapshots",
        ],
    ),
    (
        "Security & compliance",
        &[
            "scan",
            "secrets",
            "rescue",
            "cleanup",
            "network-audit",
            "compliance",
            "malware",
            "health",
            "audit",
            "repair",
            "policy",
            "cloud-profile",
            "harden",
            "anomaly",
            "recommend",
            "predict",
            "threat-intel",
            "hunt",
            "reconstruct",
            "evolve",
            "verify",
        ],
    ),
    (
        "Migrate & plan",
        &[
            "migrate",
            "migrate-plan",
            "migrate-assess",
            "migrate-repair",
            "passport",
            "cloud-init",
            "gate",
            "selinux-relabel",
            "sysprep",
            "bitlocker",
            "agent-sign",
            "virtio-initramfs",
            "blueprint",
            "plan",
            "cost",
            "dependencies",
            "risk",
            "fleet",
            "agent",
            "agent-proxy",
            "agent-call",
            "qga",
            "vm",
        ],
    ),
    (
        "Systemd",
        &["systemd-journal", "systemd-services", "systemd-boot"],
    ),
    (
        "Interactive",
        &["tui", "shell", "interactive", "explore", "script", "ai"],
    ),
    (
        "Utilities",
        &[
            "cache-clear",
            "cache-stats",
            "completion",
            "commands",
            "version",
            "lvm-clone",
        ],
    ),
];

/// Print grouped subcommands; `bin` is the invoked program name.
pub fn print_grouped_commands(bin: &str) {
    println!();
    println!("{} subcommands (by category):", bin.bold());
    println!();

    for (title, cmds) in GROUPS {
        println!("  {}", title.truecolor(222, 115, 86).bold());
        for cmd in *cmds {
            println!("    {cmd}");
        }
        println!();
    }

    println!(
        "  Run {} for per-command help.",
        format!("{bin} <command> --help").dimmed()
    );
    println!();
}
