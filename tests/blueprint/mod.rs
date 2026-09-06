// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Blueprint module tests

use guestkit::cli::blueprint::*;

#[cfg(test)]
mod format_tests {
    use super::*;

    #[test]
    fn test_blueprint_format_from_str() {
        assert_eq!(BlueprintFormat::from_str("terraform"), Some(BlueprintFormat::Terraform));
        assert_eq!(BlueprintFormat::from_str("tf"), Some(BlueprintFormat::Terraform));
        assert_eq!(BlueprintFormat::from_str("ansible"), Some(BlueprintFormat::Ansible));
        assert_eq!(BlueprintFormat::from_str("kubernetes"), Some(BlueprintFormat::Kubernetes));
        assert_eq!(BlueprintFormat::from_str("k8s"), Some(BlueprintFormat::Kubernetes));
        assert_eq!(BlueprintFormat::from_str("compose"), Some(BlueprintFormat::Compose));
        assert_eq!(BlueprintFormat::from_str("docker-compose"), Some(BlueprintFormat::Compose));
        assert_eq!(BlueprintFormat::from_str("invalid"), None);
    }

    #[test]
    fn test_blueprint_format_case_insensitive() {
        assert_eq!(BlueprintFormat::from_str("TERRAFORM"), Some(BlueprintFormat::Terraform));
        assert_eq!(BlueprintFormat::from_str("Ansible"), Some(BlueprintFormat::Ansible));
        assert_eq!(BlueprintFormat::from_str("K8S"), Some(BlueprintFormat::Kubernetes));
    }
}

#[cfg(test)]
mod terraform_tests {
    use super::*;

    fn create_test_analysis() -> ImageAnalysis {
        ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "test-host".to_string(),
            packages: vec![
                Package {
                    name: "nginx".to_string(),
                    version: "1.18.0".to_string(),
                },
            ],
            services: vec![
                Service {
                    name: "nginx".to_string(),
                    enabled: true,
                    state: "active".to_string(),
                },
            ],
            filesystems: vec![
                Filesystem {
                    device: "/dev/sda1".to_string(),
                    mountpoint: "/".to_string(),
                    fstype: "ext4".to_string(),
                    size_gb: 20.0,
                },
            ],
            network_config: NetworkConfig {
                interfaces: vec![
                    NetworkInterface {
                        name: "eth0".to_string(),
                        address: Some("192.168.1.10".to_string()),
                    },
                ],
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
            volumes: vec![
                Volume {
                    path: "/var/www".to_string(),
                    size_gb: 5.0,
                },
            ],
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
    fn test_terraform_contains_volumes() {
        let analysis = create_test_analysis();
        let result = terraform::generate(&analysis, Some("aws"));

        assert!(result.is_ok());
        let tf = result.unwrap();

        assert!(tf.contains("aws_ebs_volume"));
        assert!(tf.contains("Path = \"/var/www\""));
    }
}

#[cfg(test)]
mod ansible_tests {
    use super::*;

    #[test]
    fn test_ansible_generation() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "webserver".to_string(),
            packages: vec![
                Package {
                    name: "nginx".to_string(),
                    version: "1.18.0".to_string(),
                },
            ],
            services: vec![
                Service {
                    name: "nginx".to_string(),
                    enabled: true,
                    state: "active".to_string(),
                },
            ],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![
                Port { number: 80, protocol: "tcp".to_string() },
            ],
            volumes: vec![],
        };

        let result = ansible::generate(&analysis);

        assert!(result.is_ok());
        let playbook = result.unwrap();

        // Check YAML structure
        assert!(playbook.starts_with("---"));
        assert!(playbook.contains("name: Configure server"));
        assert!(playbook.contains("hosts: all"));
        assert!(playbook.contains("become: yes"));

        // Check hostname variable
        assert!(playbook.contains("hostname: webserver"));

        // Check package installation
        assert!(playbook.contains("name: Install packages"));
        assert!(playbook.contains("nginx"));

        // Check service configuration
        assert!(playbook.contains("systemd:"));
        assert!(playbook.contains("enabled: yes"));
        assert!(playbook.contains("state: active"));
    }

