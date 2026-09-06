// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::{Multipart, Path, Query, State};
use axum::Json;
use guestkit::export::{generate_kubevirt_manifests, manifests_to_yaml, DiskMetadata};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;
use crate::error::{ApiError, ApiResult};
use crate::jobs::{hydrate_job_record, submit_disk_path_job};
use crate::models::{
    ApiResponse, JobEnqueueResponse, JobRecord, ProvisionResponse, VmImage, VmImportResponse,
};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct DoctorQuery {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default)]
    pub explain: bool,
}

fn default_target() -> String {
    "kubevirt".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ProvisionQuery {
    #[serde(default)]
    pub apply: bool,
}

#[derive(Debug, Deserialize, Default)]
pub struct ListVmsQuery {
    #[serde(default)]
    pub include_shadow: bool,
}

pub async fn import_vm(
    State(state): State<AppState>,
    mut multipart: Multipart,
) -> ApiResult<Json<ApiResponse<VmImportResponse>>> {
    use tokio::io::AsyncWriteExt;

    let mut filename = String::from("uploaded-image");
    let mut size_bytes: u64 = 0;
    let mut wrote_file = false;
    let id = Uuid::new_v4();
    let mut disk_path = state.config.storage_path.join("pending");

    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::bad_request(e.to_string()))?
    {
        if field.name() == Some("file") {
            if let Some(name) = field.file_name() {
                filename = name.to_string();
            }
            let format = PathBuf::from(&filename)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("qcow2")
                .to_lowercase();
            let object_key = format!("{id}.{format}");
            disk_path = state.config.storage_path.join(&object_key);
            let mut out = tokio::fs::File::create(&disk_path)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;

            while let Some(chunk) = field
                .chunk()
                .await
                .map_err(|e| ApiError::bad_request(e.to_string()))?
            {
                out.write_all(&chunk)
                    .await
                    .map_err(|e| ApiError::internal(e.to_string()))?;
                size_bytes += chunk.len() as u64;
            }
            out.flush()
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
            wrote_file = true;
        }
    }

    if !wrote_file || size_bytes == 0 {
        let _ = tokio::fs::remove_file(&disk_path).await;
        return Err(ApiError::bad_request("file field is required"));
    }

    let format = PathBuf::from(&filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("qcow2")
        .to_lowercase();
    let object_key = format!("{id}.{format}");

    sqlx::query(
        r#"INSERT INTO vm_images (id, tenant, name, object_key, format, size_bytes, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(id)
    .bind("default")
    .bind(&filename)
    .bind(&object_key)
    .bind(&format)
    .bind(size_bytes as i64)
    .bind("imported")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(ApiResponse::ok(VmImportResponse {
        id,
        name: filename,
        format,
        size_bytes: size_bytes as i64,
        path: disk_path.display().to_string(),
    })))
}

pub async fn list_vms(
    State(state): State<AppState>,
    Query(query): Query<ListVmsQuery>,
) -> ApiResult<Json<ApiResponse<Vec<VmImage>>>> {
    let rows = if query.include_shadow {
        sqlx::query_as::<_, VmImage>(
            "SELECT id, tenant, name, object_key, format, size_bytes, checksum, status, created_at FROM vm_images ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await
    } else {
        sqlx::query_as::<_, VmImage>(
            "SELECT id, tenant, name, object_key, format, size_bytes, checksum, status, created_at
             FROM vm_images
             WHERE size_bytes > 0
             AND lower(name) NOT LIKE '%(cluster doctor)%'
             AND lower(name) NOT LIKE '%(cluster inspect)%'
             AND lower(name) NOT LIKE '%cluster-shadow%'
             ORDER BY created_at DESC",
        )
        .fetch_all(&state.pool)
        .await
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(rows)))
}

pub(crate) async fn load_vm(state: &AppState, id: Uuid) -> ApiResult<VmImage> {
    sqlx::query_as::<_, VmImage>(
        "SELECT id, tenant, name, object_key, format, size_bytes, checksum, status, created_at FROM vm_images WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found(format!("VM {id} not found")))
}

async fn submit_vm_job(
    state: &AppState,
    vm: &VmImage,
    operation: &str,
    payload_type: &str,
    data: serde_json::Value,
) -> ApiResult<JobEnqueueResponse> {
    let image_path = state.config.storage_path.join(&vm.object_key);
    submit_disk_path_job(
        state,
        vm.id,
        &image_path,
        &vm.format,
        operation,
        payload_type,
        data,
    )
    .await
}

pub async fn inspect_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.inspect",
        "guestkit.inspect.v1",
        serde_json::json!({
            "options": {
                "include_packages": true,
                "include_services": true,
                "include_security": true
            }
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn doctor_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.doctor",
        "guestkit.doctor.v1",
        serde_json::json!({
            "target": query.target,
            "explain": query.explain,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn migration_plan_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.migrate-plan",
        "guestkit.migrate-plan.v1",
        serde_json::json!({
            "target": query.target,
            "explain": query.explain,
            "export_fix_plan": true,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn passport_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<DoctorQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.passport",
        "guestkit.passport.v1",
        serde_json::json!({
            "target": query.target,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn repair_plan_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<RepairQuery>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.repair",
        "guestkit.repair.v1",
        serde_json::json!({
            "fix": query.fix,
            "dry_run": query.dry_run,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

#[derive(Debug, Deserialize)]
pub struct RepairQuery {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_fix_boot")]
    pub fix: String,
}

fn default_true() -> bool {
    true
}

fn default_fix_boot() -> String {
    "boot".to_string()
}

#[derive(Debug, Deserialize)]
pub struct ProfileRequest {
    #[serde(default = "default_profiles")]
    pub profiles: Vec<String>,
}

fn default_profiles() -> Vec<String> {
    vec![
        "security".into(),
        "compliance".into(),
        "hardening".into(),
        "migration".into(),
    ]
}

pub async fn profile_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    body: Option<Json<ProfileRequest>>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let profiles = body
        .map(|Json(b)| b.profiles)
        .unwrap_or_else(default_profiles);
    let profiles = if profiles.is_empty() {
        default_profiles()
    } else {
        profiles
    };
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.profile",
        "guestkit.profile.v1",
        serde_json::json!({
            "profiles": profiles,
            "options": {
                "include_remediation": true
            }
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

#[derive(Debug, Deserialize)]
pub struct ExploreRequest {
    #[serde(default = "default_explore_action")]
    pub action: String,
    #[serde(default = "default_explore_path")]
    pub path: String,
}

fn default_explore_action() -> String {
    "ls".into()
}

fn default_explore_path() -> String {
    "/".into()
}

pub async fn explore_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ExploreRequest>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let action = body.action.to_lowercase();
    if !matches!(action.as_str(), "ls" | "stat" | "cat") {
        return Err(ApiError::bad_request(
            "action must be one of: ls, stat, cat",
        ));
    }
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.explore",
        "guestkit.explore.v1",
        serde_json::json!({
            "action": action,
            "path": body.path,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

pub async fn provision_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Query(query): Query<ProvisionQuery>,
) -> ApiResult<Json<ApiResponse<ProvisionResponse>>> {
    let vm = load_vm(&state, id).await?;
    let image_path = state.config.storage_path.join(&vm.object_key);

    let plan_result = guestkit::assurance::run_migrate_plan(
        &image_path,
        "kubevirt",
        &guestkit::assurance::MigratePlanOptions {
            explain: false,
            verbose: false,
            export_fix_plan: true,
            inject_agent: false,
        },
    )
    // `{e:#}` (not `e.to_string()`/`{e}`): anyhow's plain Display only
    // shows the outermost .context() layer. collect_assurance_data wraps
    // every mount_all_ro failure in "No operating system found in disk
    // image" regardless of the actual cause (file not found, guestfs
    // launch failure, device busy, ...) — the alternate-Display chain is
    // the only way this handler's error response reflects what actually
    // went wrong instead of that one fixed, possibly-misleading string.
    .map_err(|e| ApiError::internal(format!("{e:#}")))?;

    let mut disk = DiskMetadata::from_image_path(
        &image_path,
        &state.config.default_namespace,
        &state.config.storage_class,
    );
    disk.import_url = Some(format!(
        "{}/{}",
        state.config.storage_public_url.trim_end_matches('/'),
        vm.object_key
    ));

    let manifests = generate_kubevirt_manifests(&plan_result, &disk)
        .map_err(|e| ApiError::internal(format!("{e:#}")))?;
    let yaml = manifests_to_yaml(&manifests).map_err(|e| ApiError::internal(format!("{e:#}")))?;

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
        vm_id: id,
        yaml,
        applied,
        resources,
        apply_errors,
    })))
}

#[derive(Debug, Deserialize)]
pub struct ConvertRequest {
    #[serde(default = "default_qcow2")]
    pub target_format: String,
    #[serde(default)]
    pub compression: bool,
}

fn default_qcow2() -> String {
    "qcow2".into()
}

pub async fn convert_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ConvertRequest>,
) -> ApiResult<Json<ApiResponse<JobEnqueueResponse>>> {
    let vm = load_vm(&state, id).await?;
    let resp = submit_vm_job(
        &state,
        &vm,
        "guestkit.convert",
        "guestkit.convert.v1",
        serde_json::json!({
            "target_format": body.target_format,
            "compression": body.compression,
        }),
    )
    .await?;
    Ok(Json(ApiResponse::ok(resp)))
}

#[derive(Debug, Deserialize)]
pub struct ImportUrlRequest {
    pub url: String,
}

fn import_filename_from_url(url: &str) -> &str {
    let path = url
        .strip_prefix("s3://")
        .or_else(|| url.strip_prefix("gs://"))
        .unwrap_or(url);
    path.rsplit('/').next().filter(|s| !s.is_empty()).unwrap_or("imported-image")
}

fn is_cloud_disk_url(url: &str) -> bool {
    url.starts_with("s3://")
        || url.starts_with("gs://")
        || (url.starts_with("https://") && url.contains(".blob.core.windows.net"))
}

async fn materialize_cloud_disk(url: &str, dest: &std::path::Path) -> ApiResult<i64> {
    let url = url.to_string();
    let dest = dest.to_path_buf();
    let size = tokio::task::spawn_blocking(move || -> anyhow::Result<u64> {
        let local = guestkit::storage::resolve_to_local_path(&url)?;
        let bytes = std::fs::read(&local)?;
        std::fs::write(&dest, &bytes)?;
        Ok(bytes.len() as u64)
    })
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .map_err(|e| ApiError::bad_request(format!("cloud import failed: {e:#}")))?;
    Ok(size as i64)
}

pub async fn import_from_url(
    State(state): State<AppState>,
    Json(body): Json<ImportUrlRequest>,
) -> ApiResult<Json<ApiResponse<VmImportResponse>>> {
    let url = body.url.trim();
    if url.is_empty() {
        return Err(ApiError::bad_request("url is required"));
    }
    let filename = import_filename_from_url(url);
    let id = Uuid::new_v4();
    let format = PathBuf::from(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("qcow2")
        .to_lowercase();
    let object_key = format!("{id}.{format}");
    let disk_path = state.config.storage_path.join(&object_key);

    let size_bytes = if is_cloud_disk_url(url) {
        materialize_cloud_disk(url, &disk_path).await?
    } else if url.starts_with("http://") || url.starts_with("https://") {
        let client = reqwest::Client::new();
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| ApiError::bad_request(format!("download failed: {e}")))?;
        if !resp.status().is_success() {
            return Err(ApiError::bad_request(format!("download HTTP {}", resp.status())));
        }
        let bytes = resp
            .bytes()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        tokio::fs::write(&disk_path, &bytes)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        bytes.len() as i64
    } else {
        return Err(ApiError::bad_request(
            "url must be http(s), s3://, gs://, or Azure blob https URL",
        ));
    };

    register_imported_vm(&state, id, filename, &object_key, &format, size_bytes).await?;
    Ok(Json(ApiResponse::ok(VmImportResponse {
        id,
        name: filename.to_string(),
        format,
        size_bytes,
        path: disk_path.display().to_string(),
    })))
}

#[derive(Debug, Deserialize)]
pub struct ImportS3Request {
    pub bucket: String,
    pub key: String,
    pub endpoint: Option<String>,
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
}

pub async fn import_from_s3(
    State(state): State<AppState>,
    Json(body): Json<ImportS3Request>,
) -> ApiResult<Json<ApiResponse<VmImportResponse>>> {
    if body.bucket.trim().is_empty() || body.key.trim().is_empty() {
        return Err(ApiError::bad_request("bucket and key are required"));
    }
    let endpoint = body
        .endpoint
        .as_deref()
        .unwrap_or("https://s3.amazonaws.com")
        .trim_end_matches('/');
    let url = format!("{endpoint}/{}/{}", body.bucket.trim(), body.key.trim());
    let filename = body.key.rsplit('/').next().unwrap_or("s3-import");
    let id = Uuid::new_v4();
    let format = PathBuf::from(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("qcow2")
        .to_lowercase();
    let object_key = format!("{id}.{format}");
    let disk_path = state.config.storage_path.join(&object_key);

    let mut req = reqwest::Client::new().get(&url);
    if let (Some(ak), Some(sk)) = (&body.access_key, &body.secret_key) {
        req = req.basic_auth(ak, Some(sk));
    }
    let resp = req
        .send()
        .await
        .map_err(|e| ApiError::bad_request(format!("s3 download failed: {e}")))?;
    if !resp.status().is_success() {
        return Err(ApiError::bad_request(format!("s3 HTTP {}", resp.status())));
    }
    let bytes = resp
        .bytes()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    tokio::fs::write(&disk_path, &bytes)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let size_bytes = bytes.len() as i64;

    register_imported_vm(&state, id, filename, &object_key, &format, size_bytes).await?;
    Ok(Json(ApiResponse::ok(VmImportResponse {
        id,
        name: filename.to_string(),
        format,
        size_bytes,
        path: disk_path.display().to_string(),
    })))
}

async fn register_imported_vm(
    state: &AppState,
    id: Uuid,
    filename: &str,
    object_key: &str,
    format: &str,
    size_bytes: i64,
) -> ApiResult<()> {
    sqlx::query(
        r#"INSERT INTO vm_images (id, tenant, name, object_key, format, size_bytes, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(id)
    .bind("default")
    .bind(filename)
    .bind(object_key)
    .bind(format)
    .bind(size_bytes)
    .bind("imported")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CompareQuery {
    pub before: Uuid,
    pub after: Uuid,
}

#[derive(Debug, Serialize)]
pub struct CompareResult {
    pub before: Uuid,
    pub after: Uuid,
    pub diff: serde_json::Value,
}

pub async fn compare_vms(
    State(state): State<AppState>,
    Query(query): Query<CompareQuery>,
) -> ApiResult<Json<ApiResponse<CompareResult>>> {
    let before_vm = load_vm(&state, query.before).await?;
    let after_vm = load_vm(&state, query.after).await?;
    let before_path = state.config.storage_path.join(&before_vm.object_key);
    let after_path = state.config.storage_path.join(&after_vm.object_key);

    let before_data = guestkit::assurance::collect_assurance_data(
        &before_path,
        guestkit::assurance::boot_target_from_str("kubevirt"),
        false,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;
    let after_data = guestkit::assurance::collect_assurance_data(
        &after_path,
        guestkit::assurance::boot_target_from_str("kubevirt"),
        false,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let before_doctor = guestkit::assurance::run_doctor(&before_path, "kubevirt", false, false)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let after_doctor = guestkit::assurance::run_doctor(&after_path, "kubevirt", false, false)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let diff = serde_json::json!({
        "boot_score_delta": after_doctor.bootability.score - before_doctor.bootability.score,
        "before_boot_score": before_doctor.bootability.score,
        "after_boot_score": after_doctor.bootability.score,
        "before_os": before_data.0.os,
        "after_os": after_data.0.os,
        "before_blockers": before_doctor.bootability.blockers.len(),
        "after_blockers": after_doctor.bootability.blockers.len(),
        "new_warnings": after_doctor.bootability.warnings.len().saturating_sub(before_doctor.bootability.warnings.len()),
    });

    Ok(Json(ApiResponse::ok(CompareResult {
        before: query.before,
        after: query.after,
        diff,
    })))
}

pub async fn readiness_report(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<axum::response::Response> {
    use axum::body::Body;
    use axum::http::{header, StatusCode};
    use axum::response::IntoResponse;

    let vm = load_vm(&state, id).await?;
    let image_path = state.config.storage_path.join(&vm.object_key);
    let doctor = guestkit::assurance::run_doctor(&image_path, "kubevirt", true, false)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let evidence = guestkit::assurance::collect_assurance_data(
        &image_path,
        guestkit::assurance::boot_target_from_str("kubevirt"),
        false,
    )
    .map_err(|e| ApiError::internal(e.to_string()))?;

    let title = format!("GuestKit Readiness Report — {}", vm.name);
    let body = format!(
        "GuestKit Readiness Report\nVM: {}\nFormat: {}\nSize: {} bytes\nBoot score: {}\nBlockers: {}\nWarnings: {}\nOS: {:?}\nGenerated: {}\n",
        vm.name,
        vm.format,
        vm.size_bytes,
        doctor.bootability.score,
        doctor.bootability.blockers.len(),
        doctor.bootability.warnings.len(),
        evidence.0.os,
        chrono::Utc::now().format("%Y-%m-%d %H:%M UTC"),
    );
    let pdf = minimal_pdf(&title, &body);

    let filename = format!("attachment; filename=\"{}-readiness.pdf\"", vm.name.replace('/', "_"));
    Ok((
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "application/pdf"),
            (header::CONTENT_DISPOSITION, filename.as_str()),
        ],
        Body::from(pdf),
    )
        .into_response())
}

fn minimal_pdf(title: &str, body: &str) -> Vec<u8> {
    let escaped_title = escape_pdf_text(title);
    let escaped_body = escape_pdf_text(body);
    let content = format!(
        "BT /F1 14 Tf 50 750 Td ({escaped_title}) Tj ET\nBT /F1 10 Tf 50 720 Td ({escaped_body}) Tj ET\n"
    );
    let content_len = content.len();
    format!(
        "%PDF-1.4\n\
1 0 obj<< /Type /Catalog /Pages 2 0 R >>endobj\n\
2 0 obj<< /Type /Pages /Kids [3 0 R] /Count 1 >>endobj\n\
3 0 obj<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R /Resources<< /Font<< /F1 5 0 R >> >> >>endobj\n\
4 0 obj<< /Length {content_len} >>stream\n{content}endstream endobj\n\
5 0 obj<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>endobj\n\
xref\n0 6\n0000000000 65535 f \n0000000009 00000 n \n0000000058 00000 n \n0000000115 00000 n \n0000000266 00000 n \n0000000360 00000 n \n\
trailer<< /Size 6 /Root 1 0 R >>\nstartxref\n440\n%%EOF"
    )
    .into_bytes()
}

fn escape_pdf_text(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('(', "\\(")
        .replace(')', "\\)")
        .replace('\n', "\\n")
        .chars()
        .take(800)
        .collect()
}

pub async fn list_vm_jobs(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<Vec<JobRecord>>>> {
    let _ = load_vm(&state, id).await?;
    let mut rows = sqlx::query_as::<_, JobRecord>(
        "SELECT id, vm_id, operation, status, worker_id, submitted_at, completed_at
         FROM jobs WHERE vm_id = $1 ORDER BY submitted_at DESC LIMIT 30",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    for row in rows.iter_mut() {
        hydrate_job_record(&state, row, true).await;
    }
    Ok(Json(ApiResponse::ok(rows)))
}

pub async fn cleanup_shadow_vms(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    let deleted: Vec<(Uuid,)> = sqlx::query_as(
        "DELETE FROM vm_images
         WHERE size_bytes = 0
         OR lower(name) LIKE '%(cluster doctor)%'
         OR lower(name) LIKE '%(cluster inspect)%'
         OR lower(name) LIKE '%cluster-shadow%'
         RETURNING id",
    )
    .fetch_all(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(serde_json::json!({
        "deleted": deleted.len(),
        "ids": deleted.iter().map(|(id,)| id).collect::<Vec<_>>(),
    }))))
}

pub async fn delete_vm(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    let vm = load_vm(&state, id).await?;
    let disk_path = state.config.storage_path.join(&vm.object_key);
    let _ = tokio::fs::remove_file(&disk_path).await;
    // Remove dependent rows first — jobs.vm_id and job_results.job_id are
    // FKs without ON DELETE CASCADE, so a bare DELETE on vm_images fails for
    // any disk that has ever been analyzed. Tear down the chain in one tx.
    let mut tx = state
        .pool
        .begin()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query("DELETE FROM job_results WHERE job_id IN (SELECT id FROM jobs WHERE vm_id = $1)")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query("DELETE FROM jobs WHERE vm_id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    sqlx::query("DELETE FROM vm_images WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    tx.commit()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(serde_json::json!({ "deleted": id }))))
}

#[derive(Debug, Deserialize)]
pub struct ImportNfsRequest {
    pub path: String,
    #[allow(dead_code)]
    pub host: Option<String>,
}

pub async fn import_from_nfs(
    State(state): State<AppState>,
    Json(body): Json<ImportNfsRequest>,
) -> ApiResult<Json<ApiResponse<VmImportResponse>>> {
    let path = body.path.trim();
    if path.is_empty() {
        return Err(ApiError::bad_request("path is required"));
    }
    let abs = if let Some(host) = body.host.as_deref().map(str::trim).filter(|h| !h.is_empty()) {
        if path.starts_with('/') {
            state.config.nfs_mount_root.join(host).join(path.trim_start_matches('/'))
        } else {
            state.config.nfs_mount_root.join(host).join(path)
        }
    } else {
        std::path::PathBuf::from(path)
    };
    let meta = tokio::fs::metadata(&abs)
        .await
        .map_err(|e| ApiError::bad_request(format!("NFS path not readable: {e}")))?;
    if !meta.is_file() {
        return Err(ApiError::bad_request("path must be a file on the server"));
    }
    let filename = abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("nfs-import");
    let id = Uuid::new_v4();
    let format = PathBuf::from(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("qcow2")
        .to_lowercase();
    let object_key = format!("{id}.{format}");
    let dest = state.config.storage_path.join(&object_key);
    tokio::fs::copy(&abs, &dest)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let size_bytes = meta.len() as i64;
    register_imported_vm(&state, id, filename, &object_key, &format, size_bytes).await?;
    Ok(Json(ApiResponse::ok(VmImportResponse {
        id,
        name: filename.to_string(),
        format,
        size_bytes,
        path: dest.display().to_string(),
    })))
}
