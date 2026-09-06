// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::Result;
use sqlx::PgPool;

use super::types::AuthSettings;

const SETTINGS_ID: i32 = 1;

pub async fn load_settings(pool: &PgPool) -> Result<AuthSettings> {
    let row: Option<(serde_json::Value,)> =
        sqlx::query_as("SELECT settings FROM auth_settings WHERE id = $1")
            .bind(SETTINGS_ID)
            .fetch_optional(pool)
            .await?;

    match row {
        Some((value,)) => Ok(serde_json::from_value(value).unwrap_or_default()),
        None => Ok(AuthSettings::defaults()),
    }
}

pub async fn save_settings(pool: &PgPool, settings: &AuthSettings) -> Result<()> {
    let value = serde_json::to_value(settings)?;
    sqlx::query(
        r#"INSERT INTO auth_settings (id, settings, updated_at)
           VALUES ($1, $2, NOW())
           ON CONFLICT (id) DO UPDATE SET settings = EXCLUDED.settings, updated_at = NOW()"#,
    )
    .bind(SETTINGS_ID)
    .bind(value)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn bootstrap_from_env(pool: &PgPool, config: &crate::config::Config) -> Result<()> {
    let mut settings = load_settings(pool).await?;
    let mut changed = false;

    if let Ok(issuer) = std::env::var("OIDC_ISSUER_URL") {
        if !issuer.trim().is_empty() && settings.oidc.issuer_url.is_empty() {
            settings.oidc.issuer_url = issuer;
            changed = true;
        }
    }
    if let Ok(client_id) = std::env::var("OIDC_CLIENT_ID") {
        if !client_id.trim().is_empty() && settings.oidc.client_id.is_empty() {
            settings.oidc.client_id = client_id;
            changed = true;
        }
    }
    if let Ok(secret) = std::env::var("OIDC_CLIENT_SECRET") {
        if !secret.trim().is_empty()
            && settings
                .oidc
                .client_secret
                .as_ref()
                .is_none_or(|s| s.is_empty())
        {
            settings.oidc.client_secret = Some(secret);
            changed = true;
        }
    }
    if config.auth_enabled && !settings.oidc.issuer_url.is_empty() && !settings.oidc.enabled {
        settings.oidc.enabled = true;
        changed = true;
    }
    if let Ok(label) = std::env::var("OIDC_BUTTON_LABEL") {
        if !label.trim().is_empty() {
            settings.oidc.button_label = label;
            changed = true;
        }
    }
    if let Ok(entity) = std::env::var("SAML_ENTITY_ID") {
        if !entity.trim().is_empty() && settings.saml.entity_id.is_empty() {
            settings.saml.entity_id = entity;
            changed = true;
        }
    }
    if let Ok(sso_url) = std::env::var("SAML_SSO_URL") {
        if !sso_url.trim().is_empty() && settings.saml.sso_url.is_empty() {
            settings.saml.sso_url = sso_url;
            changed = true;
        }
    }
    if let Ok(meta) = std::env::var("SAML_METADATA_URL") {
        if !meta.trim().is_empty() && settings.saml.metadata_url.is_empty() {
            settings.saml.metadata_url = meta;
            changed = true;
        }
    }
    if let Ok(cert) = std::env::var("SAML_CERTIFICATE_PEM") {
        if !cert.trim().is_empty() && settings.saml.certificate_pem.is_empty() {
            settings.saml.certificate_pem = cert;
            changed = true;
        }
    }
    if std::env::var("SAML_ENABLED")
        .ok()
        .map(|v| matches!(v.to_lowercase().as_str(), "1" | "true" | "yes" | "on"))
        .unwrap_or(false)
    {
        settings.saml.enabled = true;
        changed = true;
    }

    if changed {
        save_settings(pool, &settings).await?;
    }
    Ok(())
}
