// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dependency graph analysis and visualization

pub mod analyzer;
pub mod graph;
pub mod visualizer;

use crate::Guestfs;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

/// Dependency graph
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyGraph {
    pub packages: Vec<Package>,
    pub dependencies: Vec<Dependency>,
    pub conflicts: Vec<Conflict>,
    pub circular_dependencies: Vec<CircularDependency>,
    pub statistics: GraphStatistics,
}

/// Package node
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub depends_on: Vec<String>,
    pub required_by: Vec<String>,
    pub is_leaf: bool,
    pub is_root: bool,
    pub depth: usize,
}

/// Dependency edge
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Dependency {
    pub from: String,
    pub to: String,
    pub dependency_type: DependencyType,
    pub is_optional: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DependencyType {
    Required,
    Recommended,
    Suggested,
    Conflicts,
}

/// Dependency conflict
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Conflict {
    pub package1: String,
    pub package2: String,
    pub reason: String,
    pub severity: ConflictSeverity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ConflictSeverity {
    Low,
    Medium,
    High,
    Critical,
}

impl ConflictSeverity {
    pub fn emoji(&self) -> &str {
        match self {
            Self::Low => "🟢",
            Self::Medium => "🟡",
            Self::High => "🟠",
            Self::Critical => "🔴",
        }
    }
}

/// Circular dependency
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CircularDependency {
    pub cycle: Vec<String>,
    pub length: usize,
}

/// Graph statistics
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GraphStatistics {
    pub total_packages: usize,
    pub total_dependencies: usize,
    pub leaf_packages: usize,
    pub root_packages: usize,
    pub max_depth: usize,
    pub circular_dependencies: usize,
    pub conflicts: usize,
    pub average_dependencies: f64,
}

/// Analyze dependencies from disk image
pub fn analyze_dependencies<P: AsRef<Path>>(
    image_path: P,
    verbose: bool,
) -> Result<DependencyGraph> {
    let image_path_str = image_path.as_ref().display().to_string();

    if verbose {
        println!("🔍 Analyzing dependencies: {}", image_path_str);
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

    // Get OS type to determine package manager
    let os_name = g.inspect_get_product_name(root)?;
    let os_lower = os_name.to_lowercase();

    if verbose {
        println!("  OS: {}", os_name);
        println!("  Extracting package information...");
    }

    // Get packages
    let applications = g.inspect_list_applications2(root)?;

    if verbose {
        println!("  Found {} packages", applications.len());
        println!("  Building dependency graph...");
    }

    // Build dependency information
    let (packages, dependencies) = if os_lower.contains("debian") || os_lower.contains("ubuntu") {
        extract_debian_dependencies(&mut g, &applications, verbose)?
    } else if os_lower.contains("red hat")
        || os_lower.contains("centos")
        || os_lower.contains("fedora")
        || os_lower.contains("rocky")
    {
        extract_rpm_dependencies(&mut g, &applications, verbose)?
    } else {
        // Fallback to basic dependency extraction
        extract_basic_dependencies(&applications)
    };

    g.shutdown()?;

    if verbose {
        println!("  Analyzing dependency relationships...");
    }

    // Detect circular dependencies
    let circular_dependencies = analyzer::detect_circular_dependencies(&packages, &dependencies);

    // Detect conflicts
    let conflicts = analyzer::detect_conflicts(&packages, &dependencies);

    // Calculate statistics
    let statistics =
        calculate_statistics(&packages, &dependencies, &circular_dependencies, &conflicts);

    if verbose {
        println!("✅ Dependency analysis complete");
        println!("  Total packages: {}", statistics.total_packages);
        println!("  Total dependencies: {}", statistics.total_dependencies);
        println!(
            "  Circular dependencies: {}",
            statistics.circular_dependencies
        );
        println!("  Conflicts: {}", statistics.conflicts);
    }

    Ok(DependencyGraph {
        packages,
        dependencies,
        conflicts,
        circular_dependencies,
        statistics,
    })
}

fn extract_debian_dependencies(
    g: &mut Guestfs,
    applications: &[(String, String, String)],
    verbose: bool,
) -> Result<(Vec<Package>, Vec<Dependency>)> {
    let mut packages = Vec::new();
    let mut dependencies = Vec::new();
    let mut dep_map: HashMap<String, Vec<String>> = HashMap::new();
    let mut reverse_dep_map: HashMap<String, Vec<String>> = HashMap::new();

    // Try to read dpkg status for dependency info
    let has_dpkg = g.is_file("/var/lib/dpkg/status").unwrap_or(false);

    if has_dpkg {
        if verbose {
            println!("  Reading dpkg dependency information...");
        }

        if let Ok(status_content) = g.cat("/var/lib/dpkg/status") {
            parse_dpkg_status(&status_content, &mut dep_map);
        }
    }

    // Build reverse dependency map
    for (pkg, deps) in &dep_map {
        for dep in deps {
            reverse_dep_map
                .entry(dep.clone())
                .or_default()
                .push(pkg.clone());
        }
    }

    // Create package nodes
    for (name, version, _) in applications {
        let depends_on = dep_map.get(name).cloned().unwrap_or_default();
        let required_by = reverse_dep_map.get(name).cloned().unwrap_or_default();

        let is_leaf = depends_on.is_empty();
        let is_root = required_by.is_empty();

        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
            depends_on: depends_on.clone(),
            required_by: required_by.clone(),
            is_leaf,
            is_root,
            depth: 0,
        });

        // Create dependency edges
        for dep in &depends_on {
            dependencies.push(Dependency {
                from: name.clone(),
                to: dep.clone(),
                dependency_type: DependencyType::Required,
                is_optional: false,
            });
        }
    }

    // Calculate depths
    calculate_depths(&mut packages);

    Ok((packages, dependencies))
}

fn extract_rpm_dependencies(
    _g: &mut Guestfs,
    applications: &[(String, String, String)],
    _verbose: bool,
) -> Result<(Vec<Package>, Vec<Dependency>)> {
    let mut packages = Vec::new();
    let dependencies = Vec::new();
    let dep_map: HashMap<String, Vec<String>> = HashMap::new();

    // For RPM systems, we'd need to query RPM database
    // For now, create basic structure
    for (name, version, _) in applications {
        let depends_on = dep_map.get(name).cloned().unwrap_or_default();

        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
            depends_on: depends_on.clone(),
            required_by: Vec::new(),
            is_leaf: depends_on.is_empty(),
            is_root: true,
            depth: 0,
        });
    }

    Ok((packages, dependencies))
}

