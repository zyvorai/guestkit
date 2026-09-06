// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Infrastructure as Code blueprint generation

pub mod ansible;
pub mod compose;
pub mod kubernetes;
pub mod terraform;

use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Blueprint format type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlueprintFormat {
    Terraform,
    Ansible,
    Kubernetes,
    Compose,
}

impl BlueprintFormat {
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "terraform" | "tf" => Some(Self::Terraform),
            "ansible" => Some(Self::Ansible),
            "kubernetes" | "k8s" => Some(Self::Kubernetes),
            "compose" | "docker-compose" => Some(Self::Compose),
            _ => None,
        }
    }
}

/// Image analysis data for blueprint generation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAnalysis {
    pub os_name: String,
    pub os_version: String,
    pub arch: String,
    pub hostname: String,
    pub packages: Vec<Package>,
    pub services: Vec<Service>,
    pub filesystems: Vec<Filesystem>,
    pub network_config: NetworkConfig,
    pub ports: Vec<Port>,
    pub volumes: Vec<Volume>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Service {
    pub name: String,
    pub enabled: bool,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Filesystem {
    pub device: String,
    pub mountpoint: String,
    pub fstype: String,
    pub size_gb: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub interfaces: Vec<NetworkInterface>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    pub name: String,
    pub address: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Port {
    pub number: u16,
    pub protocol: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub path: String,
    pub size_gb: f64,
}

/// Analyze disk image for blueprint generation
pub fn analyze_image<P: AsRef<Path>>(image_path: P, verbose: bool) -> Result<ImageAnalysis> {
    let image_path_str = image_path.as_ref().display().to_string();

    if verbose {
        println!("📋 Analyzing image for blueprint: {}", image_path_str);
    }

    // Initialize guestfs
    let mut g = Guestfs::new()?;
    g.add_drive_opts(&image_path, true, None)?;
    g.launch()?;

    // Inspect OS
    let roots = g.inspect_os()?;
    if roots.is_empty() {
        anyhow::bail!("No operating systems found in disk image");
    }

    let root = &roots[0];

    // Mount filesystems
    let mountpoints = g.inspect_get_mountpoints(root)?;
    for (mp, dev) in mountpoints {
        let _ = g.mount(&dev, &mp);
    }

    // Get OS information
    let os_name = g
        .inspect_get_product_name(root)
        .unwrap_or_else(|_| "Unknown".to_string());
    let os_version = g.inspect_get_major_version(root).unwrap_or(0);
    let os_minor = g.inspect_get_minor_version(root).unwrap_or(0);
    let arch = g
        .inspect_get_arch(root)
        .unwrap_or_else(|_| "x86_64".to_string());

    let hostname = g
        .inspect_get_hostname(root)
        .unwrap_or_else(|_| "unknown".to_string());

    // Get packages
    let mut packages = Vec::new();
    let applications = g.inspect_list_applications2(root)?;
    for (name, version, _release) in applications.iter().take(20) {
        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
        });
    }

    // Get services (detect systemd/init.d)
    let services = detect_services(&mut g, verbose);

    // Get filesystems
    let filesystems = detect_filesystems(&mut g, verbose);

    // Detect exposed ports
    let ports = detect_ports(&mut g, verbose);

    // Detect volumes
    let volumes = detect_volumes(&mut g, verbose);

    // Network config
    let network_config = NetworkConfig {
        interfaces: vec![NetworkInterface {
            name: "eth0".to_string(),
            address: None,
        }],
    };

    g.shutdown()?;

    Ok(ImageAnalysis {
        os_name,
        os_version: format!("{}.{}", os_version, os_minor),
        arch,
        hostname,
        packages,
        services,
        filesystems,
        network_config,
        ports,
        volumes,
    })
}

fn detect_services(g: &mut Guestfs, verbose: bool) -> Vec<Service> {
    let mut services = Vec::new();

    // Check for systemd services
    if g.is_file("/etc/systemd/system").unwrap_or(false)
        || g.is_dir("/lib/systemd/system").unwrap_or(false)
    {
        if verbose {
            println!("  Detecting systemd services...");
        }

        // Common services to check
        for service_name in &[
            "nginx",
            "apache2",
            "httpd",
            "mysql",
            "mariadb",
            "postgresql",
            "redis",
            "docker",
        ] {
            let service_file = format!("/lib/systemd/system/{}.service", service_name);
            if g.is_file(&service_file).unwrap_or(false) {
                // Check if service is enabled by looking for symlink in wants directories
                let enabled_link = format!(
                    "/etc/systemd/system/multi-user.target.wants/{}.service",
                    service_name
                );
                let is_enabled = g.is_file(&enabled_link).unwrap_or(false);

                services.push(Service {
                    name: service_name.to_string(),
                    enabled: is_enabled,
                    state: "installed".to_string(),
                });
            }
        }
    }

    services
}