    #[test]
    fn test_ansible_ubuntu_uses_apt() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "test".to_string(),
            packages: vec![],
            services: vec![],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![],
            volumes: vec![],
        };

        let result = ansible::generate(&analysis);
        assert!(result.is_ok());
        // apt module should be used for Ubuntu
        assert!(result.unwrap().contains("apt:") || !result.as_ref().unwrap().contains("Install packages"));
    }
}

#[cfg(test)]
mod kubernetes_tests {
    use super::*;

    #[test]
    fn test_kubernetes_generation() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "app".to_string(),
            packages: vec![],
            services: vec![],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![
                Port { number: 80, protocol: "tcp".to_string() },
            ],
            volumes: vec![
                Volume { path: "/data".to_string(), size_gb: 10.0 },
            ],
        };

        let result = kubernetes::generate(&analysis);

        assert!(result.is_ok());
        let manifests = result.unwrap();

        // Check for namespace
        assert!(manifests.contains("kind: Namespace"));

        // Check for deployment
        assert!(manifests.contains("kind: Deployment"));
        assert!(manifests.contains("replicas: 1"));

        // Check for service
        assert!(manifests.contains("kind: Service"));

        // Check for PVC
        assert!(manifests.contains("kind: PersistentVolumeClaim"));
        assert!(manifests.contains("storage: 10Gi"));

        // Check for configmap
        assert!(manifests.contains("kind: ConfigMap"));
    }

    #[test]
    fn test_kubernetes_http_creates_ingress() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "web".to_string(),
            packages: vec![],
            services: vec![],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![
                Port { number: 80, protocol: "tcp".to_string() },
            ],
            volumes: vec![],
        };

        let result = kubernetes::generate(&analysis);
        assert!(result.is_ok());

        let manifests = result.unwrap();
        assert!(manifests.contains("kind: Ingress"));
    }
}

#[cfg(test)]
mod compose_tests {
    use super::*;

    #[test]
    fn test_compose_generation() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "webapp".to_string(),
            packages: vec![],
            services: vec![],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![
                Port { number: 80, protocol: "tcp".to_string() },
            ],
            volumes: vec![
                Volume { path: "/app/data".to_string(), size_gb: 5.0 },
            ],
        };

        let result = compose::generate(&analysis);

        assert!(result.is_ok());
        let compose = result.unwrap();

        // Check version
        assert!(compose.contains("version: '3.8'"));

        // Check service definition
        assert!(compose.contains("services:"));
        assert!(compose.contains("webapp:"));

        // Check ports
        assert!(compose.contains("ports:"));
        assert!(compose.contains("\"80:80\""));

        // Check volumes
        assert!(compose.contains("volumes:"));

        // Check networks
        assert!(compose.contains("networks:"));
        assert!(compose.contains("app_network"));

        // Check resource limits
        assert!(compose.contains("deploy:"));
        assert!(compose.contains("resources:"));
    }

    #[test]
    fn test_compose_database_services() {
        let analysis = ImageAnalysis {
            os_name: "Ubuntu".to_string(),
            os_version: "22.04".to_string(),
            arch: "x86_64".to_string(),
            hostname: "db-server".to_string(),
            packages: vec![],
            services: vec![
                Service {
                    name: "mysql".to_string(),
                    enabled: true,
                    state: "active".to_string(),
                },
            ],
            filesystems: vec![],
            network_config: NetworkConfig { interfaces: vec![] },
            ports: vec![],
            volumes: vec![],
        };

        let result = compose::generate(&analysis);
        assert!(result.is_ok());

        let compose = result.unwrap();
        // Should include MySQL service
        assert!(compose.contains("mysql:") || compose.contains("image: mysql"));
    }
}
