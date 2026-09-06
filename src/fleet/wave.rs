// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dependency-aware migration wave ordering.
//!
//! `analyze_fleet` (see `analyzer.rs`) scores and clusters VMs independently
//! — it has no notion of "migrate this DB before that app server". This
//! module builds an explicit dependency graph from evidence already present
//! in every `EvidenceSnapshot` (no new collectors, no extra per-VM mount)
//! and topologically sorts it into migration waves.
//!
//! Two signals, both real and offline (grounded in evidence, not guessed):
//!
//! - **Database role**: a VM is tagged `Role::Database` if any of its
//!   systemd units (already collected, `EvidenceSnapshot.systemd`) name a
//!   known DB service (postgresql, mysql/mariadb, mongod, redis,
//!   cassandra). This is the same "unit name contains X" heuristic
//!   `cli/migrate/planner.rs` and `cli/blueprint/compose.rs` already use
//!   for a different purpose (Dockerization / compose sidecar generation).
//! - **Storage dependency**: if a VM's `/etc/fstab` (already collected,
//!   `EvidenceSnapshot.storage.fstab_entries`) mounts an NFS export whose
//!   server hostname matches another VM's hostname *in the same fleet
//!   scan*, the mounting VM depends on that VM.
//!
//! Deliberately **not** attempted: parsing app config files for DB
//! connection strings to infer "app X talks to db Y" — no existing
//! offline collector records the target host from a connection string
//! (only `cli/commands/security.rs` flags them as leaked secrets, without
//! extracting the host), and guessing at this without a real signal would
//! produce dependency edges that look authoritative but aren't. Database
//! VMs are instead given priority via role alone (see [`plan_waves`]).

use crate::evidence::snapshot::EvidenceSnapshot;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, VecDeque};

