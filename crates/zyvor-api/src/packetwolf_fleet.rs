// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Periodic fleet-wide PacketWolf correlation across all guest-agent reports.

use kube::api::{Api, PatchParams};
use kube::Client;
use redis::AsyncCommands;
use serde_json::{json, Value};
use tracing::{info, warn};

use crate::config::Config;
use crate::packetwolf_correlate::CorrelationMember;
use crate::routes::guest_agent::fetch_vm_guest_report;
use crate::routes::kubevirt::{list_dynamic_all, vm_resource};

const FLEET_SNAPSHOT_KEY: &str = "guest-agent:fleet-correlation";
const FLEET_ANNOTATION: &str = "zeus.zyvor.dev/packetwolf-fleet-correlation";

pub fn spawn_fleet_worker(
    config: Config,
    kube: Option<Client>,
    redis: redis::aio::ConnectionManager,
) {
    let url = fleet_url(&config);
    if url.is_none() && kube.is_none() {
        return;
    }
    let interval = config.packetwolf_fleet_interval_secs.max(60);
    tokio::spawn(async move {
        let mut redis = redis;
        loop {
            if let Err(e) =
                run_fleet_correlation(&config, kube.as_ref(), &mut redis, url.as_deref()).await
            {
                warn!("PacketWolf fleet correlation failed: {e}");
            }
            tokio::time::sleep(std::time::Duration::from_secs(interval)).await;
        }
    });
}

pub fn fleet_url_public(config: &Config) -> Option<String> {
    fleet_url(config)
}

fn fleet_url(config: &Config) -> Option<String> {
    if let Some(url) = config
        .packetwolf_fleet_correlate_url
        .as_ref()
        .filter(|u| !u.trim().is_empty())
    {
        return Some(url.clone());
    }
    config
        .packetwolf_correlation_url
        .as_ref()
        .filter(|u| !u.trim().is_empty())
        .map(|base| format!("{}/fleet", base.trim_end_matches('/')))
}

pub async fn run_fleet_correlation(
    _config: &Config,
    kube: Option<&Client>,
    redis: &mut redis::aio::ConnectionManager,
    url: Option<&str>,
) -> Result<(), String> {
    let members = collect_fleet_members(kube, redis).await?;
    if members.is_empty() {
        return Ok(());
    }

    let snapshot_id = format!("fleet-{}", chrono::Utc::now().format("%Y%m%dT%H%M%SZ"));
    let body = json!({
        "source": "zyvor-guest-agent",
        "kind": "fleet",
        "snapshot_id": snapshot_id,
        "member_count": members.len(),
        "observed_at": chrono::Utc::now().to_rfc3339(),
        "members": members,
    });

    if let Some(url) = url {
        let client = reqwest::Client::builder()
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .map_err(|e| e.to_string())?;
        client
            .post(url)
            .json(&body)
            .send()
            .await
            .map_err(|e| e.to_string())?;
        info!(
            "PacketWolf fleet correlation posted {} members to {}",
            members.len(),
            url
        );
    }

    redis
        .set_ex::<_, _, ()>(
            FLEET_SNAPSHOT_KEY,
            serde_json::to_string(&body).unwrap_or_default(),
            86400,
        )
        .await
        .map_err(|e| e.to_string())?;

    if let Some(client) = kube {
        patch_fleet_annotations(client, &snapshot_id, members.len()).await;
    }

    Ok(())
}

pub async fn last_fleet_snapshot(
    redis: &mut redis::aio::ConnectionManager,
) -> Option<Value> {
    let raw: Option<String> = redis.get(FLEET_SNAPSHOT_KEY).await.ok().flatten();
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

async fn collect_fleet_members(
    kube: Option<&Client>,
    redis: &mut redis::aio::ConnectionManager,
) -> Result<Vec<CorrelationMember>, String> {
    let client = kube.ok_or_else(|| "kubernetes client unavailable".to_string())?;
    let vms = list_dynamic_all(client, &vm_resource())
        .await
        .map_err(|e| e.message.clone())?;
    let mut members = Vec::new();
    for vm in vms {
        let ns = vm
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        let name = vm
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if name.is_empty() {
            continue;
        }
        let report = fetch_vm_guest_report(redis, ns, name).await;
        if let Some(report) = report {
            let guest_health = report
                .get("guest_health")
                .cloned()
                .unwrap_or(json!({}));
            let metrics = report.get("metrics").cloned().unwrap_or(json!({}));
            let recent_events = report.get("recent_events").cloned().unwrap_or(json!([]));
            let agent_version = guest_health
                .get("agent_version")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            members.push(crate::packetwolf_correlate::build_member(
                ns,
                name,
                &guest_health,
                &metrics,
                &recent_events,
                agent_version,
            ));
        }
    }
    Ok(members)
}

async fn patch_fleet_annotations(client: &Client, snapshot_id: &str, count: usize) {
    let vms = match list_dynamic_all(client, &vm_resource()).await {
        Ok(v) => v,
        Err(e) => {
            warn!("fleet VM list for annotations failed: {}", e.message);
            return;
        }
    };
    let ar = vm_resource();
    let at = chrono::Utc::now().to_rfc3339();
    for vm in vms {
        let name = vm
            .pointer("/metadata/name")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let ns = vm
            .pointer("/metadata/namespace")
            .and_then(|v| v.as_str())
            .unwrap_or("default");
        if name.is_empty() {
            continue;
        }
        let patch = json!({
            "metadata": {
                "annotations": {
                    FLEET_ANNOTATION: snapshot_id,
                    "zeus.zyvor.dev/packetwolf-fleet-count": count.to_string(),
                    "zeus.zyvor.dev/packetwolf-fleet-at": at,
                }
            }
        });
        let namespaced: Api<kube::api::DynamicObject> =
            Api::namespaced_with(client.clone(), ns, &ar);
        if let Err(e) = namespaced
            .patch(name, &PatchParams::default(), &kube::api::Patch::Merge(&patch))
            .await
        {
            warn!("fleet annotation patch {ns}/{name}: {e}");
        }
    }
}
