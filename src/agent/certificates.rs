// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Certificate and SSH host-key inventory (spec §23).
//!
//! Read-only: discovers X.509 certificates in standard locations, parses
//! expiry / key size / signature algorithm, and flags expiring or weak
//! certificates. Also inventories SSH host keys. Private keys are never
//! read or exported. Parsing uses the `openssl` CLI (universally present),
//! matching the codebase's shell-out convention.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::process::Command;

const EXPIRY_WARN_DAYS: i64 = 30;
const WEAK_RSA_BITS: u32 = 2048;
const MAX_CERTS: usize = 200;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertInfo {
    pub path: String,
    pub subject: String,
    pub issuer: String,
    pub not_after: String,
    pub days_until_expiry: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key_bits: Option<u32>,
    pub signature_algorithm: String,
    pub expiring_soon: bool,
    pub expired: bool,
    pub weak: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SshHostKey {
    pub path: String,
    pub key_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub bits: Option<u32>,
    pub fingerprint: String,
}

fn cert_search_dirs() -> Vec<PathBuf> {
    [
        "/etc/ssl/certs",
        "/etc/pki/tls/certs",
        "/etc/pki/tls/private",
        "/etc/nginx",
        "/etc/nginx/ssl",
        "/etc/apache2/ssl",
        "/etc/httpd/conf.d",
        "/etc/letsencrypt/live",
        "/etc/kubernetes/pki",
    ]
    .iter()
    .map(PathBuf::from)
    .filter(|p| p.is_dir())
    .collect()
}

fn is_cert_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("crt") | Some("pem") | Some("cer") | Some("cert")
    )
}

/// Parse one certificate file via openssl. Returns None for anything that
/// isn't a readable certificate (e.g. a private key or bundle head).
fn parse_cert(path: &Path) -> Option<CertInfo> {
    let out = Command::new("openssl")
        .args(["x509", "-noout", "-subject", "-issuer", "-enddate", "-text"])
        .arg("-in")
        .arg(path)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let text = String::from_utf8_lossy(&out.stdout);
    let field = |prefix: &str| -> Option<String> {
        text.lines()
            .find(|l| l.trim_start().starts_with(prefix))
            .map(|l| l.trim().trim_start_matches(prefix).trim().to_string())
    };
    let subject = field("subject=").unwrap_or_default();
    let issuer = field("issuer=").unwrap_or_default();
    let not_after = field("notAfter=").unwrap_or_default();
    let sig_alg = text
        .lines()
        .find(|l| l.trim_start().starts_with("Signature Algorithm:"))
        .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        .unwrap_or_default();
    let key_bits = text
        .lines()
        .find(|l| l.contains("Public-Key:"))
        .and_then(|l| {
            l.split('(')
                .nth(1)
                .and_then(|s| s.split_whitespace().next())
                .and_then(|n| n.parse().ok())
        });
    // "Public Key Algorithm: id-ecPublicKey" | "rsaEncryption" | ...
    let key_algorithm = text
        .lines()
        .find(|l| l.trim_start().starts_with("Public Key Algorithm:"))
        .map(|l| l.split(':').nth(1).unwrap_or("").trim().to_string())
        .unwrap_or_default();

    let days = parse_expiry_days(&not_after);
    let weak = is_weak_key(&key_algorithm, key_bits, &sig_alg);

    Some(CertInfo {
        path: path.display().to_string(),
        subject,
        issuer,
        not_after,
        days_until_expiry: days.unwrap_or(0),
        key_bits,
        signature_algorithm: sig_alg,
        expiring_soon: days
            .map(|d| (0..=EXPIRY_WARN_DAYS).contains(&d))
            .unwrap_or(false),
        expired: days.map(|d| d < 0).unwrap_or(false),
        weak,
    })
}

