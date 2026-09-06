// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cluster VM Copilot — deterministic briefings from live guest-agent evidence.

use axum::extract::{Path, State};
use axum::Json;
use guestkit::assurance::{
    answer_copilot_question, CopilotAction, CopilotInsight, EvidenceDigest, EvidenceHighlight,
    MigrationBriefing,
};
use serde::Deserialize;

use crate::error::ApiResult;
use crate::kubevirt_boot_inspect::{boot_inspect_for_vm, BootInspectInfo};
use crate::models::ApiResponse;
use crate::routes::kubevirt::{fetch_vm, fetch_vmi, get_guest_agent_info, GuestAgentInfo};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ClusterCopilotInput {
    pub guest_agent: Option<GuestAgentInfo>,
    pub boot_inspect: Option<BootInspectInfo>,
}

#[derive(Debug, Deserialize)]
pub struct ClusterAskBody {
    pub question: String,
    pub briefing: Option<MigrationBriefing>,
}

pub async fn cluster_briefing(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    body: Option<Json<ClusterCopilotInput>>,
) -> ApiResult<Json<ApiResponse<MigrationBriefing>>> {
    let input = body.map(|Json(b)| b).unwrap_or_default();
    let info = if let Some(g) = input.guest_agent {
        g
    } else {
        get_guest_agent_info(
            State(state.clone()),
            Path((namespace.clone(), name.clone())),
        )
        .await?
        .0
        .data
    };
    let boot = if let Some(b) = input.boot_inspect {
        Some(b)
    } else {
        boot_inspect_for_vm(&state, &namespace, &name, None)
            .await
            .ok()
    };
    let vm_meta = cluster_vm_meta(&state, &namespace, &name).await;
    let briefing = build_cluster_briefing(&info, name.as_str(), &vm_meta, boot.as_ref());
    Ok(Json(ApiResponse::ok(briefing)))
}

pub async fn cluster_ask(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Json(body): Json<ClusterAskBody>,
) -> ApiResult<Json<ApiResponse<guestkit::assurance::CopilotInsight>>> {
    let briefing = if let Some(b) = body.briefing {
        b
    } else {
        cluster_briefing(State(state), Path((namespace, name)), None)
            .await?
            .0
            .data
    };
    let insight = answer_copilot_question(&body.question, &briefing);
    Ok(Json(ApiResponse::ok(insight)))
}

#[derive(Default)]
pub(crate) struct ClusterVmMeta {
    ip_address: Option<String>,
    root_pvc: Option<String>,
}

async fn cluster_vm_meta(state: &AppState, namespace: &str, name: &str) -> ClusterVmMeta {
    let Some(client) = state.kube.as_ref() else {
        return ClusterVmMeta::default();
    };
    let vm = fetch_vm(client, namespace, name).await;
    let vmi = fetch_vmi(client, namespace, name).await;
    let ip = vmi.as_ref().and_then(|v| {
        v.pointer("/status/interfaces")
            .and_then(|ifs| ifs.as_array())
            .and_then(|arr| {
                arr.iter()
                    .find_map(|i| i.get("ipAddress").and_then(|ip| ip.as_str()))
            })
            .map(String::from)
    });
    let root_pvc = vm
        .as_ref()
        .and_then(crate::kubevirt_boot_inspect::root_pvc_from_vm);
    ClusterVmMeta {
        ip_address: ip,
        root_pvc,
    }
}

