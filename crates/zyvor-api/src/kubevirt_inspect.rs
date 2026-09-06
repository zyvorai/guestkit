// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline GuestKit jobs against KubeVirt VM root disks (stopped VMs).

use axum::extract::{Path, Query, State};
use axum::Json;
use guestkit::export::{generate_kubevirt_manifests, manifests_to_yaml, DiskMetadata};
use kube::Client;
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::jobs::submit_disk_path_job;
use crate::kubevirt_boot_inspect::{resolve_disk_path, root_pvc_from_vm};
use crate::models::{ApiResponse, JobEnqueueResponse, ProvisionResponse};
use crate::routes::kubevirt::{fetch_vm, fetch_vmi, json_str};
use crate::state::AppState;

#[derive(Debug, Deserialize, Default)]
pub struct ClusterDoctorQuery {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default)]
    pub explain: bool,
}

fn default_target() -> String {
    "kubevirt".into()
}

pub struct ResolvedDisk {
    pub shadow_id: Uuid,
    pub disk_path: std::path::PathBuf,
    pub format: String,
    pub label: String,
}

/// Public disk resolution for guest control offline repair routes.
pub async fn resolve_stopped_vm_disk_public(
    state: &AppState,
    client: &Client,
    namespace: &str,
    name: &str,
    shadow_prefix: &str,
) -> ApiResult<ResolvedDisk> {
    resolve_stopped_vm_disk(state, client, namespace, name, shadow_prefix).await
}

