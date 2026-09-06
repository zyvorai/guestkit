// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! In-cluster guest-agent CA and client certificate issuance.

use rcgen::{
    BasicConstraints, Certificate, CertificateParams, DistinguishedName, DnType, IsCa, KeyPair,
    PKCS_ECDSA_P256_SHA256,
};
use std::fs;
use std::path::PathBuf;

use crate::error::{ApiError, ApiResult};

const CA_CERT_FILE: &str = "ca.pem";
const CA_KEY_FILE: &str = "ca-key.pem";
const SERVER_CERT_FILE: &str = "zeus-api.pem";
const SERVER_KEY_FILE: &str = "zeus-api-key.pem";
const DEFAULT_VALIDITY_DAYS: i64 = 90;

pub struct AgentCa {
    dir: PathBuf,
}

impl AgentCa {
    pub fn from_config(dir: PathBuf) -> Self {
        Self { dir }
    }

    pub fn is_ready(&self) -> bool {
        self.ca_cert_path().exists() && self.ca_key_path().exists()
    }

    pub fn ca_cert_path(&self) -> PathBuf {
        self.dir.join(CA_CERT_FILE)
    }

    pub fn ca_key_path(&self) -> PathBuf {
        self.dir.join(CA_KEY_FILE)
    }

    pub fn server_cert_path(&self) -> PathBuf {
        self.dir.join(SERVER_CERT_FILE)
    }

    pub fn server_key_path(&self) -> PathBuf {
        self.dir.join(SERVER_KEY_FILE)
    }

