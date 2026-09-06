// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Guestkit explore handler — read-only directory listing and file preview

use async_trait::async_trait;
use guestkit_job_spec::Payload;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use crate::error::{WorkerError, WorkerResult};
use crate::handler::{HandlerContext, HandlerResult, OperationHandler};

const MAX_CAT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
enum ExploreAction {
    Ls,
    Stat,
    Cat,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ExplorePayload {
    image: ImageSpec,
    #[serde(default = "default_action")]
    action: ExploreAction,
    #[serde(default = "default_path")]
    path: String,
}

fn default_action() -> ExploreAction {
    ExploreAction::Ls
}

fn default_path() -> String {
    "/".to_string()
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ImageSpec {
    path: String,
    #[serde(default = "default_format")]
    format: String,
}

fn default_format() -> String {
    "qcow2".to_string()
}

pub struct ExploreHandler;

fn normalize_guest_path(path: &str) -> String {
    let trimmed = path.trim();
    if trimmed.is_empty() {
        return "/".to_string();
    }
    let mut out = if trimmed.starts_with('/') {
        trimmed.to_string()
    } else {
        format!("/{trimmed}")
    };
    while out.contains("//") {
        out = out.replace("//", "/");
    }
    if out.len() > 1 && out.ends_with('/') {
        out.pop();
    }
    out
}

#[async_trait]
impl OperationHandler for ExploreHandler {
    fn name(&self) -> &str {
        "guestkit-explore"
    }

    fn operations(&self) -> Vec<String> {
        vec![guestkit_job_spec::operations::GUESTKIT_EXPLORE.to_string()]
    }

    async fn validate(&self, payload: &Payload) -> WorkerResult<()> {
        let p: ExplorePayload = serde_json::from_value(payload.data.clone())
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;
        if p.image.path.is_empty() {
            return Err(WorkerError::ExecutionError("image.path is required".into()));
        }
        Ok(())
    }

    async fn execute(
        &self,
        context: HandlerContext,
        payload: Payload,
    ) -> WorkerResult<HandlerResult> {
        let p: ExplorePayload = serde_json::from_value(payload.data)
            .map_err(|e| WorkerError::ExecutionError(e.to_string()))?;
        let guest_path = normalize_guest_path(&p.path);
        let action = p.action.clone();
        let image = PathBuf::from(p.image.path);

        context
            .report_progress("explore", Some(10), &format!("Mounting for {:?}", action))
            .await?;

        let data = tokio::task::spawn_blocking(move || -> WorkerResult<serde_json::Value> {
            use guestkit::Guestfs;

            let mut g = Guestfs::new()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to create Guestfs: {e}")))?;
            g.add_drive_ro(image.to_string_lossy().as_ref())
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to add drive: {e}")))?;
            g.launch()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to launch: {e}")))?;

            let inspected = g
                .inspect()
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to inspect: {e}")))?;
            if inspected.is_empty() {
                return Err(WorkerError::ExecutionError(
                    "No operating system found in image".into(),
                ));
            }
            let os_info = &inspected[0];
            g.mount_ro(&os_info.root, "/")
                .map_err(|e| WorkerError::ExecutionError(format!("Failed to mount: {e}")))?;

            let result = match action {
                ExploreAction::Ls => {
                    let names = g.ls(&guest_path).map_err(|e| {
                        WorkerError::ExecutionError(format!("ls {guest_path}: {e}"))
                    })?;
                    let mut entries = Vec::new();
                    for name in names {
                        if name == "." || name == ".." {
                            continue;
                        }
                        let full = if guest_path == "/" {
                            format!("/{name}")
                        } else {
                            format!("{guest_path}/{name}")
                        };
                        let is_dir = g.is_dir(&full).unwrap_or(false);
                        let is_file = g.is_file(&full).unwrap_or(false);
                        entries.push(serde_json::json!({
                            "name": name,
                            "path": full,
                            "is_dir": is_dir,
                            "is_file": is_file,
                        }));
                    }
                    entries.sort_by(|a, b| {
                        let ad = a.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                        let bd = b.get("is_dir").and_then(|v| v.as_bool()).unwrap_or(false);
                        match (ad, bd) {
                            (true, false) => std::cmp::Ordering::Less,
                            (false, true) => std::cmp::Ordering::Greater,
                            _ => a
                                .get("name")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .cmp(b.get("name").and_then(|v| v.as_str()).unwrap_or("")),
                        }
                    });
                    serde_json::json!({
                        "action": "ls",
                        "path": guest_path,
                        "entries": entries,
                    })
                }
                ExploreAction::Stat => {
                    let exists = g.exists(&guest_path).unwrap_or(false);
                    let is_dir = g.is_dir(&guest_path).unwrap_or(false);
                    let is_file = g.is_file(&guest_path).unwrap_or(false);
                    serde_json::json!({
                        "action": "stat",
                        "path": guest_path,
                        "exists": exists,
                        "is_dir": is_dir,
                        "is_file": is_file,
                    })
                }
                ExploreAction::Cat => {
                    if !g.is_file(&guest_path).unwrap_or(false) {
                        return Err(WorkerError::ExecutionError(format!(
                            "not a file: {guest_path}"
                        )));
                    }
                    let content = g.cat(&guest_path).map_err(|e| {
                        WorkerError::ExecutionError(format!("cat {guest_path}: {e}"))
                    })?;
                    let truncated = content.len() > MAX_CAT_BYTES;
                    let preview: String = content.chars().take(MAX_CAT_BYTES).collect();
                    serde_json::json!({
                        "action": "cat",
                        "path": guest_path,
                        "truncated": truncated,
                        "size": content.len(),
                        "content": preview,
                    })
                }
            };

            let _ = g.umount_all();
            let _ = g.shutdown();
            Ok(result)
        })
        .await
        .map_err(|e| WorkerError::ExecutionError(format!("Task join error: {e}")))??;

        context
            .report_progress("complete", Some(100), "Explore complete")
            .await?;

        Ok(HandlerResult::new().with_data(data))
    }
}