const DB_UNIT_HINTS: &[&str] = &[
    "postgresql",
    "mysql",
    "mariadb",
    "mongod",
    "redis",
    "cassandra",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VmRole {
    /// Runs a systemd unit matching a known database service name.
    Database,
    Standard,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WaveMember {
    pub image: String,
    pub hostname: String,
    pub role: VmRole,
    /// Images this VM depends on (must migrate no later than them) —
    /// currently populated only from NFS-mount hostname matches.
    pub depends_on: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MigrationWave {
    pub wave: usize,
    pub images: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WavePlan {
    pub members: Vec<WaveMember>,
    pub waves: Vec<MigrationWave>,
    /// Images that couldn't be placed in any wave because they're part of a
    /// dependency cycle (A depends on B depends on A). Surfaced, not
    /// silently dropped or arbitrarily broken — a cycle usually means the
    /// storage-dependency heuristic matched something it shouldn't have
    /// (e.g. two VMs both NFS-mounting each other for unrelated reasons).
    pub cycles: Vec<Vec<String>>,
}

fn detect_role(ev: &EvidenceSnapshot) -> VmRole {
    let Some(systemd) = &ev.systemd else {
        return VmRole::Standard;
    };
    let is_db = systemd.units.iter().any(|u| {
        let lname = u.name.to_lowercase();
        DB_UNIT_HINTS.iter().any(|hint| lname.contains(hint))
    });
    if is_db {
        VmRole::Database
    } else {
        VmRole::Standard
    }
}

fn detect_storage_deps(
    image: &str,
    ev: &EvidenceSnapshot,
    hostname_to_image: &HashMap<String, String>,
) -> Vec<String> {
    let mut deps = Vec::new();
    for entry in &ev.storage.fstab_entries {
        if !entry.fstype.eq_ignore_ascii_case("nfs") && !entry.fstype.eq_ignore_ascii_case("nfs4") {
            continue;
        }
        let Some((server, _export)) = entry.device.split_once(':') else {
            continue;
        };
        if let Some(dep_image) = hostname_to_image.get(server) {
            if dep_image != image && !deps.contains(dep_image) {
                deps.push(dep_image.clone());
            }
        }
    }
    deps
}

/// Build a dependency-aware migration wave plan from the same
/// `(image, evidence, boot_score)` triples `analyze_fleet` consumes.
///
/// Waves are topological-sort levels (Kahn's algorithm): everything with no
/// unmet dependency lands in wave 0, then wave 1 once wave 0's dependencies
/// are satisfied, and so on — VMs within a wave have no ordering
/// constraint between them and can migrate in parallel. Within a wave,
/// members are ordered database-role-first, then by descending boot score
/// (healthiest/easiest first — start a wave's cutover slot with the VMs
/// least likely to need last-minute triage).
pub fn plan_waves(snapshots: &[(String, EvidenceSnapshot, f64)]) -> WavePlan {
    let hostname_to_image: HashMap<String, String> = snapshots
        .iter()
        .filter(|(_, ev, _)| !ev.os.hostname.is_empty())
        .map(|(image, ev, _)| (ev.os.hostname.clone(), image.clone()))
        .collect();

    let mut members: Vec<WaveMember> = Vec::with_capacity(snapshots.len());
    let mut scores: HashMap<String, f64> = HashMap::new();
    for (image, ev, score) in snapshots {
        members.push(WaveMember {
            image: image.clone(),
            hostname: ev.os.hostname.clone(),
            role: detect_role(ev),
            depends_on: detect_storage_deps(image, ev, &hostname_to_image),
        });
        scores.insert(image.clone(), *score);
    }

    let waves = topo_waves(&members, &scores);
    let placed: std::collections::HashSet<&str> = waves
        .iter()
        .flat_map(|w| w.images.iter().map(String::as_str))
        .collect();
    let cycles = find_cycles(&members, &placed);

    WavePlan {
        members,
        waves,
        cycles,
    }
}

fn topo_waves(members: &[WaveMember], scores: &HashMap<String, f64>) -> Vec<MigrationWave> {
    let mut in_degree: HashMap<&str, usize> = members
        .iter()
        .map(|m| (m.image.as_str(), m.depends_on.len()))
        .collect();
    // dependents[X] = images that depend on X, so we can decrement their
    // in_degree once X is placed.
    let mut dependents: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in members {
        for dep in &m.depends_on {
            dependents
                .entry(dep.as_str())
                .or_default()
                .push(m.image.as_str());
        }
    }

    let mut waves = Vec::new();
    let mut frontier: Vec<&str> = in_degree
        .iter()
        .filter(|(_, deg)| **deg == 0)
        .map(|(img, _)| *img)
        .collect();

    let by_image: HashMap<&str, &WaveMember> =
        members.iter().map(|m| (m.image.as_str(), m)).collect();

    let mut wave_num = 0usize;
    while !frontier.is_empty() {
        frontier.sort_by(|a, b| {
            let ma = by_image[a];
            let mb = by_image[b];
            match (ma.role, mb.role) {
                (VmRole::Database, VmRole::Standard) => std::cmp::Ordering::Less,
                (VmRole::Standard, VmRole::Database) => std::cmp::Ordering::Greater,
                _ => scores
                    .get(*b)
                    .partial_cmp(&scores.get(*a))
                    .unwrap_or(std::cmp::Ordering::Equal),
            }
        });

        waves.push(MigrationWave {
            wave: wave_num,
            images: frontier.iter().map(|s| s.to_string()).collect(),
        });

        let mut next_frontier = VecDeque::new();
        for &img in &frontier {
            if let Some(deps) = dependents.get(img) {
                for &dependent in deps {
                    if let Some(deg) = in_degree.get_mut(dependent) {
                        *deg -= 1;
                        if *deg == 0 {
                            next_frontier.push_back(dependent);
                        }
                    }
                }
            }
        }
        frontier = next_frontier.into_iter().collect();
        wave_num += 1;
    }

    waves
}

/// Any image not in `placed` has unresolved (cyclic) dependencies. Group
/// them by weakly-connected component so a report can show which VMs are
/// stuck together, rather than one flat unordered list.
fn find_cycles(
    members: &[WaveMember],
    placed: &std::collections::HashSet<&str>,
) -> Vec<Vec<String>> {
    let stuck: Vec<&WaveMember> = members
        .iter()
        .filter(|m| !placed.contains(m.image.as_str()))
        .collect();
    if stuck.is_empty() {
        return Vec::new();
    }

    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();
    for m in &stuck {
        for dep in &m.depends_on {
            if placed.contains(dep.as_str()) {
                continue;
            }
            adjacency
                .entry(m.image.as_str())
                .or_default()
                .push(dep.as_str());
            adjacency
                .entry(dep.as_str())
                .or_default()
                .push(m.image.as_str());
        }
    }

    let mut visited: std::collections::HashSet<&str> = std::collections::HashSet::new();
    let mut components = Vec::new();
    for m in &stuck {
        if visited.contains(m.image.as_str()) {
            continue;
        }
        let mut component = Vec::new();
        let mut queue = VecDeque::from([m.image.as_str()]);
        while let Some(node) = queue.pop_front() {
            if !visited.insert(node) {
                continue;
            }
            component.push(node.to_string());
            if let Some(neighbors) = adjacency.get(node) {
                for &n in neighbors {
                    if !visited.contains(n) {
                        queue.push_back(n);
                    }
                }
            }
        }
        component.sort();
        components.push(component);
    }
    components.sort();
    components
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::evidence::snapshot::{
        BootEvidence, EvidenceSnapshot, FstabEntry, OsEvidence, PackageEvidence, SecurityEvidence,
        StorageEvidence, VmToolsEvidence,
    };

    fn evidence(hostname: &str, fstab: Vec<FstabEntry>) -> EvidenceSnapshot {
        EvidenceSnapshot {
            schema_version: crate::evidence::snapshot::SCHEMA_VERSION,
            image_path: format!("/tmp/{hostname}.qcow2"),
            collected_at: "now".into(),
            root: "/".into(),
            os: OsEvidence {
                hostname: hostname.to_string(),
                ..Default::default()
            },
            storage: StorageEvidence {
                fstab_entries: fstab,
                ..Default::default()
            },
            boot: BootEvidence::default(),
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

    fn nfs_entry(server: &str) -> FstabEntry {
        FstabEntry {
            device: format!("{server}:/export/data"),
            mountpoint: "/mnt/data".into(),
            fstype: "nfs4".into(),
            options: "defaults".into(),
        }
    }

    #[test]
    fn independent_vms_all_land_in_wave_zero() {
        let snapshots = vec![
            ("a.qcow2".to_string(), evidence("a", vec![]), 90.0),
            ("b.qcow2".to_string(), evidence("b", vec![]), 80.0),
        ];
        let plan = plan_waves(&snapshots);
        assert_eq!(plan.waves.len(), 1);
        assert_eq!(plan.waves[0].images.len(), 2);
        assert!(plan.cycles.is_empty());
    }

    #[test]
    fn storage_dependency_orders_into_separate_waves() {
        // app.qcow2 NFS-mounts from "storage" -> depends on storage.qcow2
        let snapshots = vec![
            (
                "app.qcow2".to_string(),
                evidence("app", vec![nfs_entry("storage")]),
                85.0,
            ),
            (
                "storage.qcow2".to_string(),
                evidence("storage", vec![]),
                95.0,
            ),
        ];
        let plan = plan_waves(&snapshots);
        assert_eq!(plan.waves.len(), 2);
        assert_eq!(plan.waves[0].images, vec!["storage.qcow2"]);
        assert_eq!(plan.waves[1].images, vec!["app.qcow2"]);
        assert!(plan.cycles.is_empty());
    }

    #[test]
    fn cyclic_dependency_is_reported_not_dropped() {
        let snapshots = vec![
            (
                "a.qcow2".to_string(),
                evidence("a", vec![nfs_entry("b")]),
                90.0,
            ),
            (
                "b.qcow2".to_string(),
                evidence("b", vec![nfs_entry("a")]),
                90.0,
            ),
        ];
        let plan = plan_waves(&snapshots);
        assert!(plan.waves.iter().all(|w| w.images.is_empty()) || plan.waves.is_empty());
        assert_eq!(plan.cycles.len(), 1);
        assert_eq!(plan.cycles[0], vec!["a.qcow2", "b.qcow2"]);
    }
}
