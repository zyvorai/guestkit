// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Extract disk image paths from libvirt domain XML or KubeVirt VM/VMI YAML.
//!
//! Replaces `virsh dumpxml | grep source file=` without talking to libvirt.

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DomainDisk {
    /// Guest target (vda, sda, …) when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    /// Host path, PVC name, or volume name.
    pub source: String,
    /// file | block | network | pvc | dv | container | cloudinit | unknown
    pub kind: String,
    pub bootable: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DomainDisks {
    pub document: String,
    pub format: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    pub disks: Vec<DomainDisk>,
}

/// Parse a libvirt XML or KubeVirt YAML file and list attachable disks.
pub fn parse_domain_disks(path: &Path) -> Result<DomainDisks> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    parse_domain_disks_str(&raw, path)
}

pub fn parse_domain_disks_str(raw: &str, origin: &Path) -> Result<DomainDisks> {
    let trimmed = raw.trim_start();
    if trimmed.starts_with('<') {
        Ok(parse_libvirt_xml(raw, origin))
    } else {
        parse_kubevirt_yaml(raw, origin)
    }
}

fn parse_libvirt_xml(raw: &str, origin: &Path) -> DomainDisks {
    let name = xml_tag_text(raw, "name");
    let mut disks = Vec::new();
    for block in xml_blocks(raw, "disk") {
        let target = xml_attr(&block, "target", "dev");
        let bootable = block.contains("<boot");
        let (kind, source) = if let Some(f) = xml_attr(&block, "source", "file") {
            ("file".into(), f)
        } else if let Some(d) = xml_attr(&block, "source", "dev") {
            ("block".into(), d)
        } else if let Some(n) = xml_attr(&block, "source", "name") {
            ("network".into(), n)
        } else {
            continue;
        };
        disks.push(DomainDisk {
            target,
            source,
            kind,
            bootable,
        });
    }
    DomainDisks {
        document: origin.display().to_string(),
        format: "libvirt-xml".into(),
        name,
        disks,
    }
}

