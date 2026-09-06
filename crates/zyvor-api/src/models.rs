// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct VmImage {
    pub id: Uuid,
    pub tenant: String,
    pub name: String,
    pub object_key: String,
    pub format: String,
    pub size_bytes: i64,
    pub checksum: Option<String>,
    pub status: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct JobRecord {
    pub id: Uuid,
    pub vm_id: Uuid,
    pub operation: String,
    pub status: String,
    pub worker_id: Option<String>,
    pub submitted_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub success: bool,
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(data: T) -> Self {
        Self {
            success: true,
            data,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct VmImportResponse {
    pub id: Uuid,
    pub name: String,
    pub format: String,
    pub size_bytes: i64,
    pub path: String,
}

#[derive(Debug, Serialize)]
pub struct JobEnqueueResponse {
    pub job_id: Uuid,
    pub operation: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ProvisionResponse {
    pub vm_id: Uuid,
    pub yaml: String,
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resources: Option<Vec<crate::kubevirt_apply::AppliedResource>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub apply_errors: Option<Vec<String>>,
}