/// Weak-key heuristic that is algorithm-aware: the 2048-bit floor only
/// applies to RSA/DSA. ECC/EdDSA keys use far smaller key sizes for
/// equivalent strength (P-256 ≈ RSA-3072), so a 256-bit ECC key is strong,
/// not weak. A SHA-1/MD5 signature is weak regardless of key type.
fn is_weak_key(algorithm: &str, key_bits: Option<u32>, sig_alg: &str) -> bool {
    let sig = sig_alg.to_lowercase();
    if sig.contains("md5") || (sig.contains("sha1") && !sig.contains("sha1-")) {
        return true;
    }
    let algo = algorithm.to_lowercase();
    let is_ecc = algo.contains("ec") || algo.contains("ed25519") || algo.contains("ed448");
    if is_ecc {
        // Sub-224-bit ECC curves are considered weak; P-256+ is strong.
        return matches!(key_bits, Some(bits) if bits < 224);
    }
    // RSA/DSA (or unknown): apply the classic 2048-bit floor.
    matches!(key_bits, Some(bits) if bits < WEAK_RSA_BITS)
}

/// Days until `notAfter` using `openssl`-formatted dates ("MMM DD HH:MM:SS YYYY GMT").
fn parse_expiry_days(not_after: &str) -> Option<i64> {
    let dt = chrono::NaiveDateTime::parse_from_str(
        not_after.trim_end_matches(" GMT").trim(),
        "%b %e %H:%M:%S %Y",
    )
    .ok()?;
    let now = chrono::Utc::now().naive_utc();
    Some((dt - now).num_days())
}

fn collect_ssh_host_keys() -> Vec<SshHostKey> {
    let mut keys = Vec::new();
    let dir = Path::new("/etc/ssh");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return keys;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        // Public host keys only — never touch private material.
        if !name.starts_with("ssh_host_") || !name.ends_with(".pub") {
            continue;
        }
        // ssh-keygen -l -f <pub>  -> "<bits> SHA256:<fp> <comment> (<type>)"
        if let Ok(out) = Command::new("ssh-keygen")
            .args(["-l", "-f"])
            .arg(&path)
            .output()
        {
            if out.status.success() {
                let line = String::from_utf8_lossy(&out.stdout);
                let parts: Vec<&str> = line.split_whitespace().collect();
                let bits = parts.first().and_then(|b| b.parse().ok());
                let fingerprint = parts.get(1).unwrap_or(&"").to_string();
                let key_type = line
                    .rsplit('(')
                    .next()
                    .map(|s| s.trim_end_matches(')').trim().to_string())
                    .unwrap_or_default();
                keys.push(SshHostKey {
                    path: path.display().to_string(),
                    key_type,
                    bits,
                    fingerprint,
                });
            }
        }
    }
    keys
}

#[cfg(target_os = "windows")]
fn windows_cert_inventory() -> Vec<CertInfo> {
    let script = "Get-ChildItem Cert:\\LocalMachine\\My -ErrorAction SilentlyContinue | \
        Select-Object Subject,Issuer,@{n='NotAfter';e={$_.NotAfter.ToString('o')}},\
        @{n='Days';e={($_.NotAfter - (Get-Date)).Days}},\
        @{n='SigAlg';e={$_.SignatureAlgorithm.FriendlyName}},\
        @{n='KeyBits';e={$_.PublicKey.Key.KeySize}},\
        @{n='KeyAlgorithm';e={$_.PublicKey.Oid.FriendlyName}} | ConvertTo-Json -Compress";
    let out = Command::new("powershell")
        .args(["-NoProfile", "-NonInteractive", "-Command", script])
        .output();
    let Ok(out) = out else { return Vec::new() };
    let json_text = String::from_utf8_lossy(&out.stdout);
    #[derive(serde::Deserialize)]
    struct Row {
        #[serde(rename = "Subject", default)]
        subject: String,
        #[serde(rename = "Issuer", default)]
        issuer: String,
        #[serde(rename = "NotAfter", default)]
        not_after: String,
        #[serde(rename = "Days", default)]
        days: i64,
        #[serde(rename = "SigAlg", default)]
        sig_alg: Option<String>,
        #[serde(rename = "KeyBits", default)]
        key_bits: Option<u32>,
        #[serde(rename = "KeyAlgorithm", default)]
        algorithm: Option<String>,
    }
    let rows: Vec<Row> = if json_text.trim_start().starts_with('[') {
        serde_json::from_str(&json_text).unwrap_or_default()
    } else if json_text.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str::<Row>(&json_text)
            .map(|r| vec![r])
            .unwrap_or_default()
    };
    rows.into_iter()
        .map(|r| {
            let sig = r.sig_alg.unwrap_or_default();
            CertInfo {
                path: "Cert:\\LocalMachine\\My".into(),
                subject: r.subject,
                issuer: r.issuer,
                not_after: r.not_after,
                days_until_expiry: r.days,
                key_bits: r.key_bits,
                weak: is_weak_key(&r.algorithm.clone().unwrap_or_default(), r.key_bits, &sig),
                signature_algorithm: sig,
                expiring_soon: r.days >= 0 && r.days <= EXPIRY_WARN_DAYS,
                expired: r.days < 0,
            }
        })
        .collect()
}