pub(crate) fn build_cluster_briefing(
    info: &GuestAgentInfo,
    name: &str,
    meta: &ClusterVmMeta,
    boot: Option<&BootInspectInfo>,
) -> MigrationBriefing {
    let agent_ok = info.agent_connected;
    let running = info.vmi_running;
    let is_win = info.is_windows;
    let os = info
        .os_name
        .as_deref()
        .map(|n| {
            format!("{} {}", n, info.os_version.as_deref().unwrap_or(""))
                .trim()
                .to_string()
        })
        .unwrap_or_else(|| {
            boot.map(|b| b.os_release.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| {
                    if is_win {
                        "Windows".into()
                    } else {
                        "Unknown OS".into()
                    }
                })
        });
    let ips = info
        .interfaces
        .as_ref()
        .map(|ifaces| {
            ifaces
                .iter()
                .filter_map(|i| i.get("ipAddress").and_then(|v| v.as_str()))
                .collect::<Vec<_>>()
                .join(", ")
        })
        .filter(|s| !s.is_empty())
        .or_else(|| meta.ip_address.clone())
        .unwrap_or_else(|| "no IP".into());

    let mut recommended_actions = Vec::new();
    let mut evidence_highlights = Vec::new();
    let (readiness, headline, summary, boot_score, blocker_count, next_workflow): (
        &str,
        String,
        String,
        f64,
        usize,
        &str,
    ) = if !running {
        recommended_actions.push(CopilotAction {
            priority: 1,
            title: "Start VM".into(),
            detail: "Start the VM, then refresh guest info.".into(),
            workflow: "cluster-start".into(),
        });
        recommended_actions.push(CopilotAction {
            priority: 2,
            title: "Open in Zeus".into(),
            detail: "Boot the VM from Zeus OS console.".into(),
            workflow: "cluster-zeus".into(),
        });
        (
            "blocked",
            "VM is not running".into(),
            "Start the VM in Zeus OS before live guest agent inspection.".into(),
            0.0,
            1,
            "cluster-start",
        )
    } else if !agent_ok {
        recommended_actions.push(CopilotAction {
            priority: 1,
            title: if is_win {
                "Install QEMU Guest Agent".into()
            } else {
                "Install GuestKit agent".into()
            },
            detail: if is_win {
                "Open Zeus OS Guest Tools and attach virtio-win.iso.".into()
            } else {
                "Merge GuestKit cloud-init and restart the VM.".into()
            },
            workflow: if is_win {
                "cluster-zeus".into()
            } else {
                "cluster-install-agent".into()
            },
        });
        (
            if is_win { "caution" } else { "blocked" },
            if is_win {
                "Guest agent missing — install virtio-win".into()
            } else {
                "Guest agent not connected".into()
            },
            info.message.clone(),
            if is_win { 45.0 } else { 30.0 },
            1,
            if is_win {
                "cluster-zeus"
            } else {
                "cluster-install-agent"
            },
        )
    } else {
        recommended_actions.push(CopilotAction {
            priority: 1,
            title: "Run boot inspect".into(),
            detail: "Collect offline boot hints from the root PVC when the VM is stopped.".into(),
            workflow: "cluster-boot-inspect".into(),
        });
        recommended_actions.push(CopilotAction {
            priority: 2,
            title: "Export root disk".into(),
            detail: "Copy the cluster root PVC into Zyvor ingest for Doctor, Migrate, and YAML.".into(),
            workflow: "cluster-export-disk".into(),
        });
        (
            "ready",
            format!("Live guest healthy — {os}"),
            format!(
                "{} is reachable at {}. Guest agent {}. Export the root disk for full migration plan and YAML.",
                info.hostname.as_deref().unwrap_or(name),
                ips,
                info.guest_agent_version
                    .as_deref()
                    .map(|v| format!("v{v}"))
                    .unwrap_or_else(|| "connected".into())
            ),
            85.0,
            0,
            "cluster-boot-inspect",
        )
    };

    if let Some(h) = &info.hostname {
        evidence_highlights.push(EvidenceHighlight {
            r#ref: "guest.hostname".into(),
            label: "Hostname".into(),
            detail: h.clone(),
        });
    }
    evidence_highlights.push(EvidenceHighlight {
        r#ref: "guest.os".into(),
        label: "Operating system".into(),
        detail: os.clone(),
    });
    evidence_highlights.push(EvidenceHighlight {
        r#ref: "guest.network".into(),
        label: "Network".into(),
        detail: ips.clone(),
    });
    if let Some(v) = &info.guest_agent_version {
        evidence_highlights.push(EvidenceHighlight {
            r#ref: "guest.agent".into(),
            label: "Guest agent".into(),
            detail: format!("v{v}"),
        });
    }
    if let Some(pvc) = &meta.root_pvc {
        evidence_highlights.push(EvidenceHighlight {
            r#ref: "cluster.root_pvc".into(),
            label: "Root PVC".into(),
            detail: pvc.clone(),
        });
    }
    if let Some(b) = boot {
        let detail = [
            (!b.os_release.is_empty()).then_some(b.os_release.as_str()),
            (!b.bootloader.is_empty()).then_some(b.bootloader.as_str()),
            b.cloud_init_present.then_some("cloud-init"),
        ]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" · ");
        evidence_highlights.push(EvidenceHighlight {
            r#ref: "boot.inspect".into(),
            label: "Boot inspect".into(),
            detail: if detail.is_empty() {
                b.message.clone()
            } else {
                detail
            },
        });
        if b.source == "vm_spec_heuristic" && !b.available {
            recommended_actions.insert(
                0,
                CopilotAction {
                    priority: 1,
                    title: "Stop VM for boot inspect".into(),
                    detail: "Stop the VM, then re-run boot inspect on the root disk.".into(),
                    workflow: "cluster-stop-boot-inspect".into(),
                },
            );
        }
    }

    let mut boot_score = boot_score;
    if boot.map(|b| b.available).unwrap_or(false) && boot_score < 90.0 {
        boot_score = 90.0;
    }

    let insights = vec![
        CopilotInsight {
            id: "agent_status".into(),
            question: "Is the guest agent connected?".into(),
            answer: if agent_ok {
                format!("Yes — {}. {}", info.health, info.message)
            } else {
                format!(
                    "No — {}. {}",
                    info.message,
                    if is_win {
                        "Use Zeus OS Guest Tools (virtio-win)."
                    } else {
                        "Use Install agent to merge GuestKit cloud-init."
                    }
                )
            },
        },
        CopilotInsight {
            id: "blockers".into(),
            question: "What is blocking live inspection?".into(),
            answer: if !running {
                "VM is not in Running phase.".into()
            } else if !agent_ok {
                if is_win {
                    "Windows needs QEMU Guest Agent from virtio-win ISO.".into()
                } else {
                    "Linux needs qemu-guest-agent or GuestKit agent.".into()
                }
            } else {
                "No blockers — live inspection is available.".into()
            },
        },
        CopilotInsight {
            id: "fix_first".into(),
            question: "What should I fix first?".into(),
            answer: recommended_actions
                .first()
                .map(|a| a.detail.clone())
                .unwrap_or_else(|| "Refresh guest info.".into()),
        },
        CopilotInsight {
            id: "ready".into(),
            question: "Is this VM ready for migration tooling?".into(),
            answer: if agent_ok && running {
                "Live agent is connected — export the root disk for full boot score, driver plan, and KubeVirt YAML.".into()
            } else {
                "Resolve guest agent connectivity first, then export the disk for offline Doctor and Migrate workflows.".into()
            },
        },
        CopilotInsight {
            id: "evidence".into(),
            question: "What do we know about this guest?".into(),
            answer: [
                Some(os.clone()),
                info.hostname.clone().map(|h| format!("hostname {h}")),
                Some(format!("IP {ips}")),
                info.guest_agent_version
                    .clone()
                    .map(|v| format!("agent {v}")),
                boot
                    .filter(|b| !b.bootloader.is_empty())
                    .map(|b| format!("{} bootloader", b.bootloader)),
                meta.root_pvc.clone().map(|p| format!("root PVC {p}")),
            ]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>()
            .join(". "),
        },
        CopilotInsight {
            id: "migration_changes".into(),
            question: "What migration steps apply to a live KubeVirt VM?".into(),
            answer: "Live cluster VMs use the guest agent for assurance. Export the root disk to Zyvor ingest and run offline Doctor → Migrate → Provision, then Apply YAML to the cluster.".into(),
        },
    ];

    MigrationBriefing {
        readiness: readiness.into(),
        headline,
        summary,
        boot_score,
        migration_score: None,
        blocker_count,
        warning_count: if agent_ok { 0 } else { 1 },
        evidence_digest: EvidenceDigest {
            os,
            architecture: String::new(),
            bootloader: boot
                .map(|b| b.bootloader.clone())
                .filter(|s| !s.is_empty())
                .unwrap_or_else(|| if running { "live".into() } else { "unknown".into() }),
            root_filesystem: String::new(),
            kernel_count: 0,
            fstab_entries: if boot.map(|b| !b.fstab_valid).unwrap_or(false) {
                0
            } else {
                1
            },
            virtio_modules_loaded: agent_ok,
            vm_tools: if agent_ok {
                vec!["qemu-guest-agent".into()]
            } else {
                vec![]
            },
            selinux: String::new(),
        },
        evidence_highlights,
        recommended_actions,
        insights,
        next_workflow: next_workflow.into(),
    }
}