fn detect_filesystems(g: &mut Guestfs, _verbose: bool) -> Vec<Filesystem> {
    let mut filesystems = Vec::new();

    if let Ok(list) = g.list_filesystems() {
        for (device, fstype) in list {
            if fstype != "unknown" && !fstype.is_empty() {
                let size_bytes = g.blockdev_getsize64(&device).unwrap_or(0);
                let size_gb = size_bytes as f64 / 1_073_741_824.0;

                filesystems.push(Filesystem {
                    device,
                    mountpoint: "/".to_string(),
                    fstype,
                    size_gb,
                });
            }
        }
    }

    filesystems
}

fn detect_ports(g: &mut Guestfs, verbose: bool) -> Vec<Port> {
    let mut ports = Vec::new();

    if verbose {
        println!("  Detecting exposed ports...");
    }

    // Check for common web servers
    if g.is_file("/etc/nginx/nginx.conf").unwrap_or(false)
        || g.is_file("/etc/apache2/apache2.conf").unwrap_or(false)
        || g.is_file("/etc/httpd/conf/httpd.conf").unwrap_or(false)
    {
        ports.push(Port {
            number: 80,
            protocol: "tcp".to_string(),
        });
        ports.push(Port {
            number: 443,
            protocol: "tcp".to_string(),
        });
    }

    // Check for databases
    if g.is_dir("/var/lib/mysql").unwrap_or(false) {
        ports.push(Port {
            number: 3306,
            protocol: "tcp".to_string(),
        });
    }
    if g.is_dir("/var/lib/postgresql").unwrap_or(false) {
        ports.push(Port {
            number: 5432,
            protocol: "tcp".to_string(),
        });
    }
    if g.is_file("/etc/redis/redis.conf").unwrap_or(false) {
        ports.push(Port {
            number: 6379,
            protocol: "tcp".to_string(),
        });
    }

    // SSH is almost always present
    if g.is_file("/etc/ssh/sshd_config").unwrap_or(false) {
        ports.push(Port {
            number: 22,
            protocol: "tcp".to_string(),
        });
    }

    ports
}

fn detect_volumes(g: &mut Guestfs, verbose: bool) -> Vec<Volume> {
    let mut volumes = Vec::new();

    if verbose {
        println!("  Detecting data volumes...");
    }

    // Common data directories
    let data_dirs = vec![
        "/var/lib/mysql",
        "/var/lib/postgresql",
        "/var/lib/redis",
        "/var/www",
        "/opt",
        "/srv",
    ];

    for dir in data_dirs {
        if g.is_dir(dir).unwrap_or(false) {
            let size_bytes = g.du(dir).unwrap_or(0);
            let size_gb = size_bytes as f64 / 1_073_741_824.0;

            if size_gb > 0.1 {
                // Only include if > 100MB
                volumes.push(Volume {
                    path: dir.to_string(),
                    size_gb,
                });
            }
        }
    }

    volumes
}

