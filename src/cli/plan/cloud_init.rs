// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Offline cloud-init datasource + seed inject.
//!
//! migrate-plan already says "reconfigure datasource". This writes the
//! FileWrite operations so `plan apply` can do it without virsh or a live guest.

use super::types::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Datasource {
    Ec2,
    Azure,
    Gce,
    OpenStack,
    NoCloud,
}

impl Datasource {
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "aws" | "ec2" | "amazon" => Some(Self::Ec2),
            "azure" | "az" => Some(Self::Azure),
            "gcp" | "gce" | "google" => Some(Self::Gce),
            "openstack" | "os" => Some(Self::OpenStack),
            "nocloud" | "none" | "seed" => Some(Self::NoCloud),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Ec2 => "ec2",
            Self::Azure => "azure",
            Self::Gce => "gce",
            Self::OpenStack => "openstack",
            Self::NoCloud => "nocloud",
        }
    }

    pub fn cfg(self) -> &'static str {
        match self {
            Self::Ec2 => {
                "datasource_list: [ Ec2, None ]\n\
                 datasource:\n\
                   Ec2:\n\
                     timeout: 50\n\
                     max_wait: 120\n"
            }
            Self::Azure => "datasource_list: [ Azure, None ]\n",
            Self::Gce => "datasource_list: [ GCE, None ]\n",
            Self::OpenStack => "datasource_list: [ OpenStack, ConfigDrive, None ]\n",
            Self::NoCloud => "datasource_list: [ NoCloud, None ]\n",
        }
    }
}

fn write_op(id: &str, path: &str, content: &str, desc: &str) -> Operation {
    Operation {
        id: id.into(),
        op_type: OperationType::FileWrite(FileWrite {
            path: path.into(),
            content: content.to_string(),
            mode: Some("0644".into()),
        }),
        priority: Priority::High,
        description: desc.into(),
        risk: Priority::Low,
        reversible: true,
        depends_on: vec![],
        validation: None,
        undo: Some(UndoInfo::Command {
            command: format!("rm -f {path}"),
        }),
    }
}

pub struct CloudInitOpts<'a> {
    pub vm: &'a str,
    pub ds: Datasource,
    pub user_data: Option<&'a str>,
    pub meta_data: Option<&'a str>,
    pub disable_network: bool,
    pub instance_id: Option<&'a str>,
}

pub fn cloud_init_plan(opts: CloudInitOpts<'_>) -> FixPlan {
    let mut plan = FixPlan::new(
        opts.vm.to_string(),
        format!("cloud-init-{}", opts.ds.name()),
    );
    plan.version = "1".into();
    plan.overall_risk = "medium".into();
    plan.estimated_duration = "seconds".into();
    plan.metadata.author = "guestkit".into();
    plan.metadata.review_required = true;
    plan.metadata.reversible = true;
    plan.metadata.description = Some(format!(
        "Offline cloud-init datasource → {}",
        opts.ds.name()
    ));
    plan.metadata.tags = vec!["cloud-init".into(), opts.ds.name().into(), "offline".into()];

    plan.add_operation(write_op(
        "datasource",
        "/etc/cloud/cloud.cfg.d/99-guestkit-datasource.cfg",
        opts.ds.cfg(),
        &format!("Pin cloud-init datasource to {}", opts.ds.name()),
    ));

    if opts.disable_network {
        plan.add_operation(write_op(
            "network-disabled",
            "/etc/cloud/cloud.cfg.d/99-guestkit-network.cfg",
            "network: {config: disabled}\n",
            "Disable cloud-init network so existing guest NICs stay put",
        ));
    }

    if opts.ds == Datasource::NoCloud
        || opts.user_data.is_some()
        || opts.meta_data.is_some()
        || opts.instance_id.is_some()
    {
        let iid = opts.instance_id.unwrap_or("guestkit-imported");
        let meta = opts
            .meta_data
            .map(str::to_string)
            .unwrap_or_else(|| format!("instance-id: {iid}\nlocal-hostname: {iid}\n"));
        plan.add_operation(write_op(
            "seed-meta",
            "/var/lib/cloud/seed/nocloud/meta-data",
            &meta,
            "NoCloud seed meta-data",
        ));
        let ud = opts.user_data.unwrap_or("#cloud-config\n");
        plan.add_operation(write_op(
            "seed-user",
            "/var/lib/cloud/seed/nocloud/user-data",
            ud,
            "NoCloud seed user-data",
        ));
    }

    plan
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aws_writes_ec2_datasource() {
        let p = cloud_init_plan(CloudInitOpts {
            vm: "disk.qcow2",
            ds: Datasource::Ec2,
            user_data: None,
            meta_data: None,
            disable_network: false,
            instance_id: None,
        });
        assert_eq!(p.profile, "cloud-init-ec2");
        match &p.operations[0].op_type {
            OperationType::FileWrite(fw) => {
                assert!(fw.content.contains("Ec2"));
                assert_eq!(fw.path, "/etc/cloud/cloud.cfg.d/99-guestkit-datasource.cfg");
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn nocloud_seeds_user_data() {
        let p = cloud_init_plan(CloudInitOpts {
            vm: "disk.qcow2",
            ds: Datasource::NoCloud,
            user_data: Some("#cloud-config\nusers:\n  - name: ops\n"),
            meta_data: None,
            disable_network: true,
            instance_id: Some("web01"),
        });
        assert!(p.operations.iter().any(|o| o.id == "seed-user"));
        assert!(p.operations.iter().any(|o| o.id == "network-disabled"));
        let seed = p.operations.iter().find(|o| o.id == "seed-meta").unwrap();
        match &seed.op_type {
            OperationType::FileWrite(fw) => assert!(fw.content.contains("web01")),
            _ => panic!("expected write"),
        }
    }
}
