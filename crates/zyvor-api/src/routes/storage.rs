// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Browse server-side disk storage and register images without re-upload.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::{ApiResponse, VmImage, VmImportResponse};
use crate::state::AppState;

const DISK_EXTENSIONS: &[&str] = &["qcow2", "vmdk", "raw", "img", "ova", "vpc", "vdi"];

#[derive(Debug, Clone, Serialize)]
pub struct StorageRoot {
    pub id: usize,
    pub label: String,
    pub path: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: Option<i64>,
    pub format: Option<String>,
    pub registered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vm_id: Option<Uuid>,
    pub modified: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StorageBrowseResult {
    pub root_id: usize,
    pub root_label: String,
    pub path: String,
    pub parent: Option<String>,
    pub entries: Vec<StorageEntry>,
}

#[derive(Debug, Deserialize, Default)]
pub struct BrowseQuery {
    #[serde(default)]
    pub root: Option<usize>,
    #[serde(default)]
    pub path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ImportFromStorageRequest {
    pub root: Option<usize>,
    pub path: String,
}

pub async fn list_storage_roots(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<Vec<StorageRoot>>>> {
    Ok(Json(ApiResponse::ok(storage_roots(&state))))
}

pub async fn browse_storage(
    State(state): State<AppState>,
    Query(query): Query<BrowseQuery>,
) -> ApiResult<Json<ApiResponse<StorageBrowseResult>>> {
    let root_id = query.root.unwrap_or(0);
    let rel = normalize_rel_path(query.path.as_deref().unwrap_or(""));
    let (root, abs) = resolve_browse_path(&state, root_id, &rel)?;
    let meta = tokio::fs::metadata(&abs)
        .await
        .map_err(|e| ApiError::bad_request(format!("path not found: {e}")))?;
    if !meta.is_dir() {
        return Err(ApiError::bad_request("path is not a directory"));
    }

    let mut entries = Vec::new();
    let mut read_dir = tokio::fs::read_dir(&abs)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    while let Some(entry) = read_dir
        .next_entry()
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?
    {
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let entry_path = entry.path();
        let entry_meta = entry
            .metadata()
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let child_rel = if rel.is_empty() {
            name.clone()
        } else {
            format!("{rel}/{name}")
        };

        if entry_meta.is_dir() {
            entries.push(StorageEntry {
                name,
                path: child_rel,
                kind: "directory".into(),
                size_bytes: None,
                format: None,
                registered: false,
                vm_id: None,
                modified: modified_iso(&entry_meta),
            });
            continue;
        }

        if !entry_meta.is_file() {
            continue;
        }

        let format = disk_format(&name);
        if format.is_none() {
            continue;
        }
        let reg = lookup_registered(&state, &name, &entry_path).await?;
        entries.push(StorageEntry {
            name,
            path: child_rel,
            kind: "file".into(),
            size_bytes: Some(entry_meta.len() as i64),
            format,
            registered: reg.is_some(),
            vm_id: reg,
            modified: modified_iso(&entry_meta),
        });
    }

    entries.sort_by(|a, b| {
        a.kind
            .cmp(&b.kind)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    let parent = parent_rel(&rel);
    Ok(Json(ApiResponse::ok(StorageBrowseResult {
        root_id,
        root_label: root_label(&root),
        path: rel,
        parent,
        entries,
    })))
}

pub async fn import_from_storage(
    State(state): State<AppState>,
    Json(body): Json<ImportFromStorageRequest>,
) -> ApiResult<Json<ApiResponse<VmImportResponse>>> {
    let root_id = body.root.unwrap_or(0);
    let rel = normalize_rel_path(&body.path);
    let (_root, abs) = resolve_browse_path(&state, root_id, &rel)?;
    let meta = tokio::fs::metadata(&abs)
        .await
        .map_err(|e| ApiError::bad_request(format!("file not found: {e}")))?;
    if !meta.is_file() {
        return Err(ApiError::bad_request("path is not a file"));
    }
    let filename = abs
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| ApiError::bad_request("invalid filename"))?;
    let format = disk_format(filename).ok_or_else(|| {
        ApiError::bad_request("unsupported disk format — use qcow2, vmdk, raw, img, ova, vpc, or vdi")
    })?;

    if let Some(existing) = lookup_registered(&state, filename, &abs).await? {
        let row = load_vm(&state, existing).await?;
        return Ok(Json(ApiResponse::ok(VmImportResponse {
            id: row.id,
            name: row.name,
            format: row.format,
            size_bytes: row.size_bytes,
            path: abs.display().to_string(),
        })));
    }

    let size_bytes = meta.len() as i64;
    let storage_root = state.config.storage_path.canonicalize().unwrap_or_else(|_| {
        state.config.storage_path.clone()
    });
    let abs_canon = abs
        .canonicalize()
        .map_err(|e| ApiError::bad_request(e.to_string()))?;

    let (object_key, dest_path) = if abs_canon.starts_with(&storage_root) {
        let key = unique_object_key(&state, filename).await?;
        (key.clone(), storage_root.join(&key))
    } else {
        let id = Uuid::new_v4();
        let key = format!("{id}.{format}");
        (key.clone(), storage_root.join(&key))
    };

    if abs_canon != dest_path {
        tokio::fs::create_dir_all(&storage_root)
            .await
            .map_err(|e| ApiError::internal(e.to_string()))?;
        tokio::fs::copy(&abs_canon, &dest_path)
            .await
            .map_err(|e| ApiError::internal(format!("copy disk: {e}")))?;
    } else if object_key != filename {
        tokio::fs::rename(&abs_canon, &dest_path)
            .await
            .map_err(|e| ApiError::internal(format!("rename disk: {e}")))?;
    }

    let id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO vm_images (id, tenant, name, object_key, format, size_bytes, status)
           VALUES ($1, $2, $3, $4, $5, $6, $7)"#,
    )
    .bind(id)
    .bind("default")
    .bind(filename)
    .bind(&object_key)
    .bind(&format)
    .bind(size_bytes)
    .bind("imported")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(Json(ApiResponse::ok(VmImportResponse {
        id,
        name: filename.to_string(),
        format,
        size_bytes,
        path: dest_path.display().to_string(),
    })))
}

fn storage_roots(state: &AppState) -> Vec<StorageRoot> {
    state
        .config
        .storage_browse_roots()
        .into_iter()
        .enumerate()
        .map(|(id, path)| StorageRoot {
            id,
            label: root_label(&path),
            path: path.display().to_string(),
        })
        .collect()
}

fn root_label(path: &Path) -> String {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(String::from)
        .unwrap_or_else(|| path.display().to_string())
}

fn normalize_rel_path(path: &str) -> String {
    path.trim()
        .trim_start_matches('/')
        .split('/')
        .filter(|p| !p.is_empty() && *p != ".")
        .take(32)
        .collect::<Vec<_>>()
        .join("/")
}

fn parent_rel(path: &str) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let mut parts: Vec<&str> = path.split('/').collect();
    parts.pop();
    if parts.is_empty() {
        Some(String::new())
    } else {
        Some(parts.join("/"))
    }
}

fn disk_format(name: &str) -> Option<String> {
    Path::new(name)
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_lowercase())
        .filter(|e| DISK_EXTENSIONS.contains(&e.as_str()))
}

fn modified_iso(meta: &std::fs::Metadata) -> Option<String> {
    meta.modified().ok().map(|t| {
        chrono::DateTime::<chrono::Utc>::from(t)
            .format("%Y-%m-%d %H:%M")
            .to_string()
    })
}

fn resolve_browse_path(
    state: &AppState,
    root_id: usize,
    rel: &str,
) -> ApiResult<(PathBuf, PathBuf)> {
    if rel.contains("..") {
        return Err(ApiError::bad_request("invalid path"));
    }
    let roots = state.config.storage_browse_roots();
    let root = roots
        .get(root_id)
        .ok_or_else(|| ApiError::bad_request("invalid storage root"))?
        .clone();
    let root_canon = root
        .canonicalize()
        .map_err(|e| ApiError::internal(format!("storage root unavailable: {e}")))?;
    let abs = if rel.is_empty() {
        root_canon.clone()
    } else {
        root_canon.join(rel)
    };
    let abs_canon = abs
        .canonicalize()
        .map_err(|e| ApiError::bad_request(format!("path not found: {e}")))?;
    if !abs_canon.starts_with(&root_canon) {
        return Err(ApiError::bad_request("path outside storage root"));
    }
    Ok((root_canon, abs_canon))
}

async fn lookup_registered(
    state: &AppState,
    filename: &str,
    abs: &Path,
) -> ApiResult<Option<Uuid>> {
    let object_key = abs
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or(filename);
    let row = sqlx::query_as::<_, VmImage>(
        r#"SELECT id, tenant, name, object_key, format, size_bytes, checksum, status, created_at
           FROM vm_images
           WHERE object_key = $1 OR name = $1 OR name = $2
           LIMIT 1"#,
    )
    .bind(object_key)
    .bind(filename)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(row.map(|r| r.id))
}

async fn unique_object_key(state: &AppState, filename: &str) -> ApiResult<String> {
    let exists = sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM vm_images WHERE object_key = $1")
        .bind(filename)
        .fetch_one(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if exists == 0 {
        return Ok(filename.to_string());
    }
    let format = disk_format(filename).unwrap_or_else(|| "qcow2".into());
    Ok(format!("{}.{}", Uuid::new_v4(), format))
}

async fn load_vm(state: &AppState, id: Uuid) -> ApiResult<VmImage> {
    sqlx::query_as::<_, VmImage>(
        "SELECT id, tenant, name, object_key, format, size_bytes, checksum, status, created_at FROM vm_images WHERE id = $1",
    )
    .bind(id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found("VM image not found"))
}
