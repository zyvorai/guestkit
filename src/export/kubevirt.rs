// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! KubeVirt VirtualMachine and DataVolume manifest generation.

use crate::assurance::MigrationPlanResult;
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Disk metadata for KubeVirt provisioning.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskMetadata {
    pub name: String,
    pub format: String,
    pub size_bytes: Option<u64>,
    pub storage_class: String,
    /// HTTP(S) or S3 URL for CDI import (optional).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub import_url: Option<String>,
    pub namespace: String,
}

impl DiskMetadata {
    pub fn from_image_path(image: &Path, namespace: &str, storage_class: &str) -> Self {
        let name = image
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("migrated-vm")
            .to_string();
        let format = image
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("qcow2")
            .to_lowercase();
        let size_bytes = std::fs::metadata(image).ok().map(|m| m.len());

        Self {
            name: sanitize_k8s_name(&name),
            format,
            size_bytes,
            storage_class: storage_class.to_string(),
            import_url: None,
            namespace: namespace.to_string(),
        }
    }
}

/// Generated KubeVirt manifests.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KubeVirtManifests {
    pub data_volume: serde_yaml::Value,
    pub virtual_machine: serde_yaml::Value,
}

/// Generate KubeVirt DataVolume + VirtualMachine YAML documents.
pub fn generate_kubevirt_manifests(
    plan: &MigrationPlanResult,
    disk: &DiskMetadata,
) -> Result<KubeVirtManifests> {
    let cpu_cores = recommended_cpu_cores(plan);
    let memory_gi = recommended_memory_gi(plan);
    let disk_claim = format!("{}-disk", disk.name);

    let mut dv_spec = serde_yaml::Mapping::new();
    dv_spec.insert(serde_yaml::Value::from("source"), data_volume_source(disk));
    let mut pvc = serde_yaml::Mapping::new();
    pvc.insert(
        serde_yaml::Value::from("accessModes"),
        serde_yaml::Value::Sequence(vec![serde_yaml::Value::from("ReadWriteOnce")]),
    );
    pvc.insert(
        serde_yaml::Value::from("resources"),
        serde_yaml::Value::Mapping({
            let mut r = serde_yaml::Mapping::new();
            if let Some(size) = disk.size_bytes {
                let gib = (size / (1024 * 1024 * 1024)).max(1);
                r.insert(
                    serde_yaml::Value::from("requests"),
                    serde_yaml::Value::Mapping({
                        let mut req = serde_yaml::Mapping::new();
                        req.insert(
                            serde_yaml::Value::from("storage"),
                            serde_yaml::Value::from(format!("{gib}Gi")),
                        );
                        req
                    }),
                );
            }
            r
        }),
    );
    pvc.insert(
        serde_yaml::Value::from("storageClassName"),
        serde_yaml::Value::from(disk.storage_class.clone()),
    );
    dv_spec.insert(
        serde_yaml::Value::from("pvc"),
        serde_yaml::Value::Mapping(pvc),
    );

    let mut dv_labels = serde_yaml::Mapping::new();
    dv_labels.insert(
        serde_yaml::Value::from("app"),
        serde_yaml::Value::from("zyvor"),
    );
    dv_labels.insert(
        serde_yaml::Value::from("guestkit.zyvor.dev/target"),
        serde_yaml::Value::from(plan.target.clone()),
    );

    let data_volume = serde_yaml::Value::Mapping({
        let mut m = serde_yaml::Mapping::new();
        m.insert(
            serde_yaml::Value::from("apiVersion"),
            serde_yaml::Value::from("cdi.kubevirt.io/v1beta1"),
        );
        m.insert(
            serde_yaml::Value::from("kind"),
            serde_yaml::Value::from("DataVolume"),
        );
        m.insert(
            serde_yaml::Value::from("metadata"),
            serde_yaml::Value::Mapping({
                let mut meta = serde_yaml::Mapping::new();
                meta.insert(
                    serde_yaml::Value::from("name"),
                    serde_yaml::Value::from(disk_claim.clone()),
                );
                meta.insert(
                    serde_yaml::Value::from("namespace"),
                    serde_yaml::Value::from(disk.namespace.clone()),
                );
                meta.insert(
                    serde_yaml::Value::from("labels"),
                    serde_yaml::Value::Mapping(dv_labels),
                );
                meta
            }),
        );
        m.insert(
            serde_yaml::Value::from("spec"),
            serde_yaml::Value::Mapping(dv_spec),
        );
        m
    });

    let vm = serde_yaml::Value::Mapping({
        let mut m = serde_yaml::Mapping::new();
        m.insert(
            serde_yaml::Value::from("apiVersion"),
            serde_yaml::Value::from("kubevirt.io/v1"),
        );
        m.insert(
            serde_yaml::Value::from("kind"),
            serde_yaml::Value::from("VirtualMachine"),
        );
        m.insert(
            serde_yaml::Value::from("metadata"),
            serde_yaml::Value::Mapping({
                let mut meta = serde_yaml::Mapping::new();
                meta.insert(
                    serde_yaml::Value::from("name"),
                    serde_yaml::Value::from(disk.name.clone()),
                );
                meta.insert(
                    serde_yaml::Value::from("namespace"),
                    serde_yaml::Value::from(disk.namespace.clone()),
                );
                meta.insert(
                    serde_yaml::Value::from("labels"),
                    serde_yaml::Value::Mapping({
                        let mut labels = serde_yaml::Mapping::new();
                        labels.insert(
                            serde_yaml::Value::from("app"),
                            serde_yaml::Value::from("zyvor"),
                        );
                        labels.insert(
                            serde_yaml::Value::from("guestkit.zyvor.dev/migration-score"),
                            serde_yaml::Value::from(format!("{:.0}", plan.migration_score.score)),
                        );
                        labels
                    }),
                );
                meta
            }),
        );
        m.insert(
            serde_yaml::Value::from("spec"),
            serde_yaml::Value::Mapping({
                let mut spec = serde_yaml::Mapping::new();
                spec.insert(
                    serde_yaml::Value::from("running"),
                    serde_yaml::Value::from(false),
                );
                spec.insert(
                    serde_yaml::Value::from("template"),
                    serde_yaml::Value::Mapping({
                        let mut tpl = serde_yaml::Mapping::new();
                        tpl.insert(
                            serde_yaml::Value::from("metadata"),
                            serde_yaml::Value::Mapping({
                                let mut meta = serde_yaml::Mapping::new();
                                meta.insert(
                                    serde_yaml::Value::from("labels"),
                                    serde_yaml::Value::Mapping({
                                        let mut labels = serde_yaml::Mapping::new();
                                        labels.insert(
                                            serde_yaml::Value::from("kubevirt.io/domain"),
                                            serde_yaml::Value::from(disk.name.clone()),
                                        );
                                        labels
                                    }),
                                );
                                meta
                            }),
                        );
                        tpl.insert(
                            serde_yaml::Value::from("spec"),
                            serde_yaml::Value::Mapping({
                                let mut vm_spec = serde_yaml::Mapping::new();
                                vm_spec.insert(
                                    serde_yaml::Value::from("domain"),
                                    serde_yaml::Value::Mapping({
                                        let mut domain = serde_yaml::Mapping::new();
                                        domain.insert(
                                            serde_yaml::Value::from("cpu"),
                                            serde_yaml::Value::Mapping({
                                                let mut cpu = serde_yaml::Mapping::new();
                                                cpu.insert(
                                                    serde_yaml::Value::from("cores"),
                                                    serde_yaml::Value::from(cpu_cores),
                                                );
                                                cpu
                                            }),
                                        );
                                        domain.insert(
                                            serde_yaml::Value::from("resources"),
                                            serde_yaml::Value::Mapping({
                                                let mut res = serde_yaml::Mapping::new();
                                                res.insert(
                                                    serde_yaml::Value::from("requests"),
                                                    serde_yaml::Value::Mapping({
                                                        let mut req = serde_yaml::Mapping::new();
                                                        req.insert(
                                                            serde_yaml::Value::from("memory"),
                                                            serde_yaml::Value::from(format!(
                                                                "{memory_gi}Gi"
                                                            )),
                                                        );
                                                        req
                                                    }),
                                                );
                                                res
                                            }),
                                        );
                                        domain.insert(
                                            serde_yaml::Value::from("devices"),
                                            serde_yaml::Value::Mapping({
                                                let mut devices = serde_yaml::Mapping::new();
                                                devices.insert(
                                                    serde_yaml::Value::from("disks"),
                                                    serde_yaml::Value::Sequence(vec![
                                                        serde_yaml::Value::Mapping({
                                                            let mut d = serde_yaml::Mapping::new();
                                                            d.insert(
                                                                serde_yaml::Value::from("name"),
                                                                serde_yaml::Value::from("rootdisk"),
                                                            );
                                                            d.insert(
                                                                serde_yaml::Value::from("disk"),
                                                                serde_yaml::Value::Mapping({
                                                                    let mut disk_map =
                                                                        serde_yaml::Mapping::new();
                                                                    disk_map.insert(
                                                                        serde_yaml::Value::from(
                                                                            "bus",
                                                                        ),
                                                                        serde_yaml::Value::from(
                                                                            "virtio",
                                                                        ),
                                                                    );
                                                                    disk_map
                                                                }),
                                                            );
                                                            d
                                                        }),
                                                    ]),
                                                );
                                                devices.insert(
                                                    serde_yaml::Value::from("interfaces"),
                                                    serde_yaml::Value::Sequence(vec![
                                                        serde_yaml::Value::Mapping({
                                                            let mut iface =
                                                                serde_yaml::Mapping::new();
                                                            iface.insert(
                                                                serde_yaml::Value::from("name"),
                                                                serde_yaml::Value::from("default"),
                                                            );
                                                            iface.insert(
                                                                serde_yaml::Value::from(
                                                                    "masquerade",
                                                                ),
                                                                serde_yaml::Value::Mapping(
                                                                    serde_yaml::Mapping::new(),
                                                                ),
                                                            );
                                                            iface.insert(
                                                                serde_yaml::Value::from("ports"),
                                                                serde_yaml::Value::Sequence(vec![]),
                                                            );
                                                            iface
                                                        }),
                                                    ]),
                                                );
                                                devices
                                            }),
                                        );
                                        domain
                                    }),
                                );
                                vm_spec.insert(
                                    serde_yaml::Value::from("networks"),
                                    serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(
                                        {
                                            let mut net = serde_yaml::Mapping::new();
                                            net.insert(
                                                serde_yaml::Value::from("name"),
                                                serde_yaml::Value::from("default"),
                                            );
                                            net.insert(
                                                serde_yaml::Value::from("pod"),
                                                serde_yaml::Value::Mapping(
                                                    serde_yaml::Mapping::new(),
                                                ),
                                            );
                                            net
                                        },
                                    )]),
                                );
                                vm_spec.insert(
                                    serde_yaml::Value::from("volumes"),
                                    serde_yaml::Value::Sequence(vec![serde_yaml::Value::Mapping(
                                        {
                                            let mut vol = serde_yaml::Mapping::new();
                                            vol.insert(
                                                serde_yaml::Value::from("name"),
                                                serde_yaml::Value::from("rootdisk"),
                                            );
                                            vol.insert(
                                                serde_yaml::Value::from("persistentVolumeClaim"),
                                                serde_yaml::Value::Mapping({
                                                    let mut pvc_ref = serde_yaml::Mapping::new();
                                                    pvc_ref.insert(
                                                        serde_yaml::Value::from("claimName"),
                                                        serde_yaml::Value::from(disk_claim),
                                                    );
                                                    pvc_ref
                                                }),
                                            );
                                            vol
                                        },
                                    )]),
                                );
                                vm_spec
                            }),
                        );
                        tpl
                    }),
                );
                spec
            }),
        );
        m
    });

    Ok(KubeVirtManifests {
        data_volume,
        virtual_machine: vm,
    })
}

