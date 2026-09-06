// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Ed25519 signatures for signed update manifests.

use anyhow::{Context, Result};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};

/// Canonical manifest bytes signed by Zeus release tooling.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct UpdateManifest {
    pub version: String,
    pub channel: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub linux_tar_sha256: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub windows_zip_sha256: String,
}

impl UpdateManifest {
    pub fn canonical_bytes(&self) -> Result<Vec<u8>> {
        serde_json::to_vec(self).context("serialize update manifest")
    }
}

/// Development-only signing key seed (override in production with env keys).
const DEV_SIGNING_SEED: [u8; 32] = [
    0x7a, 0x79, 0x76, 0x6f, 0x72, 0x2d, 0x67, 0x75, 0x65, 0x73, 0x74, 0x2d, 0x75, 0x70, 0x64, 0x61,
    0x74, 0x65, 0x2d, 0x64, 0x65, 0x76, 0x2d, 0x73, 0x65, 0x65, 0x64, 0x2d, 0x30, 0x30, 0x30, 0x31,
];

pub fn dev_signing_key() -> SigningKey {
    SigningKey::from_bytes(&DEV_SIGNING_SEED)
}

pub fn dev_verifying_key() -> VerifyingKey {
    dev_signing_key().verifying_key()
}

pub fn load_verifying_key() -> Result<VerifyingKey> {
    if let Ok(hex) = std::env::var("ZYVOR_UPDATE_PUBLIC_KEY_HEX") {
        if !hex.trim().is_empty() {
            return decode_verifying_key(hex.trim());
        }
    }
    Ok(dev_verifying_key())
}

pub fn load_signing_key() -> Result<SigningKey> {
    if let Ok(hex) = std::env::var("ZYVOR_UPDATE_SIGNING_KEY_HEX") {
        if !hex.trim().is_empty() {
            let bytes = hex::decode(hex.trim()).context("decode signing key hex")?;
            let bytes: [u8; 32] = bytes
                .try_into()
                .map_err(|_| anyhow::anyhow!("signing key must be 32 bytes"))?;
            return Ok(SigningKey::from_bytes(&bytes));
        }
    }
    Ok(dev_signing_key())
}

fn decode_verifying_key(hex: &str) -> Result<VerifyingKey> {
    let bytes = hex::decode(hex).context("decode public key hex")?;
    let bytes: [u8; 32] = bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("public key must be 32 bytes"))?;
    Ok(VerifyingKey::from_bytes(&bytes)?)
}

pub fn sign_manifest(manifest: &UpdateManifest) -> Result<String> {
    let key = load_signing_key()?;
    let bytes = manifest.canonical_bytes()?;
    let sig = key.sign(&bytes);
    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        sig.to_bytes(),
    ))
}

pub fn verify_manifest(manifest: &UpdateManifest, signature_b64: &str) -> Result<()> {
    let key = load_verifying_key()?;
    let bytes = manifest.canonical_bytes()?;
    let sig_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        signature_b64.trim(),
    )
    .context("decode manifest signature")?;
    let sig = Signature::from_slice(&sig_bytes).context("parse ed25519 signature")?;
    key.verify(&bytes, &sig)
        .map_err(|e| anyhow::anyhow!("manifest signature invalid: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_and_verify_linux_manifest() {
        let manifest = UpdateManifest {
            version: "0.2.0".into(),
            channel: "stable".into(),
            linux_tar_sha256: "abc123".into(),
            windows_zip_sha256: String::new(),
        };
        let sig = sign_manifest(&manifest).expect("sign");
        verify_manifest(&manifest, &sig).expect("verify");
    }

    #[test]
    fn sign_and_verify_windows_manifest() {
        let manifest = UpdateManifest {
            version: "0.2.0".into(),
            channel: "stable".into(),
            linux_tar_sha256: String::new(),
            windows_zip_sha256: "def456".into(),
        };
        let sig = sign_manifest(&manifest).expect("sign");
        verify_manifest(&manifest, &sig).expect("verify");
    }
}
