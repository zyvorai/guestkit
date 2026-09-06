// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Standard response envelope for guest control routes.

use serde::Serialize;
use serde_json::Value;

use super::capabilities::{ControlState, GuestCapabilityContract, GuestTransport};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GuestControlEnvelope {
    pub ok: bool,
    pub transport: String,
    pub network_required: bool,
    pub control_state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capabilities: Option<GuestCapabilityContract>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub recommended_actions: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl GuestControlEnvelope {
    pub fn success(
        transport: GuestTransport,
        control_state: ControlState,
        capabilities: Option<GuestCapabilityContract>,
        data: Value,
    ) -> Self {
        let network_required = capabilities
            .as_ref()
            .map(|c| c.network)
            .unwrap_or(false);
        let warnings = capabilities
            .as_ref()
            .map(|c| c.warnings.clone())
            .unwrap_or_default();
        let recommended_actions = capabilities
            .as_ref()
            .map(|c| c.recommended_actions.clone())
            .unwrap_or_default();
        Self {
            ok: true,
            transport: transport.as_str().to_string(),
            network_required,
            control_state: control_state.as_str().to_string(),
            capabilities,
            warnings,
            recommended_actions,
            data: Some(data),
        }
    }

    pub fn failure(
        control_state: ControlState,
        message: &str,
        recommended_actions: Vec<String>,
    ) -> Self {
        Self {
            ok: false,
            transport: GuestTransport::ConsoleOnly.as_str().to_string(),
            network_required: false,
            control_state: control_state.as_str().to_string(),
            capabilities: None,
            warnings: vec![message.to_string()],
            recommended_actions,
            data: Some(serde_json::json!({ "error": message })),
        }
    }
}
