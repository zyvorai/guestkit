// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Export cluster VM root disk into Zyvor ingest storage.

use axum::extract::{Path, Query, State};
use axum::Json;
use kube::Client;
use serde::Deserialize;
use serde::Serialize;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::kubevirt_boot_inspect::{resolve_disk_path, root_pvc_from_vm};
use crate::kubevirt_lifecycle;
use crate::models::ApiResponse;
use crate::routes::kubevirt::{fetch_vm, fetch_vmi, json_str};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ExportDiskQuery {
    #[serde(default)]
    pub force_stop: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ExportDiskResult {
    pub vm_id: Uuid,
    pub name: String,
    pub format: String,
    pub size_bytes: i64,
    pub source: String,
    pub requires_vm_stopped: bool,
    pub cluster_vm: String,
    pub root_pvc: Option<String>,
}

pub async fn export_vm_disk(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<ExportDiskQuery>,
) -> ApiResult<Json<ApiResponse<ExportDiskResult>>> {
    let client = kube_client(&state)?;
    let vm = fetch_vm(&client, &namespace, &name)
        .await
        .ok_or_else(|| ApiError::not_found(format!("VM {namespace}/{name} not found")))?;
    let vmi = fetch_vmi(&client, &namespace, &name).await;
    let running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");

    if running {
        if query.force_stop == Some(true) {
            kubevirt_lifecycle::stop_vm_for_export(&client, &namespace, &name).await?;
        } else {
            return Err(ApiError::conflict(
                "VM is running — stop it first or pass ?force_stop=true",
            ));
        }
    }

    let root_pvc = root_pvc_from_vm(&vm).ok_or_else(|| {
        ApiError::bad_request("No root PVC found in VM spec — cannot export disk")
    })?;

    let disk_path = resolve_disk_path(&client, &namespace, &root_pvc)
        .await
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Could not resolve disk path for PVC {root_pvc} — stop VM and check KUBEVIRT_DISK_ROOT"
            ))
        })?;

    let format = disk_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("qcow2")
        .to_lowercase();
    let id = Uuid::new_v4();
    let object_key = format!("{id}.{format}");
    let dest = state.config.storage_path.join(&object_key);

    tokio::fs::create_dir_all(&state.config.storage_path)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    tokio::fs::copy(&disk_path, &dest)
        .await
        .map_err(|e| ApiError::internal(format!("copy disk: {e}")))?;

    let size_bytes = tokio::fs::metadata(&dest)
        .await
        .map(|m| m.len() as i64)
        .unwrap_or(0);

    let import_name = format!("{namespace}-{name}.{format}");

    sqlx::query(
        r#"INSERT INTO vm_images (id, tenant, name, object_key, format, size_bytes, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(id)
    .bind("default")
    .bind(&import_name)
    .bind(&object_key)
    .bind(&format)
    .bind(size_bytes)
    .bind("imported")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(ApiResponse::ok(ExportDiskResult {
        vm_id: id,
        name: import_name,
        format,
        size_bytes,
        source: "cluster-pvc".into(),
        requires_vm_stopped: running,
        cluster_vm: format!("{namespace}/{name}"),
        root_pvc: Some(root_pvc),
    })))
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("Disk export requires in-cluster Kubernetes access"))
}
