// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Sync VMGuestAgent CR status from guest push reports.

use kube::api::{Api, ApiResource, Patch, PatchParams};
use kube::Client;
use serde_json::{json, Value};

pub fn vmguestagent_resource() -> ApiResource {
    ApiResource {
        group: "zeus.zyvor.dev".into(),
        version: "v1alpha1".into(),
        api_version: "zeus.zyvor.dev/v1alpha1".into(),
        kind: "VMGuestAgent".into(),
        plural: "vmguestagents".into(),
    }
}

/// Migration-readiness fragment for VMGuestAgent status, built from the
/// agent's `guestkit.migration.assess` payload (protocol 1.3). Returns None
/// when the payload lacks assessment fields.
pub fn migration_readiness_status(assessment: &Value) -> Option<Value> {
    let score = assessment.get("overall_score").and_then(|v| v.as_f64())?;
    let readiness = assessment.get("readiness").and_then(|v| v.as_str())?;
    let subs = assessment.get("sub_scores").cloned().unwrap_or(json!({}));
    let blockers: Vec<Value> = assessment
        .get("critical_blockers")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|b| {
                    json!({
                        "checkId": b.get("check_id").and_then(|v| v.as_str()).unwrap_or(""),
                        "title": b.get("title").and_then(|v| v.as_str()).unwrap_or(""),
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    Some(json!({
        "score": score,
        "readiness": readiness,
        "subScores": subs,
        "blockers": blockers,
        "target": assessment.get("target").and_then(|v| v.as_str()).unwrap_or(""),
        "assessedAt": assessment.get("assessed_at").and_then(|v| v.as_str()).unwrap_or(""),
    }))
}

/// Merge protocol-1.3 heartbeat fields into VMGuestAgent status.
pub async fn patch_vmguestagent_heartbeat(
    client: &Client,
    namespace: &str,
    vm_name: &str,
    heartbeat: &Value,
) {
    let cr_name = format!("{vm_name}-vmtools");
    let ar = vmguestagent_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let status = json!({
        "status": {
            "agentState": heartbeat.get("agent_state").and_then(|v| v.as_str()).unwrap_or("unknown"),
            "migrationReady": heartbeat.get("migration_ready").and_then(|v| v.as_bool()).unwrap_or(false),
            "pendingReboot": heartbeat.get("pending_reboot").and_then(|v| v.as_bool()).unwrap_or(false),
            "fsFrozen": heartbeat.get("fs_frozen").and_then(|v| v.as_bool()).unwrap_or(false),
            "lastHeartbeat": chrono::Utc::now().to_rfc3339(),
        }
    });
    let _ = api
        .patch_status(&cr_name, &PatchParams::default(), &Patch::Merge(&status))
        .await;
}

/// Merge a migration assessment into VMGuestAgent status.
pub async fn patch_vmguestagent_migration(
    client: &Client,
    namespace: &str,
    vm_name: &str,
    assessment: &Value,
) {
    let Some(fragment) = migration_readiness_status(assessment) else {
        return;
    };
    let cr_name = format!("{vm_name}-vmtools");
    let ar = vmguestagent_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let status = json!({ "status": { "migrationReadiness": fragment } });
    let _ = api
        .patch_status(&cr_name, &PatchParams::default(), &Patch::Merge(&status))
        .await;
}

/// Patch VMGuestAgent status with live guest health from agent push.
pub async fn patch_vmguestagent_health(
    client: &Client,
    namespace: &str,
    vm_name: &str,
    guest_health: &Value,
    agent_version: &str,
) {
    let cr_name = format!("{vm_name}-vmtools");
    let ar = vmguestagent_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);

    if api.get(&cr_name).await.is_err() {
        let spec_obj = json!({
            "apiVersion": "zeus.zyvor.dev/v1alpha1",
            "kind": "VMGuestAgent",
            "metadata": {
                "name": cr_name,
                "namespace": namespace,
                "labels": { "zeus.zyvor.dev/vm": vm_name }
            },
            "spec": {
                "vmRef": { "namespace": namespace, "name": vm_name },
                "desiredVersion": agent_version,
            }
        });
        if let Ok(obj) = serde_json::from_value::<kube::api::DynamicObject>(spec_obj) {
            let _ = api.create(&kube::api::PostParams::default(), &obj).await;
        }
    }

    let guest_level = guest_health
        .get("guest_health")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let score = guest_health
        .get("score")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u8;
    let failed_units = guest_health
        .get("failed_units")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let systemd_state = guest_health
        .get("systemd_state")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");
    let hostname = guest_health
        .get("vm_hostname")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let journal_hints = guest_health
        .get("journal_hints")
        .and_then(|v| v.as_array())
        .map(|a| a.len())
        .unwrap_or(0);

    let status = json!({
        "status": {
            "installed": true,
            "connected": true,
            "version": agent_version,
            "zyvorAgentReady": true,
            "lastHeartbeat": chrono::Utc::now().to_rfc3339(),
            "guestHealth": guest_level,
            "healthScore": score,
            "failedUnits": failed_units,
            "systemdState": systemd_state,
            "hostname": hostname,
            "journalHintCount": journal_hints,
        }
    });

    let _ = api
        .patch_status(&cr_name, &PatchParams::default(), &Patch::Merge(&status))
        .await;

    patch_vm_health_annotations(client, namespace, vm_name, guest_level, score, failed_units).await;
}

async fn patch_vm_health_annotations(
    client: &Client,
    namespace: &str,
    vm_name: &str,
    guest_level: &str,
    score: u8,
    failed_units: u32,
) {
    let ar = crate::routes::kubevirt::vm_resource();
    let api: Api<kube::api::DynamicObject> = Api::namespaced_with(client.clone(), namespace, &ar);
    let patch = json!({
        "metadata": {
            "annotations": {
                "zeus.zyvor.dev/guest-health": guest_level,
                "zeus.zyvor.dev/health-score": score.to_string(),
                "zeus.zyvor.dev/failed-units": failed_units.to_string(),
                "zeus.zyvor.dev/guest-health-at": chrono::Utc::now().to_rfc3339(),
            }
        }
    });
    let _ = api
        .patch(vm_name, &PatchParams::default(), &Patch::Merge(&patch))
        .await;
}
