// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! SSH and certificate operations for disk image manipulation
//!
//! This implementation provides SSH key and certificate management.

use crate::core::{Error, Result};
use crate::guestfs::Guestfs;

impl Guestfs {
    /// Get SSH host public keys
    ///
    pub fn get_ssh_host_keys(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_ssh_host_keys");
        }

        let mut keys = Vec::new();

        // Check common SSH host key locations
        let key_types = vec!["rsa", "dsa", "ecdsa", "ed25519"];

        for key_type in key_types {
            let key_path = format!("/etc/ssh/ssh_host_{}_key.pub", key_type);
            if self.exists(&key_path)? {
                if let Ok(key_content) = self.cat(&key_path) {
                    keys.push(format!("{}: {}", key_type, key_content.trim()));
                }
            }
        }

        Ok(keys)
    }

    /// List SSH authorized keys for user
    ///
    pub fn get_ssh_authorized_keys(&mut self, user: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_ssh_authorized_keys {}", user);
        }

        let mut keys = Vec::new();

        // Construct path to authorized_keys
        let auth_keys_path = if user == "root" {
            "/root/.ssh/authorized_keys".to_string()
        } else {
            format!("/home/{}/.ssh/authorized_keys", user)
        };

        if self.exists(&auth_keys_path)? {
            let content = self.cat(&auth_keys_path)?;
            for line in content.lines() {
                let line = line.trim();
                if !line.is_empty() && !line.starts_with('#') {
                    keys.push(line.to_string());
                }
            }
        }

        Ok(keys)
    }

    /// Set SSH authorized keys for user
    ///
    pub fn set_ssh_authorized_keys(&mut self, user: &str, keys: &[&str]) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_ssh_authorized_keys {}", user);
        }

        // Construct paths
        let ssh_dir = if user == "root" {
            "/root/.ssh".to_string()
        } else {
            format!("/home/{}/.ssh", user)
        };

        let auth_keys_path = format!("{}/authorized_keys", ssh_dir);

        // Create .ssh directory if it doesn't exist
        if !self.exists(&ssh_dir)? {
            self.mkdir_p(&ssh_dir)?;
            self.chmod(0o700, &ssh_dir)?;
        }

        // Write keys
        let keys_content = keys.join("\n") + "\n";
        self.write(&auth_keys_path, keys_content.as_bytes())?;
        self.chmod(0o600, &auth_keys_path)?;

        Ok(())
    }

    /// Get SSH server configuration
    ///
    pub fn get_sshd_config(&mut self) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_sshd_config");
        }

        self.cat("/etc/ssh/sshd_config")
    }

    /// List SSL certificates
    ///
    pub fn list_ssl_certificates(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_ssl_certificates");
        }

        let mut certs = Vec::new();

        // Check common certificate locations
        let cert_paths = vec![
            "/etc/ssl/certs",
            "/etc/pki/tls/certs",
            "/etc/pki/ca-trust/source/anchors",
        ];

        for cert_dir in cert_paths {
            if self.exists(cert_dir)? {
                if let Ok(entries) = self.ls(cert_dir) {
                    for entry in entries {
                        if entry.ends_with(".crt") || entry.ends_with(".pem") {
                            certs.push(format!("{}/{}", cert_dir, entry));
                        }
                    }
                }
            }
        }

        Ok(certs)
    }

    /// Get certificate info
    ///
    pub fn get_certificate_info(&mut self, cert_path: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_certificate_info {}", cert_path);
        }

        let host_path = self.resolve_guest_path(cert_path)?;

        let output = std::process::Command::new("openssl")
            .arg("x509")
            .arg("-in")
            .arg(&host_path)
            .arg("-noout")
            .arg("-text")
            .output()
            .map_err(|e| Error::CommandFailed(format!("Failed to execute openssl: {}", e)))?;

        if !output.status.success() {
            return Err(Error::CommandFailed(format!(
                "OpenSSL failed: {}",
                String::from_utf8_lossy(&output.stderr)
            )));
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// List private keys
    ///
    pub fn list_private_keys(&mut self) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_private_keys");
        }

        let mut keys = Vec::new();

        // Check SSH keys
        if self.exists("/etc/ssh")? {
            if let Ok(entries) = self.ls("/etc/ssh") {
                for entry in entries {
                    if entry.contains("_key") && !entry.ends_with(".pub") {
                        keys.push(format!("/etc/ssh/{}", entry));
                    }
                }
            }
        }

        // Check SSL keys
        let key_paths = vec!["/etc/ssl/private", "/etc/pki/tls/private"];

        for key_dir in key_paths {
            if self.exists(key_dir)? {
                if let Ok(entries) = self.ls(key_dir) {
                    for entry in entries {
                        if entry.ends_with(".key") || entry.ends_with(".pem") {
                            keys.push(format!("{}/{}", key_dir, entry));
                        }
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Get user SSH keys
    ///
    pub fn list_user_ssh_keys(&mut self, user: &str) -> Result<Vec<String>> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: list_user_ssh_keys {}", user);
        }

        let mut keys = Vec::new();

        let ssh_dir = if user == "root" {
            "/root/.ssh".to_string()
        } else {
            format!("/home/{}/.ssh", user)
        };

        if self.exists(&ssh_dir)? {
            if let Ok(entries) = self.ls(&ssh_dir) {
                for entry in entries {
                    // List both public and private keys
                    if entry.starts_with("id_") || entry.ends_with(".pub") {
                        keys.push(format!("{}/{}", ssh_dir, entry));
                    }
                }
            }
        }

        Ok(keys)
    }

    /// Get known hosts
    ///
    pub fn get_known_hosts(&mut self, user: &str) -> Result<String> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: get_known_hosts {}", user);
        }

        let known_hosts_path = if user == "root" {
            "/root/.ssh/known_hosts".to_string()
        } else {
            format!("/home/{}/.ssh/known_hosts", user)
        };

        if self.exists(&known_hosts_path)? {
            self.cat(&known_hosts_path)
        } else {
            Ok(String::new())
        }
    }

    /// Set known hosts
    ///
    pub fn set_known_hosts(&mut self, user: &str, content: &str) -> Result<()> {
        self.ensure_ready()?;

        if self.verbose {
            eprintln!("guestfs: set_known_hosts {}", user);
        }

        let ssh_dir = if user == "root" {
            "/root/.ssh".to_string()
        } else {
            format!("/home/{}/.ssh", user)
        };

        let known_hosts_path = format!("{}/known_hosts", ssh_dir);

        // Create .ssh directory if needed
        if !self.exists(&ssh_dir)? {
            self.mkdir_p(&ssh_dir)?;
            self.chmod(0o700, &ssh_dir)?;
        }

        self.write(&known_hosts_path, content.as_bytes())?;
        self.chmod(0o600, &known_hosts_path)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_api_exists() {
        let _g = Guestfs::new().unwrap();
        // API structure tests
    }
}
