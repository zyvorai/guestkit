// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::{Path, State};
use axum::Json;
use uuid::Uuid;
use crate::error::{ApiError, ApiResult};
use crate::jobs::{get_job_result, get_job_status, hydrate_job_record};
use crate::models::{ApiResponse, JobRecord};
use crate::state::AppState;

pub async fn get_job(
    State(state): State<AppState>,
    Path(job_id): Path<Uuid>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    let mut record = sqlx::query_as::<_, JobRecord>(
        "SELECT id, vm_id, operation, status, worker_id, submitted_at, completed_at FROM jobs WHERE id = $1",
    )
    .bind(job_id)
    .fetch_optional(&state.pool)
    .await
    .map_err(|e| ApiError::internal(e.to_string()))?
    .ok_or_else(|| ApiError::not_found(format!("Job {job_id} not found")))?;

    hydrate_job_record(&state, &mut record, true).await;

    let mut redis = state.redis.clone();
    let redis_status = get_job_status(&mut redis, &job_id.to_string()).await;
    let redis_result = get_job_result(&mut redis, &job_id.to_string()).await;

    let mut out = serde_json::to_value(&record).map_err(|e| ApiError::internal(e.to_string()))?;
    if let Some(obj) = out.as_object_mut() {
        if let Some(s) = redis_status {
            obj.insert("live_status".into(), s);
        }
        if let Some(r) = redis_result {
            obj.insert("result".into(), r);
        }
    }

    Ok(Json(ApiResponse::ok(out)))
}
