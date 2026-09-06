// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! PacketWolf fleet correlation API.

use axum::extract::State;
use axum::Extension;
use axum::Json;

use crate::auth::types::AuthUserClaims;
use crate::error::ApiResult;
use crate::guest_remediation_auth::require_guest_remediation_requester;
use crate::models::ApiResponse;
use crate::state::AppState;

pub async fn get_fleet_snapshot(
    State(state): State<AppState>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    let mut redis = state.redis.clone();
    let snapshot = crate::packetwolf_fleet::last_fleet_snapshot(&mut redis)
        .await
        .unwrap_or(serde_json::json!({}));
    Ok(Json(ApiResponse::ok(snapshot)))
}

pub async fn trigger_fleet_correlation(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
) -> ApiResult<Json<ApiResponse<serde_json::Value>>> {
    require_guest_remediation_requester(&state, user.as_deref())?;
    let mut redis = state.redis.clone();
    let url = crate::packetwolf_fleet::fleet_url_public(&state.config);
    crate::packetwolf_fleet::run_fleet_correlation(
        &state.config,
        state.kube.as_ref(),
        &mut redis,
        url.as_deref(),
    )
    .await
    .map_err(|e| crate::error::ApiError::internal(e))?;
    let snapshot = crate::packetwolf_fleet::last_fleet_snapshot(&mut redis)
        .await
        .unwrap_or(serde_json::json!({ "triggered": true }));
    Ok(Json(ApiResponse::ok(snapshot)))
}
