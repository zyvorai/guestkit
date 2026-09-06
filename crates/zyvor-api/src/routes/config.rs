// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::State;
use axum::Json;
use serde::Serialize;

use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct UiConfig {
    pub zeus_url: Option<String>,
    pub cluster_name: Option<String>,
    pub default_namespace: String,
    pub storage_class: String,
    pub storage_path: String,
    pub agent_proxy_url: Option<String>,
    pub auth_enabled: bool,
}

pub async fn get_config(
    State(state): State<AppState>,
) -> Json<ApiResponse<UiConfig>> {
    Json(ApiResponse::ok(UiConfig {
        zeus_url: state.config.zeus_public_url.clone(),
        cluster_name: state.config.cluster_name.clone(),
        default_namespace: state.config.default_namespace.clone(),
        storage_class: state.config.storage_class.clone(),
        storage_path: state.config.storage_path.display().to_string(),
        agent_proxy_url: state.config.agent_proxy_url.clone(),
        auth_enabled: state.config.auth_enabled,
    }))
}
