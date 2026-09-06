// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use jsonwebtoken::{decode, decode_header, Algorithm, DecodingKey, Validation};
use rand::RngCore;
use reqwest::Client;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::Url;

use super::rbac::{groups_from_json, resolve_role};
use super::types::{AuthUserClaims, IdentitySettings, OidcDiscovery, OidcSettings};
use crate::config::Config;

#[derive(Debug, Deserialize)]
struct RawDiscovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    #[serde(default)]
    userinfo_endpoint: Option<String>,
    #[serde(default)]
    jwks_uri: Option<String>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    id_token: Option<String>,
    #[serde(default)]
    token_type: Option<String>,
}

#[derive(Debug, Deserialize)]
struct JwksResponse {
    keys: Vec<JwkKey>,
}

#[derive(Debug, Deserialize)]
struct JwkKey {
    kid: Option<String>,
    kty: String,
    #[serde(default)]
    n: Option<String>,
    #[serde(default)]
    e: Option<String>,
    #[serde(default)]
    x5c: Option<Vec<String>>,
}

#[derive(Debug, Deserialize)]
struct IdTokenClaims {
    sub: String,
    #[serde(default)]
    email: Option<String>,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    preferred_username: Option<String>,
    #[serde(flatten)]
    extra: serde_json::Map<String, serde_json::Value>,
}

pub struct OidcLoginRequest {
    pub state: String,
    pub code_verifier: String,
    pub authorize_url: String,
}

pub fn build_login_request(
    config: &Config,
    oidc: &OidcSettings,
    discovery: &OidcDiscovery,
) -> Result<OidcLoginRequest> {
    let mut state_bytes = [0u8; 24];
    rand::thread_rng().fill_bytes(&mut state_bytes);
    let state = URL_SAFE_NO_PAD.encode(state_bytes);

    let mut verifier_bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut verifier_bytes);
    let code_verifier = URL_SAFE_NO_PAD.encode(verifier_bytes);
    let challenge = URL_SAFE_NO_PAD.encode(Sha256::digest(code_verifier.as_bytes()));

    let redirect_uri = config.oidc_redirect_uri();
    let mut url = Url::parse(&discovery.authorization_endpoint)
        .context("invalid authorization_endpoint from discovery")?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("response_type", "code");
        pairs.append_pair("client_id", &oidc.client_id);
        pairs.append_pair("redirect_uri", &redirect_uri);
        pairs.append_pair("scope", &oidc.scopes.join(" "));
        pairs.append_pair("state", &state);
        pairs.append_pair("code_challenge", &challenge);
        pairs.append_pair("code_challenge_method", "S256");
    }

    Ok(OidcLoginRequest {
        state,
        code_verifier,
        authorize_url: url.to_string(),
    })
}

pub async fn fetch_discovery(client: &Client, issuer_url: &str) -> Result<OidcDiscovery> {
    let issuer = issuer_url.trim_end_matches('/');
    let url = format!("{issuer}/.well-known/openid-configuration");
    let raw: RawDiscovery = client
        .get(&url)
        .send()
        .await
        .context("OIDC discovery request failed")?
        .error_for_status()
        .context("OIDC discovery returned error status")?
        .json()
        .await
        .context("OIDC discovery JSON parse failed")?;

    Ok(OidcDiscovery {
        issuer: raw.issuer,
        authorization_endpoint: raw.authorization_endpoint,
        token_endpoint: raw.token_endpoint,
        userinfo_endpoint: raw.userinfo_endpoint,
        jwks_uri: raw.jwks_uri,
    })
}

