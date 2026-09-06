// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Pending guest remediation actions (approval workflow stub).

use axum::extract::{Path, Query, State};
use axum::Extension;
use axum::Json;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::auth::rbac::can_approve_guest_actions;
use crate::auth::types::AuthUserClaims;
use crate::error::{ApiError, ApiResult};
use crate::guest_action_policy::{enforce_restart_unit, fetch_guest_action_policy};
use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingGuestAction {
    pub id: String,
    pub action: String,
    pub namespace: String,
    pub vm_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    pub status: String,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approved_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rejected_by: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
}

fn pending_key(id: &str) -> String {
    format!("guest-action:pending:{id}")
}

const PENDING_INDEX: &str = "guest-action:pending:index";
const AUDIT_INDEX: &str = "guest-action:audit:index";

#[derive(Debug, Deserialize)]
pub struct GuestActionAuditQuery {
    pub limit: Option<usize>,
}

async fn track_audit(
    redis: &mut redis::aio::ConnectionManager,
    id: &str,
) -> Result<(), ApiError> {
    redis
        .sadd::<_, _, ()>(AUDIT_INDEX, id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

pub async fn enqueue_restart_unit(
    redis: &mut redis::aio::ConnectionManager,
    namespace: &str,
    vm_name: &str,
    unit: &str,
    requested_by: Option<&str>,
) -> Result<String, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let action = PendingGuestAction {
        id: id.clone(),
        action: "restart_unit".into(),
        namespace: namespace.into(),
        vm_name: vm_name.into(),
        unit: Some(unit.into()),
        status: "pending".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        requested_by: requested_by.map(String::from),
        approved_at: None,
        approved_by: None,
        rejected_at: None,
        rejected_by: None,
        result: None,
    };
    let raw = serde_json::to_string(&action).map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(pending_key(&id), &raw, 86400)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .sadd::<_, _, ()>(PENDING_INDEX, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    track_audit(redis, &id).await?;
    Ok(id)
}

pub async fn enqueue_support_bundle(
    redis: &mut redis::aio::ConnectionManager,
    namespace: &str,
    vm_name: &str,
    requested_by: Option<&str>,
) -> Result<String, ApiError> {
    let id = uuid::Uuid::new_v4().to_string();
    let action = PendingGuestAction {
        id: id.clone(),
        action: "collect_support_bundle".into(),
        namespace: namespace.into(),
        vm_name: vm_name.into(),
        unit: None,
        status: "pending".into(),
        created_at: chrono::Utc::now().to_rfc3339(),
        requested_by: requested_by.map(String::from),
        approved_at: None,
        approved_by: None,
        rejected_at: None,
        rejected_by: None,
        result: None,
    };
    let raw = serde_json::to_string(&action).map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(pending_key(&id), &raw, 86400)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .sadd::<_, _, ()>(PENDING_INDEX, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    track_audit(redis, &id).await?;
    Ok(id)
}

async fn load_pending(
    redis: &mut redis::aio::ConnectionManager,
    id: &str,
) -> ApiResult<PendingGuestAction> {
    let raw: Option<String> = redis
        .get(pending_key(id))
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let action = raw
        .and_then(|s| serde_json::from_str(&s).ok())
        .ok_or_else(|| ApiError::bad_request("pending action not found"))?;
    Ok(action)
}

async fn save_pending(
    redis: &mut redis::aio::ConnectionManager,
    action: &PendingGuestAction,
) -> ApiResult<()> {
    let raw = serde_json::to_string(action).map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(pending_key(&action.id), &raw, 86400)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}

async fn execute_pending(state: &AppState, action: &PendingGuestAction) -> ApiResult<Value> {
    match action.action.as_str() {
        "restart_unit" => {
            let unit = action
                .unit
                .as_deref()
                .ok_or_else(|| ApiError::bad_request("unit missing on pending action"))?;
            enforce_restart_unit(state.kube.as_ref(), unit, true).await?;
            let resp = crate::kubevirt_guest_pull::pull_for_vm_api(
                state,
                &action.namespace,
                &action.vm_name,
                "guestkit.restartUnit",
                json!({ "unit": unit }),
            )
            .await?;
            Ok(json!({
                "namespace": action.namespace,
                "name": action.vm_name,
                "unit": unit,
                "result": resp,
            }))
        }
        "collect_support_bundle" => {
            crate::guest_action_policy::enforce_support_bundle(state.kube.as_ref(), true).await?;
            let resp = crate::kubevirt_guest_pull::pull_for_vm_api(
                state,
                &action.namespace,
                &action.vm_name,
                "guestkit.collectSupportBundle",
                json!({}),
            )
            .await?;
            Ok(json!({
                "namespace": action.namespace,
                "name": action.vm_name,
                "bundle": crate::kubevirt_guest_pull::rpc_result(resp),
            }))
        }
        other => Err(ApiError::bad_request(format!("unsupported action: {other}"))),
    }
}

fn require_guest_action_approver(
    state: &AppState,
    user: Option<&AuthUserClaims>,
) -> ApiResult<()> {
    if !state.config.auth_enabled {
        return Ok(());
    }
    let user = user.ok_or_else(|| ApiError::unauthorized("authentication required"))?;
    if !can_approve_guest_actions(user) {
        return Err(ApiError::forbidden("operator or admin role required"));
    }
    Ok(())
}

fn actor_label(user: Option<&AuthUserClaims>) -> Option<String> {
    user.map(|u| {
        u.email
            .clone()
            .or_else(|| u.name.clone())
            .unwrap_or_else(|| u.sub.clone())
    })
}

pub async fn list_pending_guest_actions(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
) -> ApiResult<Json<ApiResponse<Vec<PendingGuestAction>>>> {
    require_guest_action_approver(&state, user.as_deref())?;
    let mut redis = state.redis.clone();
    let ids: Vec<String> = redis
        .smembers(PENDING_INDEX)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut out = Vec::new();
    for id in ids {
        if let Ok(action) = load_pending(&mut redis, &id).await {
            if action.status == "pending" {
                out.push(action);
            }
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    Ok(Json(ApiResponse::ok(out)))
}

pub async fn list_guest_action_audit(
    State(state): State<AppState>,
    Query(query): Query<GuestActionAuditQuery>,
    user: Option<Extension<AuthUserClaims>>,
) -> ApiResult<Json<ApiResponse<Vec<PendingGuestAction>>>> {
    require_guest_action_approver(&state, user.as_deref())?;
    let limit = query.limit.unwrap_or(50).clamp(1, 200);
    let mut redis = state.redis.clone();
    let ids: Vec<String> = redis
        .smembers(AUDIT_INDEX)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut out = Vec::new();
    for id in ids {
        if let Ok(action) = load_pending(&mut redis, &id).await {
            out.push(action);
        }
    }
    out.sort_by(|a, b| b.created_at.cmp(&a.created_at));
    out.truncate(limit);
    Ok(Json(ApiResponse::ok(out)))
}

pub async fn approve_guest_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<AuthUserClaims>>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    require_guest_action_approver(&state, user.as_deref())?;
    let approver = actor_label(user.as_deref());
    let mut redis = state.redis.clone();
    let mut action = load_pending(&mut redis, &id).await?;
    if action.status != "pending" {
        return Err(ApiError::bad_request("action is not pending"));
    }
    let result = execute_pending(&state, &action).await?;
    action.status = "approved".into();
    action.approved_at = Some(chrono::Utc::now().to_rfc3339());
    action.approved_by = approver;
    action.result = Some(result.clone());
    save_pending(&mut redis, &action).await?;
    redis
        .srem::<_, _, ()>(PENDING_INDEX, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(json!({
        "action_id": id,
        "status": "approved",
        "result": result,
    }))))
}

pub async fn reject_guest_action(
    State(state): State<AppState>,
    Path(id): Path<String>,
    user: Option<Extension<AuthUserClaims>>,
) -> ApiResult<Json<ApiResponse<Value>>> {
    require_guest_action_approver(&state, user.as_deref())?;
    let rejector = actor_label(user.as_deref());
    let mut redis = state.redis.clone();
    let mut action = load_pending(&mut redis, &id).await?;
    if action.status != "pending" {
        return Err(ApiError::bad_request("action is not pending"));
    }
    action.status = "rejected".into();
    action.rejected_at = Some(chrono::Utc::now().to_rfc3339());
    action.rejected_by = rejector;
    save_pending(&mut redis, &action).await?;
    redis
        .srem::<_, _, ()>(PENDING_INDEX, &id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(json!({
        "action_id": id,
        "status": "rejected",
    }))))
}

pub async fn policy_requires_approval(client: Option<&kube::Client>) -> bool {
    if let Some(client) = client {
        if let Some(policy) = fetch_guest_action_policy(client).await {
            return policy.require_approval;
        }
    }
    false
}

const ACTION_AUDIT_INDEX: &str = "guest-action:audit:events";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GuestActionAuditEvent {
    pub id: String,
    pub action: String,
    pub namespace: String,
    pub vm_name: String,
    pub transport: String,
    pub network_required: bool,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

pub async fn record_guest_action_audit(
    redis: &mut redis::aio::ConnectionManager,
    action: &str,
    namespace: &str,
    vm_name: &str,
    transport: &str,
    network_required: bool,
    detail: Value,
) -> Result<(), ApiError> {
    let event = GuestActionAuditEvent {
        id: uuid::Uuid::new_v4().to_string(),
        action: action.into(),
        namespace: namespace.into(),
        vm_name: vm_name.into(),
        transport: transport.into(),
        network_required,
        created_at: chrono::Utc::now().to_rfc3339(),
        detail: Some(detail),
    };
    let raw = serde_json::to_string(&event).map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .set_ex::<_, _, ()>(
            format!("guest-action:audit:event:{}", event.id),
            &raw,
            86400 * 7,
        )
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    redis
        .sadd::<_, _, ()>(ACTION_AUDIT_INDEX, &event.id)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(())
}
