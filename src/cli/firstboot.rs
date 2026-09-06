// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! First-boot attestation: offline doctor + optional live QGA ping.
//!
//! CI: `guestkit firstboot disk.qcow2 --target kvm --fail-below 80 -o firstboot.json`

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::time::Duration;

use super::domain_disks::{self, DomainDisks};
use super::virtio_win::{self, VirtioWinPlan};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirstBootReport {
    pub kind: String,
    pub version: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub image: Option<String>,
    pub target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub offline: Option<OfflineSlice>,
    pub live: LiveSlice,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub virtio: Option<VirtioWinPlan>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disks: Option<DomainDisks>,
    pub ready: bool,
    pub notes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineSlice {
    pub score: f64,
    pub blockers: Vec<String>,
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LiveSlice {
    pub attempted: bool,
    pub qga_ping: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub socket: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

pub struct FirstBootArgs {
    pub image: Option<PathBuf>,
    pub target: String,
    pub socket: Option<String>,
    pub domain: Option<PathBuf>,
    pub virtio_win: Option<PathBuf>,
    pub fail_below: Option<f64>,
    pub output: Option<PathBuf>,
    pub verbose: bool,
}

pub fn run(args: FirstBootArgs) -> Result<()> {
    let report = build(&args)?;
    let json = serde_json::to_string_pretty(&report)?;
    if let Some(path) = args.output.as_ref() {
        std::fs::write(path, format!("{json}\n"))?;
        eprintln!("wrote {}", path.display());
    } else {
        println!("{json}");
    }
    if !report.ready && args.fail_below.is_some() {
        anyhow::bail!("firstboot not ready (score/QGA/virtio gate failed)");
    }
    Ok(())
}

pub fn build(args: &FirstBootArgs) -> Result<FirstBootReport> {
    let mut notes = Vec::new();

    let offline = if let Some(image) = args.image.as_ref() {
        match crate::assurance::run_doctor(image, &args.target, false, args.verbose) {
            Ok(doc) => {
                let blockers = doc
                    .bootability
                    .blockers
                    .iter()
                    .map(|f| format!("{}: {}", f.check_id, f.message))
                    .collect();
                let warnings = doc
                    .bootability
                    .warnings
                    .iter()
                    .map(|f| format!("{}: {}", f.check_id, f.message))
                    .collect();
                Some(OfflineSlice {
                    score: doc.bootability.score,
                    blockers,
                    warnings,
                })
            }
            Err(e) => {
                notes.push(format!("offline doctor failed: {e}"));
                None
            }
        }
    } else {
        notes.push("no --image; offline doctor skipped".into());
        None
    };

    let live = probe_live(args.socket.as_deref());

    let virtio = match virtio_win::discover_tree(args.virtio_win.as_deref()) {
        Ok(root) => Some(virtio_win::plan(&root, args.image.as_deref())),
        Err(e) => {
            notes.push(format!("virtio-win: {e}"));
            None
        }
    };

    let disks = match args.domain.as_ref() {
        Some(p) => match domain_disks::parse_domain_disks(p) {
            Ok(d) => Some(d),
            Err(e) => {
                notes.push(format!("domain-disks: {e}"));
                None
            }
        },
        None => None,
    };

    let score_ok = match (offline.as_ref(), args.fail_below) {
        (Some(o), Some(min)) => o.score + f64::EPSILON >= min && o.blockers.is_empty(),
        (Some(o), None) => o.blockers.is_empty(),
        (None, Some(_)) => false,
        (None, None) => true,
    };
    let virtio_ok = virtio
        .as_ref()
        .map(|v| v.missing.is_empty())
        .unwrap_or(true);
    let live_ok = if live.attempted {
        live.qga_ping == Some(true)
    } else {
        true
    };

    let ready = score_ok && virtio_ok && live_ok;

    Ok(FirstBootReport {
        kind: "guestkit.firstboot".into(),
        version: 1,
        image: args.image.as_ref().map(|p| p.display().to_string()),
        target: args.target.clone(),
        offline,
        live,
        virtio,
        disks,
        ready,
        notes,
    })
}

fn probe_live(socket: Option<&str>) -> LiveSlice {
    #[cfg(all(feature = "agent", unix))]
    {
        let sock = match socket {
            Some(s) if !s.is_empty() => Some(s.to_string()),
            _ => crate::agent::qga_client::discover_qga_socket(&[])
                .map(|p| p.to_string_lossy().into_owned()),
        };
        let Some(sock) = sock else {
            return LiveSlice {
                attempted: socket.is_some(),
                qga_ping: None,
                socket: None,
                error: Some("no QGA socket (pass --socket or start the guest)".into()),
            };
        };
        match crate::agent::qga_client::call_qga_socket(
            &sock,
            "guest-ping",
            None,
            Duration::from_secs(5),
        ) {
            Ok(v) => LiveSlice {
                attempted: true,
                qga_ping: Some(v.get("return").is_some() && v.get("error").is_none()),
                socket: Some(sock),
                error: v.get("error").map(|e| e.to_string()),
            },
            Err(e) => LiveSlice {
                attempted: true,
                qga_ping: Some(false),
                socket: Some(sock),
                error: Some(e.to_string()),
            },
        }
    }
    #[cfg(not(all(feature = "agent", unix)))]
    {
        let _ = Duration::from_secs(1);
        LiveSlice {
            attempted: socket.is_some(),
            qga_ping: None,
            socket: socket.map(|s| s.to_string()),
            error: if socket.is_some() {
                Some("rebuild with --features agent for live QGA ping".into())
            } else {
                None
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ready_without_gates_when_nothing_supplied() {
        let report = build(&FirstBootArgs {
            image: None,
            target: "kvm".into(),
            socket: None,
            domain: None,
            virtio_win: None,
            fail_below: None,
            output: None,
            verbose: false,
        })
        .unwrap();
        assert_eq!(report.kind, "guestkit.firstboot");
        assert!(report.ready);
    }

    #[test]
    fn fail_below_without_image_is_not_ready() {
        let report = build(&FirstBootArgs {
            image: None,
            target: "kvm".into(),
            socket: None,
            domain: None,
            virtio_win: None,
            fail_below: Some(80.0),
            output: None,
            verbose: false,
        })
        .unwrap();
        assert!(!report.ready);
    }
}