pub async fn exchange_code(
    client: &Client,
    config: &Config,
    oidc: &OidcSettings,
    discovery: &OidcDiscovery,
    identity: &IdentitySettings,
    code: &str,
    code_verifier: &str,
) -> Result<AuthUserClaims> {
    let redirect_uri = config.oidc_redirect_uri();
    let mut form = vec![
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", redirect_uri.as_str()),
        ("client_id", oidc.client_id.as_str()),
        ("code_verifier", code_verifier),
    ];
    let client_secret = oidc.client_secret.as_deref().unwrap_or("");
    if !client_secret.is_empty() {
        form.push(("client_secret", client_secret));
    }

    let token: TokenResponse = client
        .post(&discovery.token_endpoint)
        .form(&form)
        .send()
        .await
        .context("OIDC token exchange failed")?
        .error_for_status()
        .context("OIDC token endpoint error")?
        .json()
        .await
        .context("OIDC token JSON parse failed")?;

    if let Some(endpoint) = &discovery.userinfo_endpoint {
        let info: serde_json::Value = client
            .get(endpoint)
            .bearer_auth(&token.access_token)
            .send()
            .await
            .context("OIDC userinfo request failed")?
            .error_for_status()
            .context("OIDC userinfo error")?
            .json()
            .await
            .context("OIDC userinfo JSON parse failed")?;

        let sub = info
            .get("sub")
            .or_else(|| info.get("preferred_username"))
            .and_then(|v| v.as_str())
            .map(String::from)
            .ok_or_else(|| anyhow!("OIDC userinfo missing sub"))?;
        let email = info
            .get("email")
            .and_then(|v| v.as_str())
            .map(String::from);
        let name = info
            .get("name")
            .or_else(|| info.get("preferred_username"))
            .and_then(|v| v.as_str())
            .map(String::from);
        let groups = groups_from_json(&info, &identity.role_claim);
        let role = resolve_role(identity, email.as_deref(), name.as_deref(), &groups);
        return Ok(AuthUserClaims {
            sub,
            email,
            name,
            role,
            provider: "oidc".into(),
            jti: None,
        });
    }

    if let Some(id_token) = token.id_token {
        if oidc.client_id.is_empty() {
            return Err(anyhow!("OIDC client_id is required to verify id_token"));
        }
        return claims_from_id_token(
            client,
            discovery,
            identity,
            &oidc.client_id,
            &id_token,
        )
        .await;
    }

    Err(anyhow!(
        "OIDC provider did not expose userinfo_endpoint or id_token"
    ))
}

async fn claims_from_id_token(
    client: &Client,
    discovery: &OidcDiscovery,
    identity: &IdentitySettings,
    client_id: &str,
    id_token: &str,
) -> Result<AuthUserClaims> {
    let jwks_uri = discovery
        .jwks_uri
        .as_deref()
        .ok_or_else(|| anyhow!("OIDC discovery missing jwks_uri; cannot verify id_token"))?;

    let header = decode_header(id_token).context("invalid id_token header")?;
    let decoding_key = jwks_decoding_key(client, jwks_uri, header.kid.as_deref()).await?;
    let claims = decode_verified_id_token(
        id_token,
        &decoding_key,
        &discovery.issuer,
        client_id,
    )?;

    let email = claims.email.clone();
    let name = claims
        .name
        .clone()
        .or(claims.preferred_username.clone());
    let groups = groups_from_json(
        &serde_json::Value::Object(claims.extra),
        &identity.role_claim,
    );
    let role = resolve_role(
        identity,
        email.as_deref(),
        name.as_deref(),
        &groups,
    );
    Ok(AuthUserClaims {
        sub: claims.sub,
        email,
        name,
        role,
        provider: "oidc".into(),
        jti: None,
    })
}

fn id_token_validation(issuer: &str, client_id: &str) -> Validation {
    let mut validation = Validation::new(Algorithm::RS256);
    validation.set_issuer(&[issuer]);
    validation.set_audience(&[client_id]);
    validation.validate_aud = true;
    validation
}

fn decode_verified_id_token(
    id_token: &str,
    decoding_key: &DecodingKey,
    issuer: &str,
    client_id: &str,
) -> Result<IdTokenClaims> {
    let validation = id_token_validation(issuer, client_id);
    decode::<IdTokenClaims>(id_token, decoding_key, &validation)
        .map(|data| data.claims)
        .context("id_token signature or claim validation failed")
}

