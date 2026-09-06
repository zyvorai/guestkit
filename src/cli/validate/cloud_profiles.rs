// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Cloud cutover profiles: AWS / Azure / GCP / OpenStack.
//!
//! These are GuestKit Policy packs (same engine as `policy check -b cis-ubuntu`).
//! They encode what the destination expects, not how to call the cloud API.

use super::policy::{Policy, PolicyRule, RuleType};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CloudProfile {
    Aws,
    Azure,
    Gcp,
    OpenStack,
}

impl CloudProfile {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aws" | "ec2" | "amazon" => Some(Self::Aws),
            "azure" | "az" | "entra" => Some(Self::Azure),
            "gcp" | "gce" | "google" => Some(Self::Gcp),
            "openstack" | "os" => Some(Self::OpenStack),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Aws => "aws",
            Self::Azure => "azure",
            Self::Gcp => "gcp",
            Self::OpenStack => "openstack",
        }
    }

    pub fn to_policy(self) -> Policy {
        match self {
            Self::Aws => aws(),
            Self::Azure => azure(),
            Self::Gcp => gcp(),
            Self::OpenStack => openstack(),
        }
    }

    pub fn all() -> &'static [CloudProfile] {
        &[Self::Aws, Self::Azure, Self::Gcp, Self::OpenStack]
    }
}

fn rule(id: &str, name: &str, path: &str, severity: &str, rem: &str) -> PolicyRule {
    PolicyRule {
        id: id.into(),
        name: name.into(),
        description: format!("Expect {path} on a {name} image"),
        severity: severity.into(),
        rule_type: RuleType::FileExists { path: path.into() },
        expr: None,
        remediation: Some(rem.into()),
    }
}

fn expr_rule(id: &str, name: &str, expr: &str, severity: &str, rem: &str) -> PolicyRule {
    PolicyRule {
        id: id.into(),
        name: name.into(),
        description: expr.into(),
        severity: severity.into(),
        rule_type: RuleType::Expression { expr: expr.into() },
        expr: Some(expr.into()),
        remediation: Some(rem.into()),
    }
}

fn aws() -> Policy {
    Policy {
        name: "AWS EC2 cutover profile".into(),
        version: "1".into(),
        description: "cloud-init + SSH + no telnet; NVMe/Xen tooling is optional".into(),
        rules: vec![
            rule(
                "AWS-001",
                "cloud-init present",
                "/usr/bin/cloud-init",
                "high",
                "Install cloud-init so EC2 metadata can set hostname/SSH keys",
            ),
            rule(
                "AWS-002",
                "cloud.cfg present",
                "/etc/cloud/cloud.cfg",
                "medium",
                "Ensure /etc/cloud/cloud.cfg exists (datasource: Ec2)",
            ),
            PolicyRule {
                id: "AWS-003".into(),
                name: "telnet absent".into(),
                description: "telnet has no place on an EC2 image".into(),
                severity: "high".into(),
                rule_type: RuleType::PackageForbidden {
                    package: "telnet".into(),
                },
                expr: None,
                remediation: Some("Remove telnet".into()),
            },
            expr_rule(
                "AWS-004",
                "boot score floor",
                "bootability.score >= 80",
                "high",
                "Run guestkit doctor --target aws and repair before import",
            ),
        ],
    }
}

fn azure() -> Policy {
    Policy {
        name: "Azure cutover profile".into(),
        version: "1".into(),
        description: "WALinuxAgent or cloud-init + SSH".into(),
        rules: vec![
            rule(
                "AZ-001",
                "cloud-init or waagent bin",
                "/usr/bin/cloud-init",
                "high",
                "Install cloud-init or WALinuxAgent (waagent)",
            ),
            rule(
                "AZ-002",
                "cloud.cfg",
                "/etc/cloud/cloud.cfg",
                "medium",
                "cloud-init datasource Azure",
            ),
            expr_rule(
                "AZ-003",
                "boot score floor",
                "bootability.score >= 80",
                "high",
                "guestkit doctor --target azure",
            ),
        ],
    }
}

fn gcp() -> Policy {
    Policy {
        name: "GCP GCE cutover profile".into(),
        version: "1".into(),
        description: "google-guest-agent / cloud-init".into(),
        rules: vec![
            rule(
                "GCP-001",
                "cloud-init",
                "/usr/bin/cloud-init",
                "high",
                "Install cloud-init (GCE datasource) or google-guest-agent",
            ),
            rule(
                "GCP-002",
                "cloud.cfg",
                "/etc/cloud/cloud.cfg",
                "medium",
                "Ensure cloud-init config exists",
            ),
            expr_rule(
                "GCP-003",
                "boot score floor",
                "bootability.score >= 80",
                "high",
                "guestkit doctor --target gcp",
            ),
        ],
    }
}

fn openstack() -> Policy {
    Policy {
        name: "OpenStack cutover profile".into(),
        version: "1".into(),
        description: "cloud-init ConfigDrive/metadata".into(),
        rules: vec![
            rule(
                "OS-001",
                "cloud-init",
                "/usr/bin/cloud-init",
                "high",
                "Install cloud-init for metadata/config-drive",
            ),
            rule(
                "OS-002",
                "cloud.cfg",
                "/etc/cloud/cloud.cfg",
                "medium",
                "Datasource: OpenStack / ConfigDrive",
            ),
            expr_rule(
                "OS-003",
                "boot score floor",
                "bootability.score >= 80",
                "high",
                "guestkit doctor --target kvm (OpenStack still boots KVM)",
            ),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_aliases() {
        assert_eq!(CloudProfile::parse("ec2").unwrap().name(), "aws");
        assert_eq!(CloudProfile::parse("gce").unwrap().name(), "gcp");
    }

    #[test]
    fn aws_policy_has_cloud_init_rule() {
        let p = CloudProfile::Aws.to_policy();
        assert!(p.rules.iter().any(|r| r.id == "AWS-001"));
    }
}
