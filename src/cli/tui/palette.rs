// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Command palette (`:`) for quick TUI actions.

use super::app::View;

#[derive(Debug, Clone)]
pub struct PaletteCommand {
    pub name: &'static str,
    pub description: &'static str,
}

pub const COMMANDS: &[PaletteCommand] = &[
    PaletteCommand {
        name: "goto dashboard",
        description: "Open dashboard",
    },
    PaletteCommand {
        name: "goto assurance",
        description: "Migration assurance",
    },
    PaletteCommand {
        name: "goto issues",
        description: "Security issues",
    },
    PaletteCommand {
        name: "goto files",
        description: "File browser",
    },
    PaletteCommand {
        name: "goto packages",
        description: "Installed packages",
    },
    PaletteCommand {
        name: "goto profiles",
        description: "Profile reports",
    },
    PaletteCommand {
        name: "goto analytics",
        description: "Analytics & charts",
    },
    PaletteCommand {
        name: "goto timeline",
        description: "System timeline",
    },
    PaletteCommand {
        name: "goto recommendations",
        description: "Recommendations",
    },
    PaletteCommand {
        name: "goto topology",
        description: "System topology",
    },
    PaletteCommand {
        name: "goto network",
        description: "Network configuration",
    },
    PaletteCommand {
        name: "goto services",
        description: "System services",
    },
    PaletteCommand {
        name: "goto databases",
        description: "Databases",
    },
    PaletteCommand {
        name: "goto webservers",
        description: "Web servers",
    },
    PaletteCommand {
        name: "goto security",
        description: "Security features",
    },
    PaletteCommand {
        name: "goto storage",
        description: "Storage & filesystems",
    },
    PaletteCommand {
        name: "goto users",
        description: "User accounts",
    },
    PaletteCommand {
        name: "goto kernel",
        description: "Kernel configuration",
    },
    PaletteCommand {
        name: "goto logs",
        description: "System logs",
    },
    PaletteCommand {
        name: "doctor",
        description: "Run bootability doctor → Assurance",
    },
    PaletteCommand {
        name: "assurance",
        description: "Open Assurance view + run doctor",
    },
    PaletteCommand {
        name: "migrate-plan",
        description: "Score migration for current target",
    },
    PaletteCommand {
        name: "export plan",
        description: "Export fix plan YAML to cwd",
    },
    PaletteCommand {
        name: "apply plan",
        description: "Apply fix plan to disk (optional --skip-backup)",
    },
    PaletteCommand {
        name: "apply plan dry-run",
        description: "Dry-run fix plan application",
    },
    PaletteCommand {
        name: "plan preview",
        description: "Preview fix plan operations (read-only)",
    },
    PaletteCommand {
        name: "export json",
        description: "Export current view as JSON",
    },
    PaletteCommand {
        name: "export html",
        description: "Export security report HTML",
    },
    PaletteCommand {
        name: "refresh",
        description: "Reload current view data",
    },
    PaletteCommand {
        name: "refresh full",
        description: "Full re-inspect of image",
    },
    PaletteCommand {
        name: "compare toggle",
        description: "Toggle comparison mode",
    },
    PaletteCommand {
        name: "pin view",
        description: "Pin current tab",
    },
    PaletteCommand {
        name: "help",
        description: "Toggle help overlay",
    },
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PaletteAction {
    Goto(View),
    AssuranceRun,
    MigratePlan,
    ExportFixPlan,
    ApplyFixPlan,
    ApplyFixPlanDryRun,
    PlanPreview,
    ExportJson,
    ExportHtml,
    Refresh,
    RefreshFull,
    CompareToggle,
    PinView,
    Help,
    Unknown,
}

fn resolve_goto_alias(target: &str) -> Option<View> {
    match target {
        "dash" | "home" => Some(View::Dashboard),
        "pkg" | "pkgs" => Some(View::Packages),
        "svc" => Some(View::Services),
        "sec" => Some(View::Security),
        "prof" => Some(View::Profiles),
        "assur" | "migrate" => Some(View::Assurance),
        "net" => Some(View::Network),
        "fs" => Some(View::Files),
        "rec" => Some(View::Recommendations),
        "topo" => Some(View::Topology),
        "ana" => Some(View::Analytics),
        "web" => Some(View::WebServers),
        "db" => Some(View::Databases),
        other => View::from_name(other),
    }
}

pub fn parse_command(input: &str) -> PaletteAction {
    let cmd = input.trim().to_lowercase();
    match cmd.as_str() {
        "goto dashboard" | "dashboard" | "dash" => PaletteAction::Goto(View::Dashboard),
        "goto assurance" | "assurance" | "doctor" => PaletteAction::AssuranceRun,
        "goto issues" | "issues" => PaletteAction::Goto(View::Issues),
        "goto files" | "files" | "fs" => PaletteAction::Goto(View::Files),
        "goto packages" | "packages" | "pkg" | "pkgs" => PaletteAction::Goto(View::Packages),
        "goto profiles" | "profiles" | "prof" => PaletteAction::Goto(View::Profiles),
        "goto analytics" | "analytics" | "ana" => PaletteAction::Goto(View::Analytics),
        "goto timeline" | "timeline" => PaletteAction::Goto(View::Timeline),
        "goto recommendations" | "recommendations" | "rec" => {
            PaletteAction::Goto(View::Recommendations)
        }
        "goto topology" | "topology" | "topo" => PaletteAction::Goto(View::Topology),
        "goto network" | "network" | "net" => PaletteAction::Goto(View::Network),
        "goto services" | "services" | "svc" => PaletteAction::Goto(View::Services),
        "goto databases" | "databases" | "db" => PaletteAction::Goto(View::Databases),
        "goto webservers" | "webservers" | "web" => PaletteAction::Goto(View::WebServers),
        "goto security" | "security" | "sec" => PaletteAction::Goto(View::Security),
        "goto storage" | "storage" => PaletteAction::Goto(View::Storage),
        "goto users" | "users" => PaletteAction::Goto(View::Users),
        "goto kernel" | "kernel" => PaletteAction::Goto(View::Kernel),
        "goto logs" | "logs" => PaletteAction::Goto(View::Logs),
        "migrate-plan" | "migrate plan" => PaletteAction::MigratePlan,
        "export plan" | "export fix plan" => PaletteAction::ExportFixPlan,
        "apply plan" | "apply fix plan" => PaletteAction::ApplyFixPlan,
        "apply plan dry-run" | "dry-run plan" => PaletteAction::ApplyFixPlanDryRun,
        "plan preview" | "preview plan" => PaletteAction::PlanPreview,
        "export json" => PaletteAction::ExportJson,
        "export html" => PaletteAction::ExportHtml,
        "refresh" => PaletteAction::Refresh,
        "refresh full" => PaletteAction::RefreshFull,
        "compare toggle" | "compare" => PaletteAction::CompareToggle,
        "pin view" | "pin" => PaletteAction::PinView,
        "help" => PaletteAction::Help,
        _ if cmd.starts_with("goto ") => {
            let target = cmd.trim_start_matches("goto ");
            resolve_goto_alias(target)
                .map(PaletteAction::Goto)
                .unwrap_or(PaletteAction::Unknown)
        }
        _ => resolve_goto_alias(&cmd)
            .map(PaletteAction::Goto)
            .unwrap_or(PaletteAction::Unknown),
    }
}

pub fn filtered_commands(query: &str) -> Vec<(&'static str, &'static str)> {
    let q = query.trim().to_lowercase();
    let iter = COMMANDS.iter().filter(|c| {
        q.is_empty()
            || c.name.contains(&q)
            || c.description.to_lowercase().contains(&q)
            || fuzzy_match(&q, c.name)
    });
    iter.map(|c| (c.name, c.description)).collect()
}

fn fuzzy_match(query: &str, name: &str) -> bool {
    if query.is_empty() {
        return true;
    }
    let mut nc = name.chars();
    for qc in query.chars() {
        loop {
            match nc.next() {
                Some(nc) if nc == qc => break,
                Some(_) => continue,
                None => return false,
            }
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_doctor_runs_assurance() {
        assert_eq!(parse_command("doctor"), PaletteAction::AssuranceRun);
        assert_eq!(parse_command("assurance"), PaletteAction::AssuranceRun);
    }

    #[test]
    fn parse_goto_aliases() {
        assert_eq!(parse_command("dash"), PaletteAction::Goto(View::Dashboard));
        assert_eq!(parse_command("pkg"), PaletteAction::Goto(View::Packages));
        assert_eq!(parse_command("goto assurance"), PaletteAction::AssuranceRun);
    }

    #[test]
    fn parse_migrate_and_export_plan() {
        assert_eq!(parse_command("migrate-plan"), PaletteAction::MigratePlan);
        assert_eq!(parse_command("export plan"), PaletteAction::ExportFixPlan);
        assert_eq!(parse_command("plan preview"), PaletteAction::PlanPreview);
    }
}