/// Generate blueprint in specified format
pub fn generate_blueprint(
    analysis: &ImageAnalysis,
    format: BlueprintFormat,
    provider: Option<&str>,
) -> Result<String> {
    match format {
        BlueprintFormat::Terraform => terraform::generate(analysis, provider),
        BlueprintFormat::Ansible => ansible::generate(analysis),
        BlueprintFormat::Kubernetes => kubernetes::generate(analysis),
        BlueprintFormat::Compose => compose::generate(analysis),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_blueprint_format_from_str() {
        assert_eq!(
            BlueprintFormat::from_str("terraform"),
            Some(BlueprintFormat::Terraform)
        );
        assert_eq!(
            BlueprintFormat::from_str("tf"),
            Some(BlueprintFormat::Terraform)
        );
        assert_eq!(
            BlueprintFormat::from_str("ansible"),
            Some(BlueprintFormat::Ansible)
        );
        assert_eq!(
            BlueprintFormat::from_str("kubernetes"),
            Some(BlueprintFormat::Kubernetes)
        );
        assert_eq!(
            BlueprintFormat::from_str("k8s"),
            Some(BlueprintFormat::Kubernetes)
        );
        assert_eq!(
            BlueprintFormat::from_str("compose"),
            Some(BlueprintFormat::Compose)
        );
        assert_eq!(
            BlueprintFormat::from_str("docker-compose"),
            Some(BlueprintFormat::Compose)
        );
        assert_eq!(BlueprintFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_blueprint_format_case_insensitive() {
        assert_eq!(
            BlueprintFormat::from_str("TERRAFORM"),
            Some(BlueprintFormat::Terraform)
        );
        assert_eq!(
            BlueprintFormat::from_str("Ansible"),
            Some(BlueprintFormat::Ansible)
        );
        assert_eq!(
            BlueprintFormat::from_str("K8S"),
            Some(BlueprintFormat::Kubernetes)
        );
    }

    fn create_test_analysis() -> ImageAnalysis {
        ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "test-host".to_string(),
            packages: vec![Package {
                name: "nginx".to_string(),
                version: "1.18.0".to_string(),
            }],
            services: vec![Service {
                name: "nginx".to_string(),
                enabled: true,
                state: "active".to_string(),
            }],
            filesystems: vec![Filesystem {
                device: "/dev/sda1".to_string(),
                mountpoint: "/".to_string(),
                fstype: "ext4".to_string(),
                size_gb: 20.0,
            }],
            network_config: NetworkConfig {
                interfaces: vec![NetworkInterface {
                    name: "eth0".to_string(),
                    address: Some("192.168.1.10".to_string()),
                }],
            },
            ports: vec![
                Port {
                    number: 80,
                    protocol: "tcp".to_string(),
                },
                Port {
                    number: 443,
                    protocol: "tcp".to_string(),
                },
            ],
            volumes: vec![Volume {
                path: "/var/www".to_string(),
                size_gb: 5.0,
            }],
        }
    }

    #[test]
    fn test_terraform_generation_aws() {
        let analysis = create_test_analysis();
        let result = terraform::generate(&analysis, Some("aws"));

        assert!(result.is_ok());
        let tf = result.unwrap();

        // Check for provider configuration
        assert!(tf.contains("provider \"aws\""));
        assert!(tf.contains("region = var.region"));

        // Check for security group
        assert!(tf.contains("resource \"aws_security_group\""));

        // Check for HTTP/HTTPS ports
        assert!(tf.contains("from_port   = 80"));
        assert!(tf.contains("from_port   = 443"));
    }

    #[test]
    fn test_terraform_generation_azure() {
        let analysis = create_test_analysis();
        let result = terraform::generate(&analysis, Some("azure"));

        assert!(result.is_ok());
        let tf = result.unwrap();

        assert!(tf.contains("provider \"azurerm\""));
        assert!(tf.contains("resource_group"));
    }

    #[test]
    fn test_terraform_generation_gcp() {
        let analysis = create_test_analysis();
        let result = terraform::generate(&analysis, Some("gcp"));

        assert!(result.is_ok());
        let tf = result.unwrap();

        assert!(tf.contains("provider \"google\""));
        assert!(tf.contains("compute_instance"));
    }

    #[test]
    fn test_ansible_generation() {
        let analysis = create_test_analysis();
        let result = ansible::generate(&analysis);

        assert!(result.is_ok());
        let playbook = result.unwrap();

        // Check YAML structure
        assert!(playbook.starts_with("---"));
        assert!(playbook.contains("name: Configure server"));
        assert!(playbook.contains("hosts: all"));
        assert!(playbook.contains("become: yes"));
    }

    #[test]
    fn test_kubernetes_generation() {
        let analysis = create_test_analysis();
        let result = kubernetes::generate(&analysis);

        assert!(result.is_ok());
        let manifests = result.unwrap();

        // Check for namespace
        assert!(manifests.contains("kind: Namespace"));

        // Check for deployment
        assert!(manifests.contains("kind: Deployment"));

        // Check for service
        assert!(manifests.contains("kind: Service"));
    }

    #[test]
    fn test_compose_generation() {
        let analysis = create_test_analysis();
        let result = compose::generate(&analysis);

        assert!(result.is_ok());
        let compose = result.unwrap();

        // Check version
        assert!(compose.contains("version: '3.8'"));

        // Check service definition
        assert!(compose.contains("services:"));

        // Check ports
        assert!(compose.contains("ports:"));
    }
}