async fn resolve_stopped_vm_disk(
    state: &AppState,
    client: &Client,
    namespace: &str,
    name: &str,
    shadow_prefix: &str,
) -> ApiResult<ResolvedDisk> {
    let vm = fetch_vm(client, namespace, name)
        .await
        .ok_or_else(|| ApiError::not_found(format!("VM {namespace}/{name} not found")))?;
    let vmi = fetch_vmi(client, namespace, name).await;
    let running = vmi
        .as_ref()
        .and_then(|v| json_str(v, &["status", "phase"]))
        .as_deref()
        == Some("Running");
    if running {
        return Err(ApiError::conflict(
            "VM is running — stop it for GuestKit offline disk analysis, or use live guest agent",
        ));
    }

    let root_pvc = root_pvc_from_vm(&vm).ok_or_else(|| {
        ApiError::bad_request("No root PVC found in VM spec — cannot access disk")
    })?;

    let disk_path = resolve_disk_path(client, namespace, &root_pvc)
        .await
        .ok_or_else(|| {
            ApiError::bad_request(format!(
                "Could not resolve disk path for PVC {root_pvc} — check KUBEVIRT_DISK_ROOT"
            ))
        })?;

    let format = match disk_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase()
        .as_str()
    {
        "qcow2" => "qcow2",
        "raw" | "img" => "raw",
        other if !other.is_empty() => other,
        _ if disk_path.file_name().and_then(|n| n.to_str()) == Some("disk.img") => "raw",
        _ => "qcow2",
    }
    .to_string();

    let shadow_id = Uuid::new_v4();
    let label = format!("{namespace}/{name}");
    sqlx::query(
        r#"INSERT INTO vm_images (id, tenant, name, object_key, format, size_bytes, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(shadow_id)
    .bind("cluster")
    .bind(format!("{label} ({shadow_prefix})"))
    .bind(format!("cluster-shadow/{shadow_id}"))
    .bind(&format)
    .bind(0_i64)
    .bind("cluster-ref")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(ResolvedDisk {
        shadow_id,
        disk_path,
        format,
        label,
    })
}

pub async fn post_inspect_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let client = kube_client(&state)?;
    let disk = resolve_stopped_vm_disk(&state, &client, &namespace, &name, "cluster inspect").await?;
    let resp = submit_disk_path_job(
        &state,
        disk.shadow_id,
        &disk.disk_path,
        &disk.format,
        "guestkit.inspect",
        "guestkit.inspect.v1",
        json!({
            "options": {
                "include_packages": true,
                "include_services": true,
                "include_security": true,
                "include_network": true
            },
            "cluster_vm": disk.label,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn post_doctor_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<ClusterDoctorQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let client = kube_client(&state)?;
    let disk = resolve_stopped_vm_disk(&state, &client, &namespace, &name, "cluster doctor").await?;
    let resp = submit_disk_path_job(
        &state,
        disk.shadow_id,
        &disk.disk_path,
        &disk.format,
        "guestkit.doctor",
        "guestkit.doctor.v1",
        json!({
            "target": query.target,
            "explain": query.explain,
            "cluster_vm": disk.label,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn post_migration_plan_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<ClusterDoctorQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let client = kube_client(&state)?;
    let disk =
        resolve_stopped_vm_disk(&state, &client, &namespace, &name, "cluster migrate").await?;
    let resp = submit_disk_path_job(
        &state,
        disk.shadow_id,
        &disk.disk_path,
        &disk.format,
        "guestkit.migrate-plan",
        "guestkit.migrate-plan.v1",
        json!({
            "target": query.target,
            "explain": query.explain,
            "export_fix_plan": true,
            "cluster_vm": disk.label,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn post_repair_plan_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let client = kube_client(&state)?;
    let disk =
        resolve_stopped_vm_disk(&state, &client, &namespace, &name, "cluster repair").await?;
    let resp = submit_disk_path_job(
        &state,
        disk.shadow_id,
        &disk.disk_path,
        &disk.format,
        "guestkit.repair",
        "guestkit.repair.v1",
        json!({
            "fix": "boot",
            "dry_run": true,
            "cluster_vm": disk.label,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

#[derive(Debug, Deserialize, Default)]
pub struct ClusterProvisionQuery {
    #[serde(default)]
    pub apply: bool,
}

pub async fn post_provision_vm(
    State(state): State<AppState>,
    Path((namespace, name)): Path<(String, String)>,
    Query(query): Query<ClusterProvisionQuery>,
) -> ApiResult<Json<ApiResponse<ProvisionResponse>>> {
    let client = kube_client(&state)?;
    let disk =
        resolve_stopped_vm_disk(&state, &client, &namespace, &name, "cluster provision").await?;

    let plan_result = guestkit::assurance::run_migrate_plan(
        &disk.disk_path,
        "kubevirt",
        &guestkit::assurance::MigratePlanOptions {
            explain: false,
            verbose: false,
            export_fix_plan: true,
            inject_agent: false,
        },
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut disk_meta = DiskMetadata::from_image_path(
        &disk.disk_path,
        &namespace,
        &state.config.storage_class,
    );
    disk_meta.import_url = Some(format!(
        "{}/cluster-shadow/{}",
        state.config.storage_public_url.trim_end_matches('/'),
        disk.shadow_id
    ));

    let manifests = generate_kubevirt_manifests(&plan_result, &disk_meta)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let yaml = manifests_to_yaml(&manifests).map_err(|e| ApiError::internal(e.to_string()))?;

    let mut applied = false;
    let mut resources = None;
    let mut apply_errors = None;
    if query.apply {
        let client = state.kube.clone().ok_or_else(|| {
            ApiError::bad_request("provision apply=true requires in-cluster Kubernetes access")
        })?;
        let result = crate::kubevirt_apply::apply_kubevirt_manifests(&client, &yaml).await?;
        applied = result.applied;
        resources = Some(result.resources);
        if !result.errors.is_empty() {
            apply_errors = Some(result.errors);
        }
    }

    Ok(Json(ApiResponse::ok(ProvisionResponse {
        vm_id: disk.shadow_id,
        yaml,
        applied,
        resources,
        apply_errors,
    })))
}

fn kube_client(state: &AppState) -> ApiResult<Client> {
    state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("Cluster disk jobs require in-cluster Kubernetes access"))
}
