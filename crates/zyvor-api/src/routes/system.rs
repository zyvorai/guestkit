// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::State;
use axum::Json;
use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::error::ApiResult;
use crate::jobs::hydrate_job_record;
use crate::models::{ApiResponse, JobRecord};
use crate::routes::kubevirt::{cdi_dv_resource, list_dynamic_all, vm_resource};
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct SystemStatus {
    pub agent: String,
    pub cluster: String,
    pub storage: String,
    pub kubevirt: String,
    pub cdi: String,
    pub last_scan: Option<String>,
    pub disk_count: i64,
    pub cluster_vm_count: Option<i64>,
    pub worker: String,
    pub guest_agent_mtls: bool,
    pub packetwolf_correlation: bool,
    pub packetwolf_fleet: bool,
}

pub async fn get_system_status(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<SystemStatus>>> {
    let storage = state.config.storage_path.display().to_string();
    let cluster = if state.kube.is_some() {
        "ready"
    } else {
        "unknown"
    }
    .to_string();

    let kubevirt = probe_kubevirt(&state).await;
    let cdi = probe_cdi(&state).await;
    let last_scan = last_inspect_timestamp(&state).await;
    let disk_count = disk_count(&state).await.unwrap_or(0);
    let cluster_vm_count = cluster_vm_count(&state).await;
    let worker = worker_status(&state).await;

    Ok(Json(ApiResponse::ok(SystemStatus {
        agent: "online".into(),
        cluster,
        storage,
        kubevirt,
        cdi,
        last_scan,
        disk_count,
        cluster_vm_count,
        worker,
        guest_agent_mtls: state.config.agent_mtls_bind_addr.is_some(),
        packetwolf_correlation: state
            .config
            .packetwolf_correlation_url
            .as_ref()
            .is_some_and(|u| !u.trim().is_empty()),
        packetwolf_fleet: state
            .config
            .packetwolf_fleet_correlate_url
            .as_ref()
            .is_some_and(|u| !u.trim().is_empty())
            || state
                .config
                .packetwolf_correlation_url
                .as_ref()
                .is_some_and(|u| !u.trim().is_empty()),
    })))
}

async fn probe_kubevirt(state: &AppState) -> String {
    let Some(client) = state.kube.as_ref() else {
        return "unknown".into();
    };
    match list_dynamic_all(client, &vm_resource()).await {
        Ok(items) if !items.is_empty() => "healthy".into(),
        Ok(_) => "ready".into(),
        Err(_) => "degraded".into(),
    }
}

async fn probe_cdi(state: &AppState) -> String {
    let Some(client) = state.kube.as_ref() else {
        return "unknown".into();
    };
    match list_dynamic_all(client, &cdi_dv_resource()).await {
        Ok(_) => "ready".into(),
        Err(_) => "unknown".into(),
    }
}

async fn last_inspect_timestamp(state: &AppState) -> Option<String> {
    let rows = sqlx::query_as::<_, JobRecord>(
        "SELECT id, vm_id, operation, status, worker_id, submitted_at, completed_at
         FROM jobs
         WHERE operation IN ('guestkit.inspect', 'guestkit.doctor')
         ORDER BY COALESCE(completed_at, submitted_at) DESC
         LIMIT 20",
    )
    .fetch_all(&state.pool)
    .await
    .ok()?;

    let mut latest: Option<DateTime<Utc>> = None;
    for mut row in rows {
        hydrate_job_record(state, &mut row, true).await;
        if !matches!(row.status.as_str(), "completed" | "failed" | "running") {
            continue;
        }
        let ts = row.completed_at.unwrap_or(row.submitted_at);
        if latest.map(|l| ts > l).unwrap_or(true) {
            latest = Some(ts);
        }
    }

    latest.map(|ts| ts.to_rfc3339())
}

async fn disk_count(state: &AppState) -> Option<i64> {
    sqlx::query_scalar(
        "SELECT COUNT(*) FROM vm_images
         WHERE size_bytes > 0
         AND lower(name) NOT LIKE '%(cluster doctor)%'
         AND lower(name) NOT LIKE '%(cluster inspect)%'
         AND lower(name) NOT LIKE '%cluster-shadow%'",
    )
    .fetch_one(&state.pool)
    .await
    .ok()
}

async fn cluster_vm_count(state: &AppState) -> Option<i64> {
    let client = state.kube.as_ref()?;
    list_dynamic_all(client, &vm_resource())
        .await
        .ok()
        .map(|items| items.len() as i64)
}

async fn worker_status(state: &AppState) -> String {
    let mut redis = state.redis.clone();
    let pong: Result<String, _> = redis::cmd("PING")
        .query_async(&mut redis)
        .await;
    if pong.is_ok() {
        "online".into()
    } else {
        "unknown".into()
    }
}
