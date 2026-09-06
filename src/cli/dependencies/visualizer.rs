// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dependency visualization and reporting

use super::*;

/// Format dependency graph as text report
pub fn format_report(graph: &DependencyGraph, detailed: bool) -> String {
    let mut output = String::new();

    // Header
    output.push_str("📊 Dependency Graph Analysis\n");
    output.push_str("============================\n\n");

    // Statistics
    output.push_str("📈 Statistics\n");
    output.push_str("-------------\n");
    output.push_str(&format!(
        "Total Packages: {}\n",
        graph.statistics.total_packages
    ));
    output.push_str(&format!(
        "Total Dependencies: {}\n",
        graph.statistics.total_dependencies
    ));
    output.push_str(&format!(
        "Leaf Packages: {}\n",
        graph.statistics.leaf_packages
    ));
    output.push_str(&format!(
        "Root Packages: {}\n",
        graph.statistics.root_packages
    ));
    output.push_str(&format!("Maximum Depth: {}\n", graph.statistics.max_depth));
    output.push_str(&format!(
        "Average Dependencies: {:.1}\n",
        graph.statistics.average_dependencies
    ));
    output.push_str(&format!(
        "Circular Dependencies: {}\n",
        graph.statistics.circular_dependencies
    ));
    output.push_str(&format!("Conflicts: {}\n\n", graph.statistics.conflicts));

    // Most depended upon packages
    output.push_str("🔝 Most Depended Upon\n");
    output.push_str("---------------------\n");
    let mut top_packages: Vec<_> = graph
        .packages
        .iter()
        .filter(|p| !p.required_by.is_empty())
        .collect();
    top_packages.sort_by_key(|b| std::cmp::Reverse(b.required_by.len()));

    for (idx, pkg) in top_packages.iter().take(10).enumerate() {
        output.push_str(&format!(
            "{}. {} (v{}) - {} packages depend on it\n",
            idx + 1,
            pkg.name,
            pkg.version,
            pkg.required_by.len()
        ));
    }
    output.push('\n');

    // Circular dependencies
    if !graph.circular_dependencies.is_empty() {
        output.push_str("🔄 Circular Dependencies\n");
        output.push_str("------------------------\n");
        for (idx, circ) in graph.circular_dependencies.iter().enumerate() {
            output.push_str(&format!(
                "{}. Cycle of {} packages:\n",
                idx + 1,
                circ.length
            ));
            output.push_str("   ");
            output.push_str(&circ.cycle.join(" → "));
            output.push_str(" → ");
            output.push_str(&circ.cycle[0]);
            output.push('\n');
        }
        output.push('\n');
    }

    // Conflicts
    if !graph.conflicts.is_empty() {
        output.push_str("⚠️  Dependency Conflicts\n");
        output.push_str("-----------------------\n");
        for conflict in &graph.conflicts {
            output.push_str(&format!(
                "{} {} ⟷ {}\n",
                conflict.severity.emoji(),
                conflict.package1,
                conflict.package2
            ));
            output.push_str(&format!("   Reason: {}\n", conflict.reason));
        }
        output.push('\n');
    }

    // Detailed package information
    if detailed {
        output.push_str("📦 Package Details\n");
        output.push_str("------------------\n");

        let mut packages = graph.packages.clone();
        packages.sort_by(|a, b| a.name.cmp(&b.name));

        for pkg in packages.iter().take(50) {
            output.push_str(&format!("\n{} v{}\n", pkg.name, pkg.version));

            if !pkg.depends_on.is_empty() {
                output.push_str(&format!("  Depends on: {}\n", pkg.depends_on.join(", ")));
            }

            if !pkg.required_by.is_empty() {
                output.push_str(&format!(
                    "  Required by: {} packages\n",
                    pkg.required_by.len()
                ));
                if pkg.required_by.len() <= 5 {
                    output.push_str(&format!("    {}\n", pkg.required_by.join(", ")));
                }
            }

            if pkg.is_leaf {
                output.push_str("  Type: Leaf (no dependencies)\n");
            } else if pkg.is_root {
                output.push_str("  Type: Root (nothing depends on it)\n");
            }

            output.push_str(&format!("  Depth: {}\n", pkg.depth));
        }

        if packages.len() > 50 {
            output.push_str(&format!(
                "\n... and {} more packages\n",
                packages.len() - 50
            ));
        }
    }

    // Summary
    output.push_str("\n📝 Summary\n");
    output.push_str("----------\n");

    if graph.statistics.circular_dependencies > 0 {
        output.push_str(&format!(
            "⚠️  Found {} circular dependency cycles - these should be resolved\n",
            graph.statistics.circular_dependencies
        ));
    }

    if graph.statistics.conflicts > 0 {
        output.push_str(&format!(
            "🔴 Found {} dependency conflicts - these need attention\n",
            graph.statistics.conflicts
        ));
    }

    if graph.statistics.circular_dependencies == 0 && graph.statistics.conflicts == 0 {
        output.push_str("✅ No circular dependencies or conflicts detected\n");
    }

    output.push_str(&format!(
        "\nDependency ratio: {:.1} dependencies per package\n",
        graph.statistics.average_dependencies
    ));

    output
}