async fn jwks_decoding_key(
    client: &Client,
    jwks_uri: &str,
    kid: Option<&str>,
) -> Result<DecodingKey> {
    let jwks: JwksResponse = client
        .get(jwks_uri)
        .send()
        .await
        .context("JWKS fetch failed")?
        .error_for_status()
        .context("JWKS error status")?
        .json()
        .await
        .context("JWKS JSON parse failed")?;

    let key = jwks
        .keys
        .into_iter()
        .find(|k| kid.map(|id| k.kid.as_deref() == Some(id)).unwrap_or(true))
        .ok_or_else(|| anyhow!("matching JWKS key not found"))?;

    if let Some(chain) = key.x5c.and_then(|c| c.into_iter().next()) {
        let der = base64::engine::general_purpose::STANDARD
            .decode(chain)
            .context("x5c decode")?;
        return Ok(DecodingKey::from_rsa_der(&der));
    }

    let n = key.n.ok_or_else(|| anyhow!("JWKS key missing n"))?;
    let e = key.e.ok_or_else(|| anyhow!("JWKS key missing e"))?;
    if key.kty != "RSA" {
        return Err(anyhow!("unsupported JWK kty {}", key.kty));
    }
    DecodingKey::from_rsa_components(&n, &e).context("JWKS RSA components")
}

#[cfg(test)]
mod tests {
    use super::*;
    use jsonwebtoken::{encode, EncodingKey, Header};
    use openssl::rsa::Rsa;
    use serde::Serialize;

    const TEST_ISSUER: &str = "https://issuer.example";
    const TEST_CLIENT_ID: &str = "zyvor-api-client";

    #[derive(Debug, Serialize)]
    struct TestIdTokenClaims {
        sub: String,
        iss: String,
        aud: String,
        exp: i64,
    }

    fn test_rsa_keys() -> (EncodingKey, DecodingKey) {
        let rsa = Rsa::generate(2048).expect("rsa");
        let priv_pem = rsa.private_key_to_pem().expect("private pem");
        let pub_pem = rsa.public_key_to_pem_pkcs1().expect("public pem");
        let encoding = EncodingKey::from_rsa_pem(&priv_pem).expect("encoding key");
        let decoding = DecodingKey::from_rsa_pem(&pub_pem).expect("decoding key");
        (encoding, decoding)
    }

    fn mint_id_token(encoding: &EncodingKey, aud: &str) -> String {
        let claims = TestIdTokenClaims {
            sub: "user-123".into(),
            iss: TEST_ISSUER.into(),
            aud: aud.into(),
            exp: chrono::Utc::now().timestamp() + 3600,
        };
        encode(&Header::new(Algorithm::RS256), &claims, encoding).expect("token")
    }

    #[test]
    fn id_token_validation_accepts_matching_audience() {
        let (encoding, decoding) = test_rsa_keys();
        let token = mint_id_token(&encoding, TEST_CLIENT_ID);
        let claims = decode_verified_id_token(&token, &decoding, TEST_ISSUER, TEST_CLIENT_ID)
            .expect("valid token");
        assert_eq!(claims.sub, "user-123");
    }

    #[test]
    fn id_token_validation_rejects_wrong_audience() {
        let (encoding, decoding) = test_rsa_keys();
        let token = mint_id_token(&encoding, "wrong-client");
        let err = decode_verified_id_token(&token, &decoding, TEST_ISSUER, TEST_CLIENT_ID)
            .unwrap_err()
            .to_string();
        assert!(err.contains("id_token signature or claim validation failed"));
    }

    #[test]
    fn id_token_validation_rejects_invalid_signature() {
        let (encoding_a, _) = test_rsa_keys();
        let (_, decoding_b) = test_rsa_keys();
        let token = mint_id_token(&encoding_a, TEST_CLIENT_ID);
        assert!(decode_verified_id_token(&token, &decoding_b, TEST_ISSUER, TEST_CLIENT_ID).is_err());
    }

    #[test]
    fn id_token_validation_rejects_wrong_issuer() {
        let (encoding, decoding) = test_rsa_keys();
        let token = mint_id_token(&encoding, TEST_CLIENT_ID);
        assert!(
            decode_verified_id_token(&token, &decoding, "https://evil.example", TEST_CLIENT_ID)
                .is_err()
        );
    }
}
