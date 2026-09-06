// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use serde::{Deserialize, Serialize};

pub const ROLE_ADMIN: &str = "admin";
pub const ROLE_OPERATOR: &str = "operator";
pub const ROLE_VIEWER: &str = "viewer";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IdentitySettings {
    #[serde(default = "default_true")]
    pub allow_local_bypass: bool,
    pub default_role: String,
    pub session_hours: u32,
    #[serde(default = "default_role_claim")]
    pub role_claim: String,
    #[serde(default)]
    pub admin_roles: Vec<String>,
    #[serde(default)]
    pub admin_emails: Vec<String>,
}

fn default_true() -> bool {
    true
}

fn default_role_claim() -> String {
    "groups".into()
}

impl IdentitySettings {
    pub fn defaults() -> Self {
        Self {
            allow_local_bypass: false,
            default_role: ROLE_OPERATOR.into(),
            session_hours: 24,
            role_claim: default_role_claim(),
            admin_roles: vec!["admin".into(), "guestkit-admin".into()],
            admin_emails: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OidcSettings {
    pub enabled: bool,
    pub issuer_url: String,
    pub client_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_secret: Option<String>,
    #[serde(default = "default_oidc_scopes")]
    pub scopes: Vec<String>,
    #[serde(default)]
    pub button_label: String,
}

fn default_oidc_scopes() -> Vec<String> {
    vec!["openid".into(), "profile".into(), "email".into()]
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SamlSettings {
    pub enabled: bool,
    pub entity_id: String,
    pub sso_url: String,
    pub metadata_url: String,
    pub certificate_pem: String,
    #[serde(default)]
    pub name_id_format: String,
    #[serde(default = "default_saml_button_label")]
    pub button_label: String,
}

fn default_saml_button_label() -> String {
    "Sign in with SAML".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AuthSettings {
    #[serde(default)]
    pub identity: IdentitySettings,
    #[serde(default)]
    pub oidc: OidcSettings,
    #[serde(default)]
    pub saml: SamlSettings,
}

impl AuthSettings {
    pub fn defaults() -> Self {
        Self {
            identity: IdentitySettings::defaults(),
            oidc: OidcSettings::default(),
            saml: SamlSettings::default(),
        }
    }

    pub fn session_secs(&self) -> usize {
        (self.identity.session_hours.max(1) as usize) * 3600
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PublicAuthConfig {
    pub auth_enabled: bool,
    pub oidc_enabled: bool,
    pub saml_enabled: bool,
    pub oidc_button_label: String,
    pub saml_button_label: String,
    pub allow_local_bypass: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthUserClaims {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub role: String,
    pub provider: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub jti: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AuthMeResponse {
    pub authenticated: bool,
    pub user: Option<AuthUserClaims>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OidcDiscovery {
    pub issuer: String,
    pub authorization_endpoint: String,
    pub token_endpoint: String,
    pub userinfo_endpoint: Option<String>,
    pub jwks_uri: Option<String>,
}
