// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use axum::Form;
use axum::Json;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::Duration;
use uuid::Uuid;

use crate::auth::jwt::{issue_token, token_remaining_secs, verify_token};
use crate::auth::middleware::extract_bearer;
use crate::auth::oidc::{build_login_request, exchange_code, fetch_discovery};
use crate::auth::revoke::revoke_jti;
use crate::auth::saml::{build_login_request as build_saml_login, process_acs_response};
use crate::auth::store::load_settings;
use crate::auth::types::{AuthMeResponse, AuthUserClaims, PublicAuthConfig, ROLE_OPERATOR};
use crate::error::ApiError;
use crate::models::ApiResponse;
use crate::state::AppState;

const OIDC_STATE_TTL_SECS: u64 = 600;

#[derive(Debug, Deserialize)]
pub struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
    error_description: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct TokenQuery {
    token: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct SamlAcsForm {
    #[serde(rename = "SAMLResponse")]
    saml_response: String,
    #[serde(default, rename = "RelayState")]
    _relay_state: Option<String>,
}

pub async fn public_config(State(state): State<AppState>) -> Json<ApiResponse<PublicAuthConfig>> {
    let settings = load_settings(&state.pool).await.unwrap_or_default();
    Json(ApiResponse::ok(PublicAuthConfig {
        auth_enabled: state.config.auth_enabled,
        oidc_enabled: settings.oidc.enabled && !settings.oidc.issuer_url.is_empty(),
        saml_enabled: settings.saml.enabled && !settings.saml.sso_url.is_empty(),
        oidc_button_label: if settings.oidc.button_label.is_empty() {
            "Sign in with SSO".into()
        } else {
            settings.oidc.button_label.clone()
        },
        saml_button_label: if settings.saml.button_label.is_empty() {
            "Sign in with SAML".into()
        } else {
            settings.saml.button_label.clone()
        },
        allow_local_bypass: settings.identity.allow_local_bypass || !state.config.auth_enabled,
    }))
}

pub async fn me(
    State(state): State<AppState>,
    Query(query): Query<TokenQuery>,
    headers: axum::http::HeaderMap,
) -> Result<Json<ApiResponse<AuthMeResponse>>, ApiError> {
    let token = extract_bearer(&headers).or(query.token);
    if !state.config.auth_enabled {
        return Ok(Json(ApiResponse::ok(AuthMeResponse {
            authenticated: true,
            user: Some(AuthUserClaims {
                sub: "local".into(),
                email: Some("local@guestkit".into()),
                name: Some("Local Operator".into()),
                role: state.config.default_role().into(),
                provider: "local".into(),
                jti: None,
            }),
        })));
    }
    let Some(token) = token else {
        return Ok(Json(ApiResponse::ok(AuthMeResponse {
            authenticated: false,
            user: None,
        })));
    };
    match verify_token(&state.config, &token) {
        Ok(user) => Ok(Json(ApiResponse::ok(AuthMeResponse {
            authenticated: true,
            user: Some(user),
        }))),
        Err(_) => Err(ApiError::unauthorized("invalid or expired token")),
    }
}

#[derive(Debug, Serialize)]
pub struct LocalLoginResponse {
    pub authenticated: bool,
    pub token: String,
    pub user: AuthUserClaims,
}

pub async fn local_login(
    State(state): State<AppState>,
) -> Result<Json<ApiResponse<LocalLoginResponse>>, ApiError> {
    if !state.config.auth_enabled {
        return Err(ApiError::bad_request("authentication is disabled"));
    }
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !settings.identity.allow_local_bypass {
        return Err(ApiError::forbidden("local bypass is disabled"));
    }
    let user = AuthUserClaims {
        sub: "local".into(),
        email: Some("local@guestkit".into()),
        name: Some("Local Operator".into()),
        role: if settings.identity.default_role.is_empty() {
            ROLE_OPERATOR.into()
        } else {
            settings.identity.default_role.clone()
        },
        provider: "local".into(),
        jti: Some(Uuid::new_v4().to_string()),
    };
    let jwt = issue_token(&state.config, &user, settings.session_secs())
        .map_err(|e| ApiError::internal(format!("JWT issuance failed: {e}")))?;
    Ok(Json(ApiResponse::ok(LocalLoginResponse {
        authenticated: true,
        token: jwt,
        user,
    })))
}

pub async fn oidc_login(State(state): State<AppState>) -> Result<Response, ApiError> {
    if !state.config.auth_enabled {
        return Err(ApiError::bad_request("authentication is disabled"));
    }
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !settings.oidc.enabled {
        return Err(ApiError::bad_request("OIDC is not enabled"));
    }
    if settings.oidc.issuer_url.is_empty() || settings.oidc.client_id.is_empty() {
        return Err(ApiError::bad_request("OIDC issuer_url and client_id are required"));
    }

    let discovery = fetch_discovery(state.http_client().as_ref(), &settings.oidc.issuer_url)
        .await
        .map_err(|e| ApiError::internal(format!("OIDC discovery failed: {e}")))?;
    let login = build_login_request(&state.config, &settings.oidc, &discovery)
        .map_err(|e| ApiError::internal(e.to_string()))?;

    let redis_key = format!("oidc:state:{}", login.state);
    let payload = serde_json::json!({
        "code_verifier": login.code_verifier,
        "issuer_url": settings.oidc.issuer_url,
    });
    let mut redis = state.redis.clone();
    redis
        .set_ex::<_, _, ()>(redis_key, payload.to_string(), OIDC_STATE_TTL_SECS)
        .await
        .map_err(|e| ApiError::internal(format!("failed to store OIDC state: {e}")))?;

    Ok(Redirect::temporary(&login.authorize_url).into_response())
}

pub async fn oidc_callback(
    State(state): State<AppState>,
    Query(query): Query<CallbackQuery>,
) -> Result<Response, ApiError> {
    if let Some(err) = query.error {
        let msg = query.error_description.unwrap_or(err);
        let url = format!(
            "{}/login.html?error={}",
            state.config.ui_base_url.trim_end_matches('/'),
            urlencoding::encode(&msg)
        );
        return Ok(Redirect::temporary(&url).into_response());
    }

    let code = query
        .code
        .ok_or_else(|| ApiError::bad_request("missing authorization code"))?;
    let oidc_state = query
        .state
        .ok_or_else(|| ApiError::bad_request("missing state"))?;

    let redis_key = format!("oidc:state:{oidc_state}");
    let mut redis = state.redis.clone();
    let stored: Option<String> = redis
        .get(redis_key.clone())
        .await
        .map_err(|e| ApiError::internal(format!("redis error: {e}")))?;
    let Some(stored) = stored else {
        return Err(ApiError::bad_request("invalid or expired OIDC state"));
    };
    let _: () = redis
        .del(redis_key)
        .await
        .map_err(|e| ApiError::internal(format!("redis error: {e}")))?;

    let parsed: serde_json::Value =
        serde_json::from_str(&stored).map_err(|e| ApiError::internal(e.to_string()))?;
    let code_verifier = parsed["code_verifier"]
        .as_str()
        .ok_or_else(|| ApiError::internal("missing code_verifier in state"))?
        .to_string();
    let issuer_url = parsed["issuer_url"]
        .as_str()
        .ok_or_else(|| ApiError::internal("missing issuer_url in state"))?
        .to_string();

    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let discovery = fetch_discovery(state.http_client().as_ref(), &issuer_url)
        .await
        .map_err(|e| ApiError::internal(format!("OIDC discovery failed: {e}")))?;
    let mut user = match exchange_code(
        state.http_client().as_ref(),
        &state.config,
        &settings.oidc,
        &discovery,
        &settings.identity,
        &code,
        &code_verifier,
    )
    .await
    {
        Ok(user) => user,
        Err(e) => {
            let msg = format!("OIDC login rejected: {e}");
            let url = format!(
                "{}/login.html?error={}",
                state.config.ui_base_url.trim_end_matches('/'),
                urlencoding::encode(&msg)
            );
            return Ok(Redirect::temporary(&url).into_response());
        }
    };
    user.jti = Some(Uuid::new_v4().to_string());

    let jwt = issue_token(&state.config, &user, settings.session_secs())
        .map_err(|e| ApiError::internal(format!("JWT issuance failed: {e}")))?;
    let redirect = format!(
        "{}/login.html?token={}",
        state.config.ui_base_url.trim_end_matches('/'),
        urlencoding::encode(&jwt)
    );
    Ok(Redirect::temporary(&redirect).into_response())
}

pub async fn saml_login(State(state): State<AppState>) -> Result<Response, ApiError> {
    if !state.config.auth_enabled {
        return Err(ApiError::bad_request("authentication is disabled"));
    }
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    if !settings.saml.enabled {
        return Err(ApiError::bad_request("SAML is not enabled"));
    }
    let login = build_saml_login(&state.config, &settings.saml)
        .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(Redirect::temporary(&login.redirect_url).into_response())
}

pub async fn saml_acs(
    State(state): State<AppState>,
    Form(form): Form<SamlAcsForm>,
) -> Result<Response, ApiError> {
    let settings = load_settings(&state.pool)
        .await
        .map_err(|e| ApiError::internal(e.to_string()))?;
    let mut user = process_acs_response(
        &state.config,
        &settings.saml,
        &settings.identity,
        &form.saml_response,
    )
    .map_err(|e| ApiError::bad_request(format!("SAML ACS failed: {e}")))?;
    user.jti = Some(Uuid::new_v4().to_string());
    let jwt = issue_token(&state.config, &user, settings.session_secs())
        .map_err(|e| ApiError::internal(format!("JWT issuance failed: {e}")))?;
    let redirect = format!(
        "{}/login.html?token={}",
        state.config.ui_base_url.trim_end_matches('/'),
        urlencoding::encode(&jwt)
    );
    Ok(Redirect::temporary(&redirect).into_response())
}

pub async fn logout(
    State(state): State<AppState>,
    headers: axum::http::HeaderMap,
) -> Result<StatusCode, ApiError> {
    if !state.config.auth_enabled {
        return Ok(StatusCode::NO_CONTENT);
    }
    let Some(token) = extract_bearer(&headers) else {
        return Ok(StatusCode::NO_CONTENT);
    };
    if let Ok(user) = verify_token(&state.config, &token) {
        if let Some(jti) = user.jti {
            let ttl = token_remaining_secs(&state.config, &token).unwrap_or(3600);
            let mut redis = state.redis.clone();
            revoke_jti(&mut redis, &jti, ttl)
                .await
                .map_err(|e| ApiError::internal(e.to_string()))?;
        }
    }
    Ok(StatusCode::NO_CONTENT)
}

impl AppState {
    pub fn http_client(&self) -> Arc<reqwest::Client> {
        static CLIENT: std::sync::OnceLock<Arc<reqwest::Client>> = std::sync::OnceLock::new();
        Arc::clone(CLIENT.get_or_init(|| {
            Arc::new(
                reqwest::Client::builder()
                    .timeout(Duration::from_secs(20))
                    .build()
                    .expect("reqwest client"),
            )
        }))
    }
}