fn extract_basic_dependencies(
    applications: &[(String, String, String)],
) -> (Vec<Package>, Vec<Dependency>) {
    let mut packages = Vec::new();
    let dependencies = Vec::new();

    for (name, version, _) in applications {
        packages.push(Package {
            name: name.clone(),
            version: version.clone(),
            depends_on: Vec::new(),
            required_by: Vec::new(),
            is_leaf: true,
            is_root: true,
            depth: 0,
        });
    }

    (packages, dependencies)
}

fn parse_dpkg_status(content: &str, dep_map: &mut HashMap<String, Vec<String>>) {
    let mut current_package = String::new();
    let mut current_deps = Vec::new();

    for line in content.lines() {
        if line.starts_with("Package:") {
            // Save previous package
            if !current_package.is_empty() && !current_deps.is_empty() {
                dep_map.insert(current_package.clone(), current_deps.clone());
            }

            current_package = line
                .strip_prefix("Package:")
                .unwrap_or("")
                .trim()
                .to_string();
            current_deps.clear();
        } else if line.starts_with("Depends:") {
            let deps_str = line.strip_prefix("Depends:").unwrap_or("").trim();
            for dep in deps_str.split(',') {
                let dep_name = dep.split_whitespace().next().unwrap_or("").to_string();
                if !dep_name.is_empty() {
                    current_deps.push(dep_name);
                }
            }
        }
    }

    // Save last package
    if !current_package.is_empty() && !current_deps.is_empty() {
        dep_map.insert(current_package, current_deps);
    }
}