fn parse_kubevirt_yaml(raw: &str, origin: &Path) -> Result<DomainDisks> {
    let value: serde_yaml::Value =
        serde_yaml::from_str(raw).context("parse KubeVirt / Kubernetes YAML")?;
    let name = value
        .get("metadata")
        .and_then(|m| m.get("name"))
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // VM: spec.template.spec ; VMI: spec
    let spec = value
        .get("spec")
        .and_then(|s| s.get("template"))
        .and_then(|t| t.get("spec"))
        .or_else(|| value.get("spec"));

    let mut disks = Vec::new();
    if let Some(spec) = spec {
        let devices = spec
            .get("domain")
            .and_then(|d| d.get("devices"))
            .and_then(|d| d.get("disks"))
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_default();
        let volumes = spec
            .get("volumes")
            .and_then(|v| v.as_sequence())
            .cloned()
            .unwrap_or_default();

        for disk in devices {
            let vol_name = disk
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let target = disk
                .get("disk")
                .and_then(|d| d.get("bus"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());
            let bootable = disk.get("bootOrder").is_some();
            if let Some(vol) = volumes
                .iter()
                .find(|v| v.get("name").and_then(|n| n.as_str()) == Some(vol_name.as_str()))
            {
                disks.push(volume_to_disk(vol, target, bootable, &vol_name));
            }
        }
    }

    Ok(DomainDisks {
        document: origin.display().to_string(),
        format: "kubevirt-yaml".into(),
        name,
        disks,
    })
}

fn volume_to_disk(
    vol: &serde_yaml::Value,
    target: Option<String>,
    bootable: bool,
    vol_name: &str,
) -> DomainDisk {
    if let Some(pvc) = vol
        .get("persistentVolumeClaim")
        .and_then(|p| p.get("claimName"))
        .and_then(|v| v.as_str())
    {
        return DomainDisk {
            target,
            source: pvc.to_string(),
            kind: "pvc".into(),
            bootable,
        };
    }
    if let Some(dv) = vol
        .get("dataVolume")
        .and_then(|d| d.get("name"))
        .and_then(|v| v.as_str())
    {
        return DomainDisk {
            target,
            source: dv.to_string(),
            kind: "dv".into(),
            bootable,
        };
    }
    if let Some(p) = vol
        .get("hostDisk")
        .and_then(|h| h.get("path"))
        .and_then(|v| v.as_str())
    {
        return DomainDisk {
            target,
            source: p.to_string(),
            kind: "file".into(),
            bootable,
        };
    }
    if let Some(img) = vol
        .get("containerDisk")
        .and_then(|c| c.get("image"))
        .and_then(|v| v.as_str())
    {
        return DomainDisk {
            target,
            source: img.to_string(),
            kind: "container".into(),
            bootable,
        };
    }
    if vol.get("cloudInitNoCloud").is_some() || vol.get("cloudInitConfigDrive").is_some() {
        return DomainDisk {
            target,
            source: vol_name.to_string(),
            kind: "cloudinit".into(),
            bootable,
        };
    }
    DomainDisk {
        target,
        source: vol_name.to_string(),
        kind: "unknown".into(),
        bootable,
    }
}

pub fn file_sources(report: &DomainDisks) -> Vec<PathBuf> {
    report
        .disks
        .iter()
        .filter(|d| d.kind == "file")
        .map(|d| PathBuf::from(&d.source))
        .collect()
}

fn xml_tag_text(raw: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = raw.find(&open)? + open.len();
    let end = raw[start..].find(&close)? + start;
    Some(raw[start..end].trim().to_string())
}

fn xml_blocks(raw: &str, tag: &str) -> Vec<String> {
    let open = format!("<{tag}");
    let close = format!("</{tag}>");
    let mut out = Vec::new();
    let mut rest = raw;
    while let Some(idx) = rest.find(&open) {
        let slice = &rest[idx..];
        if let Some(end) = slice.find(&close) {
            out.push(slice[..end + close.len()].to_string());
            rest = &slice[end + close.len()..];
        } else if let Some(end) = slice.find("/>") {
            out.push(slice[..end + 2].to_string());
            rest = &slice[end + 2..];
        } else {
            break;
        }
    }
    out
}

fn xml_attr(block: &str, tag: &str, attr: &str) -> Option<String> {
    let needle = format!("<{tag}");
    let start = block.find(&needle)?;
    let after = &block[start..];
    let end = after.find('>').unwrap_or(after.len());
    let head = &after[..end];
    for quote in ['\'', '"'] {
        let pat = format!("{attr}={quote}");
        if let Some(i) = head.find(&pat) {
            let rest = &head[i + pat.len()..];
            if let Some(j) = rest.find(quote) {
                return Some(rest[..j].to_string());
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    #[test]
    fn parses_libvirt_file_and_block() {
        let xml = r#"
            <domain type='kvm'>
              <name>web01</name>
              <devices>
                <disk type='file' device='disk'>
                  <source file='/var/lib/libvirt/images/web01.qcow2'/>
                  <target dev='vda' bus='virtio'/>
                  <boot order='1'/>
                </disk>
                <disk type='block' device='disk'>
                  <source dev='/dev/vg/data'/>
                  <target dev='vdb' bus='virtio'/>
                </disk>
              </devices>
            </domain>
        "#;
        let r = parse_domain_disks_str(xml, Path::new("web01.xml")).unwrap();
        assert_eq!(r.format, "libvirt-xml");
        assert_eq!(r.name.as_deref(), Some("web01"));
        assert_eq!(r.disks.len(), 2);
        assert_eq!(r.disks[0].source, "/var/lib/libvirt/images/web01.qcow2");
        assert!(r.disks[0].bootable);
        assert_eq!(r.disks[1].kind, "block");
    }

    #[test]
    fn parses_kubevirt_vm() {
        let yaml = r##"
apiVersion: kubevirt.io/v1
kind: VirtualMachine
metadata:
  name: db
spec:
  template:
    spec:
      domain:
        devices:
          disks:
          - name: root
            disk:
              bus: virtio
            bootOrder: 1
          - name: seed
            disk:
              bus: virtio
      volumes:
      - name: root
        persistentVolumeClaim:
          claimName: db-root
      - name: seed
        cloudInitNoCloud:
          userData: "#cloud-config"
"##;
        let r = parse_domain_disks_str(yaml, Path::new("db.yaml")).unwrap();
        assert_eq!(r.format, "kubevirt-yaml");
        assert_eq!(r.name.as_deref(), Some("db"));
        assert_eq!(r.disks[0].kind, "pvc");
        assert_eq!(r.disks[0].source, "db-root");
        assert_eq!(r.disks[1].kind, "cloudinit");
    }
}
