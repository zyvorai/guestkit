// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dependency analysis algorithms

use super::*;
use std::collections::{HashMap, HashSet};

/// Detect circular dependencies
pub fn detect_circular_dependencies(
    packages: &[Package],
    dependencies: &[Dependency],
) -> Vec<CircularDependency> {
    let mut circular_deps = Vec::new();
    let mut visited = HashSet::new();
    let mut rec_stack = HashSet::new();

    // Build adjacency list
    let mut adj_list: HashMap<String, Vec<String>> = HashMap::new();
    for dep in dependencies {
        adj_list
            .entry(dep.from.clone())
            .or_default()
            .push(dep.to.clone());
    }

    // DFS to detect cycles
    for pkg in packages {
        if !visited.contains(&pkg.name) {
            let mut path = Vec::new();
            if has_cycle(
                &pkg.name,
                &adj_list,
                &mut visited,
                &mut rec_stack,
                &mut path,
            ) {
                // Extract the cycle from the path
                if let Some(cycle_start) = path.iter().rposition(|n| n == &pkg.name) {
                    let cycle: Vec<String> = path[cycle_start..].to_vec();
                    let length = cycle.len();

                    // Check if this cycle is already recorded
                    let cycle_set: HashSet<&String> = cycle.iter().collect();
                    let is_duplicate = circular_deps.iter().any(|cd: &CircularDependency| {
                        let existing_set: HashSet<&String> = cd.cycle.iter().collect();
                        cycle_set == existing_set
                    });

                    if !is_duplicate {
                        circular_deps.push(CircularDependency { cycle, length });
                    }
                }
            }
        }
    }

    circular_deps
}

fn has_cycle(
    node: &str,
    adj_list: &HashMap<String, Vec<String>>,
    visited: &mut HashSet<String>,
    rec_stack: &mut HashSet<String>,
    path: &mut Vec<String>,
) -> bool {
    visited.insert(node.to_string());
    rec_stack.insert(node.to_string());
    path.push(node.to_string());

    if let Some(neighbors) = adj_list.get(node) {
        for neighbor in neighbors {
            if !visited.contains(neighbor) {
                if has_cycle(neighbor, adj_list, visited, rec_stack, path) {
                    return true;
                }
            } else if rec_stack.contains(neighbor) {
                path.push(neighbor.to_string());
                return true;
            }
        }
    }

    rec_stack.remove(node);
    path.pop();
    false
}

/// Detect dependency conflicts
pub fn detect_conflicts(packages: &[Package], dependencies: &[Dependency]) -> Vec<Conflict> {
    let mut conflicts = Vec::new();

    // Check for version conflicts
    let mut version_map: HashMap<String, Vec<String>> = HashMap::new();
    for pkg in packages {
        let base_name = pkg.name.split(':').next().unwrap_or(&pkg.name);
        version_map
            .entry(base_name.to_string())
            .or_default()
            .push(pkg.version.clone());
    }

    for (name, versions) in version_map {
        if versions.len() > 1 {
            conflicts.push(Conflict {
                package1: format!("{}:{}", name, versions[0]),
                package2: format!("{}:{}", name, versions[1]),
                reason: "Multiple versions of the same package installed".to_string(),
                severity: ConflictSeverity::Medium,
            });
        }
    }

    // Check for explicit conflicts
    for dep in dependencies {
        if dep.dependency_type == DependencyType::Conflicts {
            conflicts.push(Conflict {
                package1: dep.from.clone(),
                package2: dep.to.clone(),
                reason: "Packages explicitly conflict".to_string(),
                severity: ConflictSeverity::High,
            });
        }
    }

    // Check for missing dependencies
    let package_names: HashSet<_> = packages.iter().map(|p| &p.name).collect();
    for dep in dependencies {
        if dep.dependency_type == DependencyType::Required && !package_names.contains(&dep.to) {
            conflicts.push(Conflict {
                package1: dep.from.clone(),
                package2: dep.to.clone(),
                reason: "Required dependency not installed".to_string(),
                severity: ConflictSeverity::Critical,
            });
        }
    }

    conflicts
}

/// Find packages on critical path to a target
pub fn find_critical_path(
    _packages: &[Package],
    dependencies: &[Dependency],
    target: &str,
) -> Vec<String> {
    let mut path = Vec::new();
    let mut visited = HashSet::new();

    // Build reverse adjacency list (who depends on whom)
    let mut reverse_adj: HashMap<String, Vec<String>> = HashMap::new();
    for dep in dependencies {
        reverse_adj
            .entry(dep.to.clone())
            .or_default()
            .push(dep.from.clone());
    }

    // BFS from target backwards
    let mut queue = vec![target.to_string()];
    visited.insert(target.to_string());

    while let Some(node) = queue.pop() {
        path.push(node.clone());

        if let Some(dependents) = reverse_adj.get(&node) {
            for dep in dependents {
                if !visited.contains(dep) {
                    visited.insert(dep.clone());
                    queue.push(dep.clone());
                }
            }
        }
    }

    path
}

/// Calculate package importance based on dependents
pub fn calculate_importance(packages: &[Package]) -> HashMap<String, f64> {
    let mut importance = HashMap::new();
    let total = (packages.len().max(1)) as f64;

    for pkg in packages {
        // Importance based on number of packages that depend on this
        let score = (pkg.required_by.len() as f64 / total) * 100.0;
        importance.insert(pkg.name.clone(), score);
    }

    importance
}

/// Find leaf packages (no dependencies)
pub fn find_leaf_packages(packages: &[Package]) -> Vec<String> {
    packages
        .iter()
        .filter(|p| p.is_leaf)
        .map(|p| p.name.clone())
        .collect()
}

/// Find root packages (nothing depends on them)
pub fn find_root_packages(packages: &[Package]) -> Vec<String> {
    packages
        .iter()
        .filter(|p| p.is_root)
        .map(|p| p.name.clone())
        .collect()
}

/// Find most depended-upon packages
pub fn find_most_depended(packages: &[Package], limit: usize) -> Vec<(String, usize)> {
    let mut pkg_deps: Vec<_> = packages
        .iter()
        .map(|p| (p.name.clone(), p.required_by.len()))
        .collect();

    pkg_deps.sort_by_key(|b| std::cmp::Reverse(b.1));
    pkg_deps.truncate(limit);
    pkg_deps
}
