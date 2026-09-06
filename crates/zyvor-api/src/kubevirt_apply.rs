// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Apply KubeVirt YAML manifests to the cluster.

use axum::extract::State;
use axum::Json;
use kube::api::{ApiResource, DynamicObject, PostParams};
use kube::Client;
use serde::Deserialize;
use serde::Serialize;
use serde_json::Value;

use crate::error::{ApiError, ApiResult};
use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ApplyYamlRequest {
    pub yaml: String,
}

#[derive(Debug, Serialize)]
pub struct AppliedResource {
    pub kind: String,
    pub name: String,
    pub namespace: Option<String>,
    pub action: String,
}

#[derive(Debug, Serialize)]
pub struct ApplyYamlResult {
    pub applied: bool,
    pub resources: Vec<AppliedResource>,
    pub errors: Vec<String>,
}

pub async fn apply_yaml_handler(
    State(state): State<AppState>,
    Json(body): Json<ApplyYamlRequest>,
) -> ApiResult<Json<ApiResponse<ApplyYamlResult>>> {
    let client = state
        .kube
        .clone()
        .ok_or_else(|| ApiError::bad_request("Apply requires in-cluster Kubernetes access"))?;
    let result = apply_kubevirt_manifests(&client, &body.yaml).await?;
    Ok(Json(ApiResponse::ok(result)))
}

pub async fn apply_kubevirt_manifests(client: &Client, yaml: &str) -> ApiResult<ApplyYamlResult> {
    let mut resources = Vec::new();
    let mut errors = Vec::new();

    for doc in split_yaml_documents(yaml) {
        let value: Value = match serde_yaml::from_str(&doc) {
            Ok(v) => v,
            Err(e) => {
                errors.push(format!("YAML parse: {e}"));
                continue;
            }
        };
        let kind = value
            .get("kind")
            .and_then(|k| k.as_str())
            .unwrap_or("")
            .to_string();
        if kind.is_empty() {
            continue;
        }
        let name = value
            .get("metadata")
            .and_then(|m| m.get("name"))
            .and_then(|n| n.as_str())
            .unwrap_or("")
            .to_string();
        let namespace = value
            .get("metadata")
            .and_then(|m| m.get("namespace"))
            .and_then(|n| n.as_str())
            .map(String::from);

        match apply_one(client, &value).await {
            Ok(action) => resources.push(AppliedResource {
                kind,
                name,
                namespace,
                action,
            }),
            Err(e) => errors.push(format!("{kind}/{name}: {e}")),
        }
    }

    Ok(ApplyYamlResult {
        applied: errors.is_empty() && !resources.is_empty(),
        resources,
        errors,
    })
}

async fn apply_one(client: &Client, value: &Value) -> Result<String, String> {
    let api_version = value
        .get("apiVersion")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing apiVersion".to_string())?;
    let kind = value
        .get("kind")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "missing kind".to_string())?;
    let (group, version) = parse_api_version(api_version);
    let ar = ApiResource {
        group: group.to_string(),
        version: version.to_string(),
        api_version: api_version.to_string(),
        kind: kind.to_string(),
        plural: plural_for_kind(kind),
    };
    let obj: DynamicObject = serde_json::from_value(value.clone())
        .map_err(|e| format!("dynamic object: {e}"))?;
    let name = obj.metadata.name.clone().ok_or("missing metadata.name")?;
    let ns = obj
        .metadata
        .namespace
        .clone()
        .unwrap_or_else(|| "default".to_string());

    let api: kube::Api<DynamicObject> = kube::Api::namespaced_with(client.clone(), &ns, &ar);
    match api.create(&PostParams::default(), &obj).await {
        Ok(_) => Ok("created".into()),
        Err(kube::Error::Api(ae)) if ae.code == 409 => {
            api.replace(&name, &PostParams::default(), &obj)
                .await
                .map_err(|e| e.to_string())?;
            Ok("updated".into())
        }
        Err(e) => Err(e.to_string()),
    }
}

fn split_yaml_documents(yaml: &str) -> Vec<String> {
    yaml.split("\n---")
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(String::from)
        .collect()
}

fn parse_api_version(api_version: &str) -> (&str, &str) {
    if let Some((group, version)) = api_version.split_once('/') {
        (group, version)
    } else {
        ("", api_version)
    }
}

fn plural_for_kind(kind: &str) -> String {
    match kind {
        "VirtualMachine" => "virtualmachines".into(),
        "DataVolume" => "datavolumes".into(),
        "PersistentVolumeClaim" => "persistentvolumeclaims".into(),
        other => format!("{}s", other.to_lowercase()),
    }
}
