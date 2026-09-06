// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use guestkit_job_spec::{builder::JobBuilder, JobDocument};
use redis::aio::ConnectionManager;
use redis::AsyncCommands;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::{ApiError, ApiResult};
use crate::models::{JobEnqueueResponse, JobRecord};
use crate::state::AppState;

pub const JOBS_STREAM: &str = "zyvor:jobs";

pub async fn enqueue_job(redis: &mut ConnectionManager, job: &JobDocument) -> anyhow::Result<()> {
    let job_json = serde_json::to_string(job).map_err(|e| anyhow::anyhow!("serialize job: {e}"))?;

    let _: String = redis::cmd("XADD")
        .arg(JOBS_STREAM)
        .arg("*")
        .arg("job")
        .arg(job_json)
        .query_async(redis)
        .await
        .map_err(|e| anyhow::anyhow!("XADD job: {e}"))?;

    let status_key = format!("zyvor:job-status:{}", job.job_id);
    let status = serde_json::json!({
        "job_id": job.job_id,
        "status": "pending",
        "updated_at": Utc::now().to_rfc3339(),
    });
    redis
        .set_ex::<_, _, ()>(&status_key, status.to_string(), 86400)
        .await
        .map_err(|e| anyhow::anyhow!("set job status: {e}"))?;
    Ok(())
}

pub async fn get_job_status(redis: &mut ConnectionManager, job_id: &str) -> Option<serde_json::Value> {
    let key = format!("zyvor:job-status:{job_id}");
    let raw: Option<String> = redis.get(&key).await.ok()?;
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

pub async fn get_job_result(redis: &mut ConnectionManager, job_id: &str) -> Option<serde_json::Value> {
    let key = format!("zyvor:results:{job_id}");
    let raw: Option<String> = redis.get(&key).await.ok()?;
    raw.and_then(|s| serde_json::from_str(&s).ok())
}

pub async fn hydrate_job_record(state: &AppState, record: &mut JobRecord, sync_db: bool) {
    let mut redis = state.redis.clone();
    let Some(live) = get_job_status(&mut redis, &record.id.to_string()).await else {
        return;
    };
    let status = live
        .get("status")
        .and_then(|s| s.as_str())
        .unwrap_or("");
    let updated_at = live
        .get("updated_at")
        .and_then(|s| s.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc));

    if !status.is_empty() && status != record.status {
        record.status = status.to_string();
    }
    if matches!(status, "completed" | "failed" | "cancelled" | "running") {
        if record.completed_at.is_none() && matches!(status, "completed" | "failed" | "cancelled") {
            record.completed_at = updated_at;
        }
        if sync_db {
            sync_job_to_db(&state.pool, record.id, &record.status, record.completed_at).await;
        }
    }
}

async fn sync_job_to_db(
    pool: &PgPool,
    id: Uuid,
    status: &str,
    completed_at: Option<DateTime<Utc>>,
) {
    let _ = sqlx::query(
        "UPDATE jobs SET status = $1, completed_at = COALESCE($2, completed_at) WHERE id = $3",
    )
    .bind(status)
    .bind(completed_at)
    .bind(id)
    .execute(pool)
    .await;
}

pub fn build_job(
    job_id: Uuid,
    operation: &str,
    payload_type: &str,
    data: serde_json::Value,
) -> anyhow::Result<JobDocument> {
    JobBuilder::new()
        .job_id(job_id.to_string())
        .operation(operation)
        .payload(payload_type, data)
        .timeout_seconds(7200)
        .build()
        .map_err(|e| anyhow::anyhow!("{e}"))
}

pub async fn submit_disk_path_job(
    state: &AppState,
    vm_id: Uuid,
    disk_path: &std::path::Path,
    format: &str,
    operation: &str,
    payload_type: &str,
    mut data: serde_json::Value,
) -> ApiResult<JobEnqueueResponse> {
    let job_id = Uuid::new_v4();
    if let Some(obj) = data.as_object_mut() {
        obj.insert(
            "image".into(),
            serde_json::json!({
                "path": disk_path.display().to_string(),
                "format": format,
            }),
        );
    }

    let job = build_job(job_id, operation, payload_type, data)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let mut redis = state.redis.clone();
    enqueue_job(&mut redis, &job)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;

    sqlx::query(
        "INSERT INTO jobs (id, vm_id, operation, status) VALUES ($1, $2, $3, $4)",
    )
    .bind(job_id)
    .bind(vm_id)
    .bind(operation)
    .bind("pending")
    .execute(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?;

    Ok(JobEnqueueResponse {
        job_id,
        operation: operation.to_string(),
        status: "pending".to_string(),
    })
}
