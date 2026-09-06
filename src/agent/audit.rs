// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Central audit trail for mutating agent operations.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::Path;

const AUDIT_LOG: &str = "/var/lib/guestkit/audit.log";
const AUDIT_LOG_LEGACY: &str = "/var/log/zyvor/agent-audit.log";

/// Append one audit line; falls back to the legacy zyvor path, then the
/// process log, so an audit sink always exists. Detail must already be
/// secret-free (callers never pass raw payloads).
pub fn audit(method: &str, outcome: &str, detail: &str) {
    let line = format!(
        "{} method={} outcome={} detail={}\n",
        chrono::Utc::now().to_rfc3339(),
        method,
        outcome,
        detail.replace('\n', " ")
    );
    for path in [AUDIT_LOG, AUDIT_LOG_LEGACY] {
        if let Some(parent) = Path::new(path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
            if file.write_all(line.as_bytes()).is_ok() {
                return;
            }
        }
    }
    log::info!("audit: {}", line.trim_end());
}
