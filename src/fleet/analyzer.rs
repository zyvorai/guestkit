// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Fleet clustering analyzer.

use super::report::{FleetAnalysisReport, MigrationBlocker, SnowflakeVm, VmCluster};
use crate::evidence::EvidenceSnapshot;
use std::collections::HashMap;

#[derive(Debug, Clone)]
struct VmFingerprint {
    image: String,
    os_key: String,
    pkg_hash: String,
    boot_score: f64,
}

pub fn analyze_fleet(snapshots: &[(String, EvidenceSnapshot, f64)]) -> FleetAnalysisReport {
    let fingerprints: Vec<VmFingerprint> = snapshots
        .iter()
        .map(|(path, ev, score)| VmFingerprint {
            image: path.clone(),
            os_key: format!("{}:{}", ev.os.distribution, ev.os.version),
            pkg_hash: fingerprint_packages(ev),
            boot_score: *score,
        })
        .collect();

    let mut cluster_map: HashMap<String, Vec<String>> = HashMap::new();
    for fp in &fingerprints {
        let key = format!("{}:{}", fp.os_key, fp.pkg_hash);
        cluster_map.entry(key).or_default().push(fp.image.clone());
    }

    let mut clusters: Vec<VmCluster> = cluster_map
        .into_iter()
        .enumerate()
        .map(|(id, (key, members))| {
            let parts: Vec<&str> = key.splitn(2, ':').collect();
            VmCluster {
                id,
                count: members.len(),
                label: if members.len() > 1 {
                    format!("Cluster {} ({} nodes)", id + 1, members.len())
                } else {
                    "Unique".to_string()
                },
                members: members.clone(),
                os: parts.first().unwrap_or(&"").to_string(),
                kernel: snapshots
                    .iter()
                    .find(|(p, _, _)| p == &members[0])
                    .and_then(|(_, ev, _)| ev.packages.kernels.first().cloned())
                    .unwrap_or_default(),
            }
        })
        .collect();

    clusters.sort_by_key(|b| std::cmp::Reverse(b.count));

    let snowflakes: Vec<SnowflakeVm> = fingerprints
        .iter()
        .filter(|fp| {
            clusters
                .iter()
                .find(|c| c.members.contains(&fp.image))
                .map(|c| c.count == 1)
                .unwrap_or(true)
        })
        .map(|fp| SnowflakeVm {
            image: fp.image.clone(),
            reason: "No matching cluster — unique configuration".to_string(),
            similarity: 0.0,
        })
        .collect();

    let migration_blockers: Vec<MigrationBlocker> = fingerprints
        .iter()
        .filter(|fp| fp.boot_score < 60.0)
        .map(|fp| MigrationBlocker {
            image: fp.image.clone(),
            issue: format!("Low boot assurance score: {:.0}%", fp.boot_score),
            boot_score: fp.boot_score,
        })
        .collect();

    let golden_image_candidates: Vec<String> = clusters
        .iter()
        .filter(|c| c.count >= 3)
        .map(|c| format!("{} ({} identical nodes)", c.members[0], c.count))
        .collect();

    FleetAnalysisReport {
        total_vms: snapshots.len(),
        clusters,
        snowflakes,
        migration_blockers,
        golden_image_candidates,
        failed_vms: Vec::new(),
    }
}

fn fingerprint_packages(ev: &EvidenceSnapshot) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(ev.packages.count.to_le_bytes());
    for pkg in ev.packages.sample_packages.iter().take(20) {
        hasher.update(pkg.as_bytes());
    }
    format!("{:x}", hasher.finalize())[..12].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::snapshot::{
        BootEvidence, EvidenceSnapshot, OsEvidence, PackageEvidence, SecurityEvidence,
        StorageEvidence, VmToolsEvidence, SCHEMA_VERSION,
    };

    fn sample_evidence(distro: &str, pkg: &str) -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: SCHEMA_VERSION,
            image_path: "test.qcow2".to_string(),
            collected_at: "2026-01-01T00:00:00Z".to_string(),
            root: "/".to_string(),
            os: OsEvidence {
                distribution: distro.to_string(),
                version: "22.04".to_string(),
                hostname: "host".to_string(),
                ..Default::default()
            },
            storage: StorageEvidence::default(),
            boot: BootEvidence::default(),
            packages: PackageEvidence {
                count: 1,
                sample_packages: vec![pkg.to_string()],
                ..Default::default()
            },
            security: SecurityEvidence::default(),
            network: Default::default(),
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
    fn clusters_identical_images() {
        let ev = sample_evidence("ubuntu", "nginx");
        let snapshots = vec![
            ("a.qcow2".to_string(), ev.clone(), 90.0),
            ("b.qcow2".to_string(), ev, 88.0),
        ];
        let report = analyze_fleet(&snapshots);
        assert_eq!(report.total_vms, 2);
        assert_eq!(report.clusters.len(), 1);
        assert_eq!(report.clusters[0].count, 2);
    }
}
