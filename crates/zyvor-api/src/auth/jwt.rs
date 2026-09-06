// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use jsonwebtoken::{decode, encode, Algorithm, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use super::types::AuthUserClaims;
use crate::config::Config;

#[derive(Debug, Serialize, Deserialize)]
struct JwtClaims {
    sub: String,
    email: Option<String>,
    name: Option<String>,
    role: String,
    provider: String,
    iss: String,
    exp: usize,
    iat: usize,
    jti: String,
}

pub fn issue_token(
    config: &Config,
    user: &AuthUserClaims,
    expiry_secs: usize,
) -> Result<String, jsonwebtoken::errors::Error> {
    let now = chrono::Utc::now().timestamp() as usize;
    let exp = now + expiry_secs.max(60);
    let jti = user
        .jti
        .clone()
        .unwrap_or_else(|| Uuid::new_v4().to_string());
    let claims = JwtClaims {
        sub: user.sub.clone(),
        email: user.email.clone(),
        name: user.name.clone(),
        role: user.role.clone(),
        provider: user.provider.clone(),
        iss: config.jwt_issuer.clone(),
        exp,
        iat: now,
        jti,
    };
    encode(
        &Header::new(Algorithm::HS256),
        &claims,
        &EncodingKey::from_secret(config.jwt_secret.as_bytes()),
    )
}

pub fn verify_token(config: &Config, token: &str) -> Result<AuthUserClaims, jsonwebtoken::errors::Error> {
    let mut validation = Validation::new(Algorithm::HS256);
    validation.set_issuer(&[config.jwt_issuer.as_str()]);
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )?;
    Ok(AuthUserClaims {
        sub: data.claims.sub,
        email: data.claims.email,
        name: data.claims.name,
        role: data.claims.role,
        provider: data.claims.provider,
        jti: Some(data.claims.jti),
    })
}

pub fn token_remaining_secs(config: &Config, token: &str) -> Option<u64> {
    let verified = verify_token(config, token).ok()?;
    let mut validation = Validation::new(Algorithm::HS256);
    validation.validate_exp = false;
    validation.set_issuer(&[config.jwt_issuer.as_str()]);
    let data = decode::<JwtClaims>(
        token,
        &DecodingKey::from_secret(config.jwt_secret.as_bytes()),
        &validation,
    )
    .ok()?;
    let now = chrono::Utc::now().timestamp();
    let remaining = data.claims.exp as i64 - now;
    if remaining <= 0 {
        return Some(0);
    }
    let _ = verified;
    Some(remaining as u64)
}