    pub fn server_sans_from_env() -> Vec<String> {
        std::env::var("AGENT_MTLS_SERVER_SANS")
            .ok()
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim())
                    .filter(|s| !s.is_empty())
                    .map(String::from)
                    .collect::<Vec<String>>()
            })
            .filter(|sans: &Vec<String>| !sans.is_empty())
            .unwrap_or_else(|| {
                vec![
                    "localhost".into(),
                    "zyvor-api".into(),
                    "api.zyvor.local".into(),
                ]
            })
    }

    pub fn ensure_server_tls(&self) -> ApiResult<()> {
        if self.server_cert_path().exists() && self.server_key_path().exists() {
            return Ok(());
        }

        let (ca_key, ca_cert) = self.ensure_ca()?;
        let sans = Self::server_sans_from_env();
        let common_name = sans
            .first()
            .cloned()
            .unwrap_or_else(|| "zyvor-api".into());

        let server_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| ApiError::internal(format!("generate server key: {e}")))?;
        let mut params = CertificateParams::new(sans)
            .map_err(|e| ApiError::internal(e.to_string()))?;
        params.distinguished_name = DistinguishedName::new();
        params
            .distinguished_name
            .push(DnType::CommonName, &common_name);
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Zyvor");

        let server_cert = params
            .signed_by(&server_key, &ca_cert, &ca_key)
            .map_err(|e| ApiError::internal(format!("sign server cert: {e}")))?;

        fs::write(self.server_cert_path(), server_cert.pem())
            .map_err(|e| ApiError::internal(e.to_string()))?;
        fs::write(self.server_key_path(), server_key.serialize_pem())
            .map_err(|e| ApiError::internal(e.to_string()))?;

        Ok(())
    }

    fn load_ca(&self) -> ApiResult<(KeyPair, Certificate)> {
        let ca_pem = fs::read_to_string(self.ca_cert_path())
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let ca_key_pem = fs::read_to_string(self.ca_key_path())
            .map_err(|e| ApiError::internal(e.to_string()))?;
        let ca_key = KeyPair::from_pem(&ca_key_pem)
            .map_err(|e| ApiError::internal(format!("parse CA key: {e}")))?;
        let ca_params = CertificateParams::from_ca_cert_pem(&ca_pem)
            .map_err(|e| ApiError::internal(format!("parse CA cert: {e}")))?;
        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| ApiError::internal(format!("load CA issuer: {e}")))?;
        Ok((ca_key, ca_cert))
    }

    pub fn ensure_ca(&self) -> ApiResult<(KeyPair, Certificate)> {
        if self.is_ready() {
            return self.load_ca();
        }

        fs::create_dir_all(&self.dir).map_err(|e| ApiError::internal(e.to_string()))?;

        let ca_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| ApiError::internal(format!("generate CA key: {e}")))?;
        let mut ca_params = CertificateParams::new(vec!["Zyvor Guest Agent CA".into()])
            .map_err(|e| ApiError::internal(e.to_string()))?;
        ca_params.is_ca = IsCa::Ca(BasicConstraints::Unconstrained);
        ca_params.distinguished_name = DistinguishedName::new();
        ca_params
            .distinguished_name
            .push(DnType::OrganizationName, "Zyvor");
        ca_params
            .distinguished_name
            .push(DnType::CommonName, "Zyvor Guest Agent CA");

        let ca_cert = ca_params
            .self_signed(&ca_key)
            .map_err(|e| ApiError::internal(format!("sign CA cert: {e}")))?;

        fs::write(self.ca_cert_path(), ca_cert.pem())
            .map_err(|e| ApiError::internal(e.to_string()))?;
        fs::write(self.ca_key_path(), ca_key.serialize_pem())
            .map_err(|e| ApiError::internal(e.to_string()))?;

        Ok((ca_key, ca_cert))
    }

    pub fn ca_pem(&self) -> ApiResult<String> {
        if self.is_ready() {
            fs::read_to_string(self.ca_cert_path()).map_err(|e| ApiError::internal(e.to_string()))
        } else {
            let (_, ca_cert) = self.ensure_ca()?;
            Ok(ca_cert.pem())
        }
    }

    pub fn issue_client_cert(&self, hostname: &str) -> ApiResult<IssuedClientCert> {
        let (ca_key, ca_cert) = self.ensure_ca()?;
        let ca_pem = fs::read_to_string(self.ca_cert_path())
            .map_err(|e| ApiError::internal(e.to_string()))?;

        let client_key = KeyPair::generate_for(&PKCS_ECDSA_P256_SHA256)
            .map_err(|e| ApiError::internal(format!("generate client key: {e}")))?;
        let mut params = CertificateParams::new(vec![hostname.into()])
            .map_err(|e| ApiError::internal(e.to_string()))?;
        params.distinguished_name = DistinguishedName::new();
        params.distinguished_name.push(DnType::CommonName, hostname);
        params
            .distinguished_name
            .push(DnType::OrganizationName, "Zyvor Guest Agent");

        let client_cert = params
            .signed_by(&client_key, &ca_cert, &ca_key)
            .map_err(|e| ApiError::internal(format!("sign client cert: {e}")))?;

        Ok(IssuedClientCert {
            cert_pem: client_cert.pem(),
            key_pem: client_key.serialize_pem(),
            ca_pem,
            expires_at: chrono::Utc::now()
                .checked_add_signed(chrono::Duration::days(DEFAULT_VALIDITY_DAYS))
                .map(|t| t.to_rfc3339())
                .unwrap_or_default(),
        })
    }
}

pub struct IssuedClientCert {
    pub cert_pem: String,
    pub key_pem: String,
    pub ca_pem: String,
    pub expires_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn ensure_ca_and_issue_client_cert() {
        let dir = TempDir::new().expect("tempdir");
        let ca = AgentCa::from_config(dir.path().to_path_buf());
        assert!(!ca.is_ready());

        let issued = ca
            .issue_client_cert("test-vm.example")
            .map_err(|e| e.message)
            .expect("issue");
        assert!(issued.cert_pem.contains("BEGIN CERTIFICATE"));
        assert!(issued.key_pem.contains("BEGIN PRIVATE KEY"));
        assert!(ca.is_ready());
        ca.ensure_server_tls().map_err(|e| e.message).expect("server tls");
        assert!(ca.server_cert_path().exists());
        assert!(ca.server_key_path().exists());
    }
}
