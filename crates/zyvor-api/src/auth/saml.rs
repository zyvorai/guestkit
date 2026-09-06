// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

use anyhow::{anyhow, Context, Result};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use flate2::write::DeflateEncoder;
use flate2::Compression;
use openssl::hash::MessageDigest;
use openssl::sign::Verifier;
use openssl::x509::X509;
use quick_xml::events::Event;
use quick_xml::Reader;
use rand::RngCore;
use std::io::Write;
use url::Url;

use super::rbac::{email_from_claims, resolve_role};
use super::types::{AuthUserClaims, SamlSettings};
use crate::config::Config;

pub struct SamlLoginRequest {
    pub redirect_url: String,
}

pub struct SamlAssertionUser {
    pub sub: String,
    pub email: Option<String>,
    pub name: Option<String>,
    pub groups: Vec<String>,
}

pub fn build_login_request(config: &Config, saml: &SamlSettings) -> Result<SamlLoginRequest> {
    if saml.sso_url.trim().is_empty() {
        return Err(anyhow!("SAML SSO URL is not configured"));
    }
    let entity_id = entity_id(config, saml);
    let acs = acs_url(config);
    let mut id_bytes = [0u8; 16];
    rand::thread_rng().fill_bytes(&mut id_bytes);
    let request_id = format!("_{}", hex::encode(id_bytes));
    let instant = chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ");
    let xml = format!(
        r#"<samlp:AuthnRequest xmlns:samlp="urn:oasis:names:tc:SAML:2.0:protocol" xmlns:saml="urn:oasis:names:tc:SAML:2.0:assertion" ID="{request_id}" Version="2.0" IssueInstant="{instant}" Destination="{destination}" AssertionConsumerServiceURL="{acs}" ProtocolBinding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST"><saml:Issuer>{entity_id}</saml:Issuer><samlp:NameIDPolicy Format="urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress" AllowCreate="true"/></samlp:AuthnRequest>"#,
        request_id = xml_escape(&request_id),
        instant = instant,
        destination = xml_escape(saml.sso_url.trim()),
        acs = xml_escape(&acs),
        entity_id = xml_escape(&entity_id),
    );
    let mut encoder = DeflateEncoder::new(Vec::new(), Compression::default());
    encoder
        .write_all(xml.as_bytes())
        .context("deflate SAML AuthnRequest")?;
    let deflated = encoder.finish().context("finish deflate")?;
    let encoded = URL_SAFE_NO_PAD.encode(deflated);
    let mut url = Url::parse(saml.sso_url.trim()).context("invalid SAML SSO URL")?;
    url.query_pairs_mut()
        .append_pair("SAMLRequest", &encoded)
        .append_pair("RelayState", "guestkit");
    Ok(SamlLoginRequest {
        redirect_url: url.to_string(),
    })
}

pub fn process_acs_response(
    _config: &Config,
    saml: &SamlSettings,
    identity: &super::types::IdentitySettings,
    saml_response_b64: &str,
) -> Result<AuthUserClaims> {
    let decoded = base64::engine::general_purpose::STANDARD
        .decode(saml_response_b64.trim())
        .or_else(|_| {
            base64::engine::general_purpose::STANDARD
                .decode(saml_response_b64.trim().replace('\n', ""))
        })
        .context("invalid SAMLResponse base64")?;
    let xml = String::from_utf8(decoded).context("SAMLResponse is not UTF-8")?;
    if !status_success(&xml) {
        return Err(anyhow!("SAML response status is not Success"));
    }
    if saml.certificate_pem.trim().is_empty() {
        return Err(anyhow!("IdP certificate PEM is required for SAML login"));
    }
    verify_signature(&xml, &saml.certificate_pem)?;
    let user = parse_assertion_user(&xml)?;
    let role = resolve_role(
        identity,
        user.email.as_deref(),
        user.name.as_deref(),
        &user.groups,
    );
    Ok(AuthUserClaims {
        sub: user.sub.clone(),
        email: email_from_claims(user.email.as_deref(), user.name.as_deref(), &user.sub),
        name: user.name,
        role,
        provider: "saml".into(),
        jti: None,
    })
}

fn entity_id(config: &Config, saml: &SamlSettings) -> String {
    if saml.entity_id.is_empty() {
        format!(
            "{}/api/v1/settings/sso/saml/metadata",
            config.public_base_url.trim_end_matches('/')
        )
    } else {
        saml.entity_id.clone()
    }
}

pub fn acs_url(config: &Config) -> String {
    format!(
        "{}/api/v1/auth/saml/acs",
        config.public_base_url.trim_end_matches('/')
    )
}

pub fn sp_metadata_xml(config: &Config, saml: &SamlSettings) -> String {
    let entity_id = entity_id(config, saml);
    let acs = acs_url(config);
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<EntityDescriptor xmlns="urn:oasis:names:tc:SAML:2.0:metadata" entityID="{entity_id}">
  <SPSSODescriptor AuthnRequestsSigned="false" WantAssertionsSigned="true" protocolSupportEnumeration="urn:oasis:names:tc:SAML:2.0:protocol">
    <NameIDFormat>{name_id_format}</NameIDFormat>
    <AssertionConsumerService Binding="urn:oasis:names:tc:SAML:2.0:bindings:HTTP-POST" Location="{acs}" index="1"/>
  </SPSSODescriptor>
</EntityDescriptor>"#,
        entity_id = xml_escape(&entity_id),
        name_id_format = xml_escape(if saml.name_id_format.is_empty() {
            "urn:oasis:names:tc:SAML:1.1:nameid-format:emailAddress"
        } else {
            &saml.name_id_format
        }),
        acs = xml_escape(&acs),
    )
}

