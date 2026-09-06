// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Resolve guest agent pull path via Guest Control Fabric transport ladder.

use serde_json::Value;

use crate::error::ApiError;
use crate::guest_control::transport::pull_method;
use crate::routes::guest_agent::pull_guest_rpc;
use crate::state::AppState;

pub async fn pull_for_vm(
    state: &AppState,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
) -> Result<Value, String> {
    match pull_method(state, namespace, name, method, params.clone()).await {
        Ok(result) => Ok(result.value),
        Err(primary) => {
            if let Some(proxy) = state.config.agent_proxy_url.as_ref() {
                return pull_guest_rpc(proxy, method, params.clone()).await;
            }
            Err(primary)
        }
    }
}

pub async fn pull_for_vm_api(
    state: &AppState,
    namespace: &str,
    name: &str,
    method: &str,
    params: Value,
) -> Result<Value, ApiError> {
    pull_for_vm(state, namespace, name, method, params)
        .await
        .map_err(ApiError::bad_request)
}

pub fn rpc_result(resp: Value) -> Value {
    resp.get("result").cloned().unwrap_or(resp)
}