fn calculate_depths(packages: &mut [Package]) {
    // Simple depth calculation using BFS
    let mut visited = HashSet::new();
    let mut queue = Vec::new();

    // Start with leaf nodes (no dependencies)
    for pkg in packages.iter() {
        if pkg.is_leaf {
            queue.push((pkg.name.clone(), 0));
        }
    }

    while let Some((name, depth)) = queue.pop() {
        if visited.contains(&name) {
            continue;
        }
        visited.insert(name.clone());

        // Find package and update depth
        if let Some(pkg) = packages.iter_mut().find(|p| p.name == name) {
            pkg.depth = depth;

            // Add packages that depend on this one
            for req_by in &pkg.required_by.clone() {
                queue.push((req_by.clone(), depth + 1));
            }
        }
    }
}

fn calculate_statistics(
    packages: &[Package],
    dependencies: &[Dependency],
    circular_deps: &[CircularDependency],
    conflicts: &[Conflict],
) -> GraphStatistics {
    let total_packages = packages.len();
    let total_dependencies = dependencies.len();
    let leaf_packages = packages.iter().filter(|p| p.is_leaf).count();
    let root_packages = packages.iter().filter(|p| p.is_root).count();
    let max_depth = packages.iter().map(|p| p.depth).max().unwrap_or(0);

    let average_dependencies = if total_packages > 0 {
        total_dependencies as f64 / total_packages as f64
    } else {
        0.0
    };

    GraphStatistics {
        total_packages,
        total_dependencies,
        leaf_packages,
        root_packages,
        max_depth,
        circular_dependencies: circular_deps.len(),
        conflicts: conflicts.len(),
        average_dependencies,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_package_creation() {
        let package = Package {
            name: "test-pkg".to_string(),
            version: "1.0.0".to_string(),
            depends_on: vec!["dep1".to_string()],
            required_by: vec!["parent1".to_string()],
            is_leaf: false,
            is_root: false,
            depth: 1,
        };

        assert_eq!(package.name, "test-pkg");
        assert_eq!(package.version, "1.0.0");
        assert_eq!(package.depth, 1);
        assert!(!package.is_leaf);
        assert!(!package.is_root);
    }

    #[test]
    fn test_dependency_creation() {
        let dependency = Dependency {
            from: "pkg-a".to_string(),
            to: "pkg-b".to_string(),
            dependency_type: DependencyType::Required,
            is_optional: false,
        };

        assert_eq!(dependency.from, "pkg-a");
        assert_eq!(dependency.to, "pkg-b");
        assert!(!dependency.is_optional);
    }

    #[test]
    fn test_graph_statistics() {
        let stats = GraphStatistics {
            total_packages: 10,
            total_dependencies: 15,
            leaf_packages: 3,
            root_packages: 2,
            max_depth: 5,
            circular_dependencies: 0,
            conflicts: 0,
            average_dependencies: 1.5,
        };

        assert_eq!(stats.total_packages, 10);
        assert_eq!(stats.total_dependencies, 15);
        assert_eq!(stats.circular_dependencies, 0);
        assert_eq!(stats.average_dependencies, 1.5);
    }

    #[test]
    fn test_circular_dependency() {
        let circular = CircularDependency {
            cycle: vec![
                "pkg-a".to_string(),
                "pkg-b".to_string(),
                "pkg-a".to_string(),
            ],
            length: 3,
        };

        assert_eq!(circular.cycle.len(), 3);
        assert_eq!(circular.length, 3);
    }

    #[test]
    fn test_dependency_graph_creation() {
        let graph = DependencyGraph {
            packages: vec![],
            dependencies: vec![],
            conflicts: vec![],
            circular_dependencies: vec![],
            statistics: GraphStatistics {
                total_packages: 0,
                total_dependencies: 0,
                leaf_packages: 0,
                root_packages: 0,
                max_depth: 0,
                circular_dependencies: 0,
                conflicts: 0,
                average_dependencies: 0.0,
            },
        };

        assert_eq!(graph.packages.len(), 0);
        assert_eq!(graph.dependencies.len(), 0);
        assert_eq!(graph.statistics.total_packages, 0);
    }
}