/// Format dependency tree for a specific package
pub fn format_tree(graph: &DependencyGraph, package_name: &str, max_depth: usize) -> String {
    let mut output = String::new();

    // Find the package
    let pkg = match graph.packages.iter().find(|p| p.name == package_name) {
        Some(p) => p,
        None => return format!("Package '{}' not found\n", package_name),
    };

    output.push_str(&format!(
        "Dependency tree for: {} v{}\n\n",
        pkg.name, pkg.version
    ));

    // Build tree recursively
    let mut visited = std::collections::HashSet::new();
    format_tree_recursive(graph, &pkg.name, 0, max_depth, &mut visited, &mut output);

    output
}

fn format_tree_recursive(
    graph: &DependencyGraph,
    package_name: &str,
    depth: usize,
    max_depth: usize,
    visited: &mut std::collections::HashSet<String>,
    output: &mut String,
) {
    if depth >= max_depth {
        return;
    }

    let indent = "  ".repeat(depth);

    // Check for circular reference
    if visited.contains(package_name) {
        output.push_str(&format!(
            "{}└─ {} (circular reference)\n",
            indent, package_name
        ));
        return;
    }

    visited.insert(package_name.to_string());

    // Find package
    let pkg = match graph.packages.iter().find(|p| p.name == package_name) {
        Some(p) => p,
        None => {
            output.push_str(&format!("{}└─ {} (not found)\n", indent, package_name));
            return;
        }
    };

    if depth > 0 {
        output.push_str(&format!("{}├─ {} v{}\n", indent, pkg.name, pkg.version));
    }

    // Recurse into dependencies
    for (idx, dep) in pkg.depends_on.iter().enumerate() {
        let is_last = idx == pkg.depends_on.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        if depth + 1 < max_depth {
            format_tree_recursive(graph, dep, depth + 1, max_depth, visited, output);
        } else {
            output.push_str(&format!("{}  {} {}\n", indent, prefix, dep));
        }
    }

    visited.remove(package_name);
}

/// Format reverse dependency tree (what depends on this package)
pub fn format_reverse_tree(
    graph: &DependencyGraph,
    package_name: &str,
    max_depth: usize,
) -> String {
    let mut output = String::new();

    // Find the package
    let pkg = match graph.packages.iter().find(|p| p.name == package_name) {
        Some(p) => p,
        None => return format!("Package '{}' not found\n", package_name),
    };

    output.push_str(&format!(
        "Reverse dependency tree for: {} v{}\n",
        pkg.name, pkg.version
    ));
    output.push_str(&format!(
        "{} packages depend on this\n\n",
        pkg.required_by.len()
    ));

    // Build reverse tree recursively
    let mut visited = std::collections::HashSet::new();
    format_reverse_tree_recursive(graph, &pkg.name, 0, max_depth, &mut visited, &mut output);

    output
}

fn format_reverse_tree_recursive(
    graph: &DependencyGraph,
    package_name: &str,
    depth: usize,
    max_depth: usize,
    visited: &mut std::collections::HashSet<String>,
    output: &mut String,
) {
    if depth >= max_depth {
        return;
    }

    let indent = "  ".repeat(depth);

    if visited.contains(package_name) {
        output.push_str(&format!(
            "{}└─ {} (circular reference)\n",
            indent, package_name
        ));
        return;
    }

    visited.insert(package_name.to_string());

    let pkg = match graph.packages.iter().find(|p| p.name == package_name) {
        Some(p) => p,
        None => return,
    };

    if depth > 0 {
        output.push_str(&format!("{}├─ {} v{}\n", indent, pkg.name, pkg.version));
    }

    // Recurse into packages that depend on this one
    for (idx, dep) in pkg.required_by.iter().enumerate() {
        let is_last = idx == pkg.required_by.len() - 1;
        let prefix = if is_last { "└─" } else { "├─" };

        if depth + 1 < max_depth {
            format_reverse_tree_recursive(graph, dep, depth + 1, max_depth, visited, output);
        } else {
            output.push_str(&format!("{}  {} {}\n", indent, prefix, dep));
        }
    }

    visited.remove(package_name);
}