fn status_success(xml: &str) -> bool {
    xml.contains("StatusCode Value=\"urn:oasis:names:tc:SAML:2.0:status:Success\"")
        || xml.contains("StatusCode Value='urn:oasis:names:tc:SAML:2.0:status:Success'")
}

fn parse_assertion_user(xml: &str) -> Result<SamlAssertionUser> {
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);
    let mut buf = Vec::new();
    let mut in_name_id = false;
    let mut in_attr_value = false;
    let mut current_attr = String::new();
    let mut sub = String::new();
    let mut email = None;
    let mut name = None;
    let mut groups = Vec::new();
    let mut not_on_or_after_valid = true;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                let local = event_local_name_start(&e);
                if local.ends_with("NameID") {
                    in_name_id = true;
                } else if local.ends_with("Attribute") {
                    current_attr = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref().ends_with(b"Name"))
                        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
                        .unwrap_or_default()
                        .to_lowercase();
                } else if local.ends_with("AttributeValue") {
                    in_attr_value = true;
                } else if local.ends_with("Conditions") {
                    if let Some(not_on) = e
                        .attributes()
                        .flatten()
                        .find(|a| a.key.as_ref().ends_with(b"NotOnOrAfter"))
                        .and_then(|a| String::from_utf8(a.value.into_owned()).ok())
                    {
                        if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(&not_on) {
                            not_on_or_after_valid = ts > chrono::Utc::now();
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                let text = t.unescape().unwrap_or_default().into_owned();
                if in_name_id && sub.is_empty() {
                    sub = text.trim().to_string();
                } else if in_attr_value {
                    let val = text.trim().to_string();
                    if current_attr.contains("mail") || current_attr.contains("email") {
                        email.get_or_insert(val);
                    } else if current_attr.contains("name") || current_attr.contains("displayname") {
                        name.get_or_insert(val);
                    } else if current_attr.contains("group") || current_attr.contains("role") {
                        groups.push(val);
                    }
                }
            }
            Ok(Event::End(e)) => {
                let local = event_local_name(&e);
                if local.ends_with("NameID") {
                    in_name_id = false;
                } else if local.ends_with("AttributeValue") {
                    in_attr_value = false;
                } else if local.ends_with("Attribute") {
                    current_attr.clear();
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => return Err(anyhow!("SAML XML parse error: {e}")),
            _ => {}
        }
        buf.clear();
    }

    if !not_on_or_after_valid {
        return Err(anyhow!("SAML assertion expired"));
    }
    if sub.is_empty() {
        return Err(anyhow!("SAML assertion missing NameID"));
    }
    Ok(SamlAssertionUser {
        sub,
        email,
        name,
        groups,
    })
}

fn verify_signature(xml: &str, cert_pem: &str) -> Result<()> {
    let sig_value = extract_between(xml, "<ds:SignatureValue", "</ds:SignatureValue>")
        .or_else(|| extract_between(xml, "<SignatureValue", "</SignatureValue>"))
        .ok_or_else(|| anyhow!("SAML response missing SignatureValue"))?;
    let sig_bytes = base64::engine::general_purpose::STANDARD
        .decode(sig_value.replace(['\n', ' ', '\t'], ""))
        .context("invalid SignatureValue base64")?;
    let signed_info = extract_between(xml, "<ds:SignedInfo", "</ds:SignedInfo>")
        .or_else(|| extract_between(xml, "<SignedInfo", "</SignedInfo>"))
        .ok_or_else(|| anyhow!("SAML response missing SignedInfo"))?;
    let canonical = format!("<SignedInfo>{signed_info}</SignedInfo>");
    let cert = X509::from_pem(cert_pem.as_bytes()).context("invalid IdP certificate PEM")?;
    let pkey = cert.public_key().context("IdP certificate public key")?;
    let mut verifier = Verifier::new(MessageDigest::sha256(), &pkey)?;
    verifier
        .update(canonical.as_bytes())
        .context("hash SignedInfo")?;
    if verifier.verify(&sig_bytes).unwrap_or(false) {
        return Ok(());
    }
    Err(anyhow!("SAML signature verification failed"))
}

fn extract_between(haystack: &str, start_tag: &str, end_tag: &str) -> Option<String> {
    let start_idx = haystack.find(start_tag)?;
    let after_start = &haystack[start_idx..];
    let content_start = after_start.find('>')? + 1;
    let content_region = &after_start[content_start..];
    let end_idx = content_region.find(end_tag)?;
    Some(content_region[..end_idx].trim().to_string())
}

fn event_local_name_start(e: &quick_xml::events::BytesStart) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

fn event_local_name(e: &quick_xml::events::BytesEnd) -> String {
    String::from_utf8_lossy(e.local_name().as_ref()).into_owned()
}

fn xml_escape(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

// hex helper without adding dep - use simple format
mod hex {
    pub fn encode(bytes: [u8; 16]) -> String {
        bytes.iter().map(|b| format!("{b:02x}")).collect()
    }
}
