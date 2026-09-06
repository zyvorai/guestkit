// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Systemd service analysis

use super::{ServiceDependencies, ServiceInfo, ServiceState, SystemdAnalyzer};
use anyhow::{Context, Result};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// Service analyzer
#[derive(Debug)]
pub struct ServiceAnalyzer {
    analyzer: SystemdAnalyzer,
}

impl ServiceAnalyzer {
    /// Create a new service analyzer
    pub fn new(analyzer: SystemdAnalyzer) -> Self {
        Self { analyzer }
    }

    /// List all available services
    pub fn list_services(&self) -> Result<Vec<ServiceInfo>> {
        let mut services = Vec::new();

        // Check system directory
        if let Ok(entries) = self.scan_service_directory(self.analyzer.systemd_dir()) {
            services.extend(entries);
        }

        // Check runtime directory
        let runtime_dir = self.analyzer.root_path.join("run/systemd/system");
        if let Ok(entries) = self.scan_service_directory(runtime_dir) {
            services.extend(entries);
        }

        // Check lib directory
        let lib_dir = self.analyzer.root_path.join("lib/systemd/system");
        if let Ok(entries) = self.scan_service_directory(lib_dir) {
            services.extend(entries);
        }

        Ok(services)
    }

    /// Scan a directory for service files
    fn scan_service_directory(&self, dir: PathBuf) -> Result<Vec<ServiceInfo>> {
        if !dir.exists() {
            return Ok(Vec::new());
        }

        let mut services = Vec::new();

        for entry in fs::read_dir(&dir)
            .with_context(|| format!("Failed to read systemd directory: {}", dir.display()))?
        {
            let entry = entry?;
            let path = entry.path();

            // Only process .service files
            if let Some(ext) = path.extension() {
                if ext == "service" {
                    if let Ok(service) = self.parse_service_file(&path) {
                        services.push(service);
                    }
                }
            }
        }

        Ok(services)
    }

    /// Parse a service unit file
    fn parse_service_file(&self, path: &Path) -> Result<ServiceInfo> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read service file: {}", path.display()))?;

        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.service")
            .to_string();

        let mut description = None;
        let mut dependencies = ServiceDependencies::default();
        let mut current_section = String::new();

        for line in content.lines() {
            let line = line.trim();

            // Skip comments and empty lines
            if line.starts_with('#') || line.starts_with(';') || line.is_empty() {
                continue;
            }

            // Section headers
            if line.starts_with('[') && line.ends_with(']') {
                current_section = line[1..line.len() - 1].to_string();
                continue;
            }

            // Parse key=value pairs
            if let Some((key, value)) = line.split_once('=') {
                let key = key.trim();
                let value = value.trim();

                match (current_section.as_str(), key) {
                    ("Unit", "Description") => {
                        description = Some(value.to_string());
                    }
                    ("Unit", "Requires") => {
                        dependencies
                            .requires
                            .extend(value.split_whitespace().map(String::from));
                    }
                    ("Unit", "Wants") => {
                        dependencies
                            .wants
                            .extend(value.split_whitespace().map(String::from));
                    }
                    ("Unit", "After") => {
                        dependencies
                            .after
                            .extend(value.split_whitespace().map(String::from));
                    }
                    ("Unit", "Before") => {
                        dependencies
                            .before
                            .extend(value.split_whitespace().map(String::from));
                    }
                    _ => {}
                }
            }
        }