/// Serialize manifests as multi-document YAML.
pub fn manifests_to_yaml(manifests: &KubeVirtManifests) -> Result<String> {
    let dv = serde_yaml::to_string(&manifests.data_volume).context("serialize DataVolume")?;
    let vm =
        serde_yaml::to_string(&manifests.virtual_machine).context("serialize VirtualMachine")?;
    Ok(format!("---\n{dv}---\n{vm}"))
}

fn data_volume_source(disk: &DiskMetadata) -> serde_yaml::Value {
    if let Some(url) = &disk.import_url {
        serde_yaml::Value::Mapping({
            let mut src = serde_yaml::Mapping::new();
            src.insert(
                serde_yaml::Value::from("http"),
                serde_yaml::Value::Mapping({
                    let mut http = serde_yaml::Mapping::new();
                    http.insert(
                        serde_yaml::Value::from("url"),
                        serde_yaml::Value::from(url.clone()),
                    );
                    http
                }),
            );
            src
        })
    } else {
        serde_yaml::Value::Mapping({
            let mut src = serde_yaml::Mapping::new();
            src.insert(
                serde_yaml::Value::from("upload"),
                serde_yaml::Value::Mapping(serde_yaml::Mapping::new()),
            );
            src
        })
    }
}

fn recommended_cpu_cores(plan: &MigrationPlanResult) -> u32 {
    let score = plan.bootability.score;
    if score >= 80.0 {
        4
    } else if score >= 50.0 {
        2
    } else {
        1
    }
}