pub fn inventory() -> Value {
    let certs: Vec<CertInfo> = {
        #[cfg(target_os = "windows")]
        {
            windows_cert_inventory()
        }
        #[cfg(not(target_os = "windows"))]
        {
            let mut found = Vec::new();
            for dir in cert_search_dirs() {
                let Ok(entries) = std::fs::read_dir(&dir) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_file() && is_cert_file(&path) {
                        if let Some(info) = parse_cert(&path) {
                            found.push(info);
                        }
                    }
                    if found.len() >= MAX_CERTS {
                        break;
                    }
                }
            }
            found
        }
    };

    let ssh_host_keys = if cfg!(target_os = "windows") {
        Vec::new()
    } else {
        collect_ssh_host_keys()
    };

    let expiring = certs.iter().filter(|c| c.expiring_soon).count();
    let expired = certs.iter().filter(|c| c.expired).count();
    let weak = certs.iter().filter(|c| c.weak).count();

    json!({
        "certificate_count": certs.len(),
        "expiring_soon": expiring,
        "expired": expired,
        "weak": weak,
        "certificates": certs,
        "ssh_host_keys": ssh_host_keys,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn expiry_days_parse() {
        // A date far in the future should yield a large positive number.
        let days = parse_expiry_days("Jan  1 00:00:00 2099 GMT").unwrap();
        assert!(days > 20000);
        let past = parse_expiry_days("Jan  1 00:00:00 2000 GMT").unwrap();
        assert!(past < 0);
    }

    #[test]
    fn inventory_well_formed() {
        let inv = inventory();
        assert!(inv.get("certificate_count").is_some());
        assert!(inv.get("ssh_host_keys").is_some());
    }

    #[test]
    fn weak_key_is_algorithm_aware() {
        // ECC P-256 (256-bit) is strong, not weak.
        assert!(!is_weak_key(
            "id-ecPublicKey",
            Some(256),
            "ecdsa-with-SHA256"
        ));
        assert!(!is_weak_key(
            "id-ecPublicKey",
            Some(384),
            "ecdsa-with-SHA384"
        ));
        // RSA below 2048 is weak.
        assert!(is_weak_key(
            "rsaEncryption",
            Some(1024),
            "sha256WithRSAEncryption"
        ));
        // RSA 2048+ is fine.
        assert!(!is_weak_key(
            "rsaEncryption",
            Some(2048),
            "sha256WithRSAEncryption"
        ));
        // SHA-1 signatures are weak regardless of key.
        assert!(is_weak_key(
            "rsaEncryption",
            Some(4096),
            "sha1WithRSAEncryption"
        ));
        // Tiny ECC curve is weak.
        assert!(is_weak_key("id-ecPublicKey", Some(160), "ecdsa-with-SHA1"));
    }

    #[test]
    fn cert_file_extensions() {
        assert!(is_cert_file(Path::new("/etc/ssl/certs/foo.pem")));
        assert!(is_cert_file(Path::new("server.crt")));
        assert!(!is_cert_file(Path::new("server.key")));
    }
}
