// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! cloud-init first-boot status from a live guest.

use crate::evidence::snapshot::CloudInitEvidence;
use std::path::Path;
use std::process::Command;

pub fn collect_cloud_init_live() -> CloudInitEvidence {
    let installed = Path::new("/usr/bin/cloud-init").exists()
        || Path::new("/bin/cloud-init").exists()
        || Path::new("/etc/cloud").exists();
    if !installed {
        return CloudInitEvidence {
            installed: false,
            ..Default::default()
        };
    }

    let mut evidence = CloudInitEvidence {
        installed: true,
        ..Default::default()
    };

    if let Ok(out) = Command::new("cloud-init")
        .args(["status", "--format=json"])
        .output()
    {
        if out.status.success() {
            if let Ok(json) = serde_json::from_slice::<serde_json::Value>(&out.stdout) {
                evidence.status = json
                    .get("status")
                    .and_then(|v| v.as_str())
                    .unwrap_or("unknown")
                    .to_string();
                evidence.boot_finished = json
                    .get("boot_status_code")
                    .and_then(|v| v.as_str())
                    .is_some_and(|s| s == "enabled-by-generator" || s.contains("done"));
            }
        }
    } else if let Ok(out) = Command::new("cloud-init").arg("status").output() {
        let text = String::from_utf8_lossy(&out.stdout);
        evidence.status = text.trim().to_string();
        evidence.boot_finished = text.contains("done");
    }

    if let Ok(out) = Command::new("cloud-init")
        .args(["query", "datasource"])
        .output()
    {
        if out.status.success() {
            evidence.datasource = String::from_utf8_lossy(&out.stdout).trim().to_string();
        }
    }

    let log_path = "/var/log/cloud-init.log";
    if Path::new(log_path).exists() {
        if let Ok(content) = std::fs::read_to_string(log_path) {
            for line in content.lines().rev().take(200) {
                if line.contains("ERROR") || line.contains("WARNING") {
                    evidence.errors.push(line.to_string());
                    if evidence.errors.len() >= 5 {
                        break;
                    }
                }
            }
            evidence.last_log_line = content.lines().last().map(str::to_string);
        }
    }

    evidence
}
