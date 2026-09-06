// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Persisted per-VM golden baselines for `fleet watch` — scheduled drift
//! monitoring against a fixed known-good `EvidenceSnapshot`, rather than
//! against whatever the previous scan happened to see.
//!
//! One JSON file per VM (keyed by canonicalized image path), under
//! `dirs::cache_dir()/guestkit/fleet-baseline` (override with
//! `GUESTKIT_FLEET_BASELINE_DIR`). Deliberately does *not* roll forward on
//! every run the way `cli/cache.rs`'s evidence cache or `ai/memory.rs`'s
//! cross-run memory do: a drift monitor that quietly re-baselines to
//! whatever it just saw would never report the very drift it exists to
//! catch. The baseline only changes when a caller explicitly establishes it
//! (first run for that VM) or resets it (`fleet watch --reset-baseline`,
//! e.g. after a reviewed and accepted change).

use crate::evidence::snapshot::EvidenceSnapshot;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::{Path, PathBuf};

const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FleetBaseline {
    schema_version: u32,
    pub hostname: String,
    pub captured_at: String,
    pub evidence: EvidenceSnapshot,
}

fn baseline_root() -> Result<PathBuf> {
    let root = if let Ok(dir) = std::env::var("GUESTKIT_FLEET_BASELINE_DIR") {
        PathBuf::from(dir)
    } else {
        dirs::cache_dir()
            .context("could not determine cache directory")?
            .join("guestkit")
            .join("fleet-baseline")
    };
    fs::create_dir_all(&root)?;
    Ok(root)
}

/// Stable key for a VM's baseline file — canonicalized image path, hashed.
fn image_key(image_path: &Path) -> String {
    let canonical = image_path
        .canonicalize()
        .unwrap_or_else(|_| image_path.to_path_buf());
    let mut hasher = Sha256::new();
    hasher.update(canonical.to_string_lossy().as_bytes());
    format!("{:x}", hasher.finalize())
}

fn baseline_path(image_path: &Path) -> Result<PathBuf> {
    Ok(baseline_root()?.join(format!("{}-v{SCHEMA_VERSION}.json", image_key(image_path))))
}

/// Load a VM's golden baseline, if one has been established.
pub fn load(image_path: &Path) -> Option<FleetBaseline> {
    let path = baseline_path(image_path).ok()?;
    let data = fs::read_to_string(&path).ok()?;
    match serde_json::from_str::<FleetBaseline>(&data) {
        Ok(b) if b.schema_version == SCHEMA_VERSION => Some(b),
        _ => None, // stale schema or corrupt file — treat as "no baseline yet"
    }
}

/// Establish or reset a VM's golden baseline to the given evidence,
/// written atomically (`.partial` then rename).
pub fn store(image_path: &Path, hostname: &str, evidence: &EvidenceSnapshot) -> Result<()> {
    let path = baseline_path(image_path)?;
    let baseline = FleetBaseline {
        schema_version: SCHEMA_VERSION,
        hostname: hostname.to_string(),
        captured_at: chrono::Utc::now().to_rfc3339(),
        evidence: evidence.clone(),
    };
    let json = serde_json::to_string_pretty(&baseline)?;
    let partial = path.with_extension("json.partial");
    fs::write(&partial, json)
        .with_context(|| format!("write fleet baseline to {}", partial.display()))?;
    fs::rename(&partial, &path)
        .with_context(|| format!("finalize fleet baseline at {}", path.display()))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::snapshot::{
        BootEvidence, OsEvidence, PackageEvidence, SecurityEvidence, StorageEvidence,
        VmToolsEvidence,
    };
    use std::sync::Mutex;

    // GUESTKIT_FLEET_BASELINE_DIR is a process-global env var — serialize
    // tests that touch it so they don't race.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    fn evidence(distribution: &str) -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: crate::evidence::snapshot::SCHEMA_VERSION,
            image_path: "/tmp/vm1.qcow2".into(),
            collected_at: "now".into(),
            root: "/".into(),
            os: OsEvidence {
                distribution: distribution.into(),
                version: "1.0".into(),
                hostname: "vm1".into(),
                ..Default::default()
            },
            boot: BootEvidence::default(),
            storage: StorageEvidence::default(),
            network: Default::default(),
            packages: PackageEvidence::default(),
            security: SecurityEvidence::default(),
            vm_tools: VmToolsEvidence::default(),
            systemd: None,
            windows: None,
            kubevirt: None,
            cloud_init: None,
            network_probes: None,
            snapshot_readiness: None,
            process: None,
            hardware: None,
            linux_migration: None,
            online_cache: None,
        }
    }

    #[test]
    fn no_baseline_until_stored() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_FLEET_BASELINE_DIR", dir.path());

        let image = dir.path().join("disk.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        assert!(load(&image).is_none());

        std::env::remove_var("GUESTKIT_FLEET_BASELINE_DIR");
    }

    #[test]
    fn store_then_load_round_trips() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_FLEET_BASELINE_DIR", dir.path());

        let image = dir.path().join("disk2.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        store(&image, "vm1", &evidence("ubuntu")).unwrap();
        let baseline = load(&image).expect("baseline should persist");
        assert_eq!(baseline.hostname, "vm1");
        assert_eq!(baseline.evidence.os.distribution, "ubuntu");

        std::env::remove_var("GUESTKIT_FLEET_BASELINE_DIR");
    }

    #[test]
    fn store_overwrites_prior_baseline() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("GUESTKIT_FLEET_BASELINE_DIR", dir.path());

        let image = dir.path().join("disk3.qcow2");
        std::fs::write(&image, b"fake").unwrap();

        store(&image, "vm1", &evidence("ubuntu")).unwrap();
        store(&image, "vm1", &evidence("rhel")).unwrap();
        let baseline = load(&image).unwrap();
        assert_eq!(baseline.evidence.os.distribution, "rhel");

        std::env::remove_var("GUESTKIT_FLEET_BASELINE_DIR");
    }
}
