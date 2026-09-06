// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Operator CLI for signed agent update manifests (`--features agent`).

use anyhow::Result;
use std::path::Path;

#[cfg(feature = "agent")]
use anyhow::Context;

#[cfg(feature = "agent")]
pub fn keygen(seed: &Path, public: &Path) -> Result<()> {
    use crate::agent::update_sign::{dev_signing_key, load_signing_key};
    let key = if std::env::var("ZYVOR_UPDATE_SIGNING_KEY_HEX").is_ok() {
        load_signing_key()?
    } else {
        let _ = dev_signing_key();
        load_signing_key()?
    };
    let seed_bytes = key.to_bytes();
    let pub_bytes = key.verifying_key().to_bytes();
    std::fs::write(seed, hex::encode(seed_bytes))?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(seed, std::fs::Permissions::from_mode(0o600));
    }
    std::fs::write(public, format!("{}\n", hex::encode(pub_bytes)))?;
    println!("wrote seed {} pubkey {}", seed.display(), public.display());
    Ok(())
}

#[cfg(feature = "agent")]
pub fn sign(manifest: &Path, output: &Path) -> Result<()> {
    use crate::agent::update_sign::{sign_manifest, UpdateManifest};
    let raw = std::fs::read_to_string(manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let m: UpdateManifest = serde_json::from_str(&raw).context("parse UpdateManifest")?;
    let sig = sign_manifest(&m)?;
    std::fs::write(output, format!("{sig}\n"))?;
    println!("signed {} → {}", manifest.display(), output.display());
    Ok(())
}

#[cfg(feature = "agent")]
pub fn verify(manifest: &Path, signature: &Path) -> Result<()> {
    use crate::agent::update_sign::{verify_manifest, UpdateManifest};
    let raw = std::fs::read_to_string(manifest)
        .with_context(|| format!("read {}", manifest.display()))?;
    let m: UpdateManifest = serde_json::from_str(&raw).context("parse UpdateManifest")?;
    let sig = std::fs::read_to_string(signature)
        .with_context(|| format!("read {}", signature.display()))?;
    verify_manifest(&m, sig.trim())?;
    println!("ok {}", manifest.display());
    Ok(())
}

#[cfg(not(feature = "agent"))]
pub fn keygen(_seed: &Path, _public: &Path) -> Result<()> {
    anyhow::bail!("rebuild with --features agent")
}
#[cfg(not(feature = "agent"))]
pub fn sign(_manifest: &Path, _output: &Path) -> Result<()> {
    anyhow::bail!("rebuild with --features agent")
}
#[cfg(not(feature = "agent"))]
pub fn verify(_manifest: &Path, _signature: &Path) -> Result<()> {
    anyhow::bail!("rebuild with --features agent")
}
