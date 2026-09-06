// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::State;
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::middleware::require_admin;
use crate::auth::saml::sp_metadata_xml;
use crate::auth::store::{load_settings, save_settings};
use crate::auth::types::{AuthUserClaims, IdentitySettings, OidcSettings, SamlSettings};
use crate::error::ApiError;
use crate::models::ApiResponse;
use crate::state::AppState;

#[derive(Debug, Serialize, Deserialize)]
pub struct IdentitySettingsView {
    pub allow_local_bypass: bool,
    pub default_role: String,
    pub session_hours: u32,
    pub role_claim: String,
    pub admin_roles: Vec<String>,
    pub admin_emails: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct OidcSettingsView {
    pub enabled: bool,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret_set: bool,
    pub scopes: Vec<String>,
    pub button_label: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SamlSettingsView {
    pub enabled: bool,
    pub entity_id: String,
    pub sso_url: String,
    pub metadata_url: String,
    pub certificate_pem: String,
    pub name_id_format: String,
    pub button_label: String,
}

#[derive(Debug, Serialize)]
pub struct SsoSettingsResponse {
    pub oidc: OidcSettingsView,
    pub saml: SamlSettingsView,
}

#[derive(Debug, Deserialize)]
pub struct PutIdentityRequest {
    pub allow_local_bypass: bool,
    pub default_role: String,
    pub session_hours: u32,
    pub role_claim: String,
    pub admin_roles: Vec<String>,
    pub admin_emails: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct PutSsoRequest {
    pub oidc: OidcSettingsInput,
    pub saml: SamlSettingsInput,
}

#[derive(Debug, Deserialize)]
pub struct OidcSettingsInput {
    pub enabled: bool,
    pub issuer_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub button_label: String,
}

#[derive(Debug, Deserialize)]
pub struct SamlSettingsInput {
    pub enabled: bool,
    pub entity_id: String,
    pub sso_url: String,
    pub metadata_url: String,
    pub certificate_pem: String,
    pub name_id_format: String,
    pub button_label: String,
}

pub async fn get_identity(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
) -> Result<Json<ApiResponse<IdentitySettingsView>>, ApiError> {
    ensure_settings_admin(&state, user.as_ref().map(|Extension(u)| u))?;
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(to_identity_view(&settings.identity))))
}

pub async fn put_identity(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
    Json(body): Json<PutIdentityRequest>,
) -> Result<Json<ApiResponse<IdentitySettingsView>>, ApiError> {
    ensure_settings_admin(&state, user.as_ref().map(|Extension(u)| u))?;
    let mut settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    settings.identity = IdentitySettings {
        allow_local_bypass: body.allow_local_bypass,
        default_role: body.default_role,
        session_hours: body.session_hours.max(1),
        role_claim: if body.role_claim.trim().is_empty() {
            "groups".into()
        } else {
            body.role_claim
        },
        admin_roles: body.admin_roles,
        admin_emails: body.admin_emails,
    };
    save_settings(&state.pool, &settings)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(to_identity_view(&settings.identity))))
}

pub async fn get_sso(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
) -> Result<Json<ApiResponse<SsoSettingsResponse>>, ApiError> {
    ensure_settings_admin(&state, user.as_ref().map(|Extension(u)| u))?;
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(SsoSettingsResponse {
        oidc: to_oidc_view(&settings.oidc),
        saml: to_saml_view(&settings.saml),
    })))
}

pub async fn put_sso(
    State(state): State<AppState>,
    user: Option<Extension<AuthUserClaims>>,
    Json(body): Json<PutSsoRequest>,
) -> Result<Json<ApiResponse<SsoSettingsResponse>>, ApiError> {
    ensure_settings_admin(&state, user.as_ref().map(|Extension(u)| u))?;
    let mut settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    settings.oidc = OidcSettings {
        enabled: body.oidc.enabled,
        issuer_url: body.oidc.issuer_url.trim().to_string(),
        client_id: body.oidc.client_id.trim().to_string(),
        client_secret: merge_secret(body.oidc.client_secret, settings.oidc.client_secret),
        scopes: if body.oidc.scopes.is_empty() {
            settings.oidc.scopes.clone()
        } else {
            body.oidc.scopes
        },
        button_label: body.oidc.button_label,
    };
    settings.saml = SamlSettings {
        enabled: body.saml.enabled,
        entity_id: body.saml.entity_id,
        sso_url: body.saml.sso_url,
        metadata_url: body.saml.metadata_url,
        certificate_pem: body.saml.certificate_pem,
        name_id_format: body.saml.name_id_format,
        button_label: body.saml.button_label,
    };
    save_settings(&state.pool, &settings)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Json(ApiResponse::ok(SsoSettingsResponse {
        oidc: to_oidc_view(&settings.oidc),
        saml: to_saml_view(&settings.saml),
    })))
}

pub async fn saml_metadata(State(state): State<AppState>) -> Result<Response, ApiError> {
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let xml = sp_metadata_xml(&state.config, &settings.saml);
    Ok((
        [(header::CONTENT_TYPE, "application/samlmetadata+xml")],
        xml,
    )
        .into_response())
}

fn to_identity_view(v: &IdentitySettings) -> IdentitySettingsView {
    IdentitySettingsView {
        allow_local_bypass: v.allow_local_bypass,
        default_role: v.default_role.clone(),
        session_hours: v.session_hours,
        role_claim: v.role_claim.clone(),
        admin_roles: v.admin_roles.clone(),
        admin_emails: v.admin_emails.clone(),
    }
}

fn to_oidc_view(v: &OidcSettings) -> OidcSettingsView {
    OidcSettingsView {
        enabled: v.enabled,
        issuer_url: v.issuer_url.clone(),
        client_id: v.client_id.clone(),
        client_secret_set: v
            .client_secret
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty()),
        scopes: v.scopes.clone(),
        button_label: v.button_label.clone(),
    }
}

fn to_saml_view(v: &SamlSettings) -> SamlSettingsView {
    SamlSettingsView {
        enabled: v.enabled,
        entity_id: v.entity_id.clone(),
        sso_url: v.sso_url.clone(),
        metadata_url: v.metadata_url.clone(),
        certificate_pem: v.certificate_pem.clone(),
        name_id_format: v.name_id_format.clone(),
        button_label: v.button_label.clone(),
    }
}

fn merge_secret(incoming: Option<String>, existing: Option<String>) -> Option<String> {
    match incoming {
        Some(value) if value.trim().is_empty() => existing,
        Some(value) => Some(value),
        None => existing,
    }
}

fn ensure_settings_admin(
    state: &AppState,
    user: Option<&AuthUserClaims>,
) -> Result<(), ApiError> {
    if !state.config.auth_enabled {
        return Ok(());
    }
    let user = user.ok_or_else(|| ApiError {
        status: StatusCode::UNAUTHORIZED,
        error: "UNAUTHORIZED".into(),
        message: "Bearer token required".into(),
    })?;
    require_admin(user)
}