fn recommended_memory_gi(plan: &MigrationPlanResult) -> u32 {
    let score = plan.bootability.score;
    if score >= 80.0 {
        8
    } else if score >= 50.0 {
        4
    } else {
        2
    }
}

fn sanitize_k8s_name(name: &str) -> String {
    let lower = name.to_lowercase();
    let sanitized: String = lower
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let trimmed = sanitized.trim_matches('-');
    if trimmed.is_empty() {
        "migrated-vm".to_string()
    } else {
        trimmed.to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assurance::MigrationPlanResult;
    use crate::boot::BootabilityReport;
    use crate::cli::migrate::plan::MigrationScoreReport;

    fn sample_plan() -> MigrationPlanResult {
        MigrationPlanResult {
            target: "kubevirt".to_string(),
            migration_score: MigrationScoreReport {
                score: 82.0,
                driver_injections: vec!["virtio_blk".to_string()],
                required_changes: vec![],
                licensing_warnings: vec![],
                estimated_downtime_minutes: 15,
            },
            bootability: BootabilityReport {
                score: 82.0,
                confidence: 0.9,
                target: "kubevirt".to_string(),
                blockers: vec![],
                warnings: vec![],
                checks: vec![],
                summary: "ok".to_string(),
            },
            root_cause: None,
            fix_plan: None,
            evidence_digest: None,
            copilot: None,
        }
    }

    #[test]
    fn generates_vm_and_datavolume() {
        let plan = sample_plan();
        let disk = DiskMetadata {
            name: "test-vm".to_string(),
            format: "qcow2".to_string(),
            size_bytes: Some(20 * 1024 * 1024 * 1024),
            storage_class: "longhorn".to_string(),
            import_url: Some("http://minio:9000/bucket/test.qcow2".to_string()),
            namespace: "zyvor".to_string(),
        };
        let manifests = generate_kubevirt_manifests(&plan, &disk).unwrap();
        let yaml = manifests_to_yaml(&manifests).unwrap();
        assert!(yaml.contains("kind: DataVolume"));
        assert!(yaml.contains("kind: VirtualMachine"));
        assert!(yaml.contains("virtio"));
        assert!(yaml.contains("test-vm-disk"));
    }
}