        Ok(ServiceInfo {
            name,
            state: ServiceState::Unknown,
            unit_file: Some(path.to_path_buf()),
            description,
            dependencies,
            enabled: false, // Would need to check symlinks to determine this
            main_pid: None,
        })
    }

    /// Get failed services
    pub fn get_failed_services(&self) -> Result<Vec<ServiceInfo>> {
        let all_services = self.list_services()?;
        Ok(all_services
            .into_iter()
            .filter(|s| s.state == ServiceState::Failed)
            .collect())
    }

    /// Get service dependency tree
    pub fn get_dependency_tree(&self, service_name: &str) -> Result<DependencyTree> {
        let services = self.list_services()?;
        let mut tree = DependencyTree::new(service_name.to_string());
        let mut visited = HashSet::new();

        if let Some(service) = services.iter().find(|s| s.name == service_name) {
            Self::build_dependency_tree(&services, service, &mut tree, &mut visited, 0);
        }

        Ok(tree)
    }

    /// Recursively build dependency tree
    fn build_dependency_tree(
        all_services: &[ServiceInfo],
        service: &ServiceInfo,
        tree: &mut DependencyTree,
        visited: &mut HashSet<String>,
        depth: usize,
    ) {
        if depth > 10 || visited.contains(&service.name) {
            return; // Prevent infinite recursion
        }

        visited.insert(service.name.clone());

        // Add required dependencies
        for req in &service.dependencies.requires {
            if let Some(dep_service) = all_services.iter().find(|s| s.name == *req) {
                let mut dep_tree = DependencyTree::new(req.clone());
                Self::build_dependency_tree(
                    all_services,
                    dep_service,
                    &mut dep_tree,
                    visited,
                    depth + 1,
                );
                tree.dependencies.push(dep_tree);
            }
        }

        // Add wanted dependencies
        for want in &service.dependencies.wants {
            if let Some(dep_service) = all_services.iter().find(|s| s.name == *want) {
                if !visited.contains(want) {
                    let mut dep_tree = DependencyTree::new(want.clone());
                    Self::build_dependency_tree(
                        all_services,
                        dep_service,
                        &mut dep_tree,
                        visited,
                        depth + 1,
                    );
                    tree.dependencies.push(dep_tree);
                }
            }
        }
    }

    /// Generate Mermaid diagram for service dependencies
    pub fn generate_dependency_diagram(&self, service_name: &str) -> Result<String> {
        let tree = self.get_dependency_tree(service_name)?;
        let mut diagram = String::from("```mermaid\ngraph LR\n");

        let mut visited = HashSet::new();
        self.add_tree_to_diagram(&tree, &mut diagram, &mut visited);

        diagram.push_str("```\n");
        Ok(diagram)
    }

    /// Add dependency tree to Mermaid diagram
    fn add_tree_to_diagram(
        &self,
        tree: &DependencyTree,
        diagram: &mut String,
        visited: &mut HashSet<String>,
    ) {
        if visited.contains(&tree.service_name) {
            return;
        }

        visited.insert(tree.service_name.clone());

        for dep in &tree.dependencies {
            diagram.push_str(&format!(
                "    {}[{}] --> {}[{}]\n",
                self.sanitize_node_id(&tree.service_name),
                tree.service_name,
                self.sanitize_node_id(&dep.service_name),
                dep.service_name
            ));

            self.add_tree_to_diagram(dep, diagram, visited);
        }
    }

    /// Sanitize node ID for Mermaid
    fn sanitize_node_id(&self, name: &str) -> String {
        name.replace(['.', '-'], "_")
    }
}

/// Dependency tree structure
#[derive(Debug, Clone)]
pub struct DependencyTree {
    pub service_name: String,
    pub dependencies: Vec<DependencyTree>,
}

impl DependencyTree {
    pub fn new(service_name: String) -> Self {
        Self {
            service_name,
            dependencies: Vec::new(),
        }
    }

    /// Get total dependency count (recursive)
    pub fn count_dependencies(&self) -> usize {
        let mut count = self.dependencies.len();
        for dep in &self.dependencies {
            count += dep.count_dependencies();
        }
        count
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_service_analyzer_creation() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let _service_analyzer = ServiceAnalyzer::new(analyzer);
    }

    #[test]
    fn test_dependency_tree_new() {
        let tree = DependencyTree::new("test.service".to_string());
        assert_eq!(tree.service_name, "test.service");
        assert_eq!(tree.dependencies.len(), 0);
    }

    #[test]
    fn test_dependency_tree_count() {
        let mut tree = DependencyTree::new("main.service".to_string());
        tree.dependencies
            .push(DependencyTree::new("dep1.service".to_string()));
        tree.dependencies
            .push(DependencyTree::new("dep2.service".to_string()));

        assert_eq!(tree.count_dependencies(), 2);
    }

    #[test]
    fn test_sanitize_node_id() {
        let analyzer = SystemdAnalyzer::new("/tmp");
        let service_analyzer = ServiceAnalyzer::new(analyzer);

        assert_eq!(
            service_analyzer.sanitize_node_id("my-service.service"),
            "my_service_service"
        );
        assert_eq!(
            service_analyzer.sanitize_node_id("test.timer"),
            "test_timer"
        );
    }
}
