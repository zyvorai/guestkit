// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Graph export formats

use super::*;

/// Export graph as Graphviz DOT format
pub fn export_dot(graph: &DependencyGraph, show_all: bool) -> String {
    let mut dot = String::new();

    dot.push_str("digraph dependencies {\n");
    dot.push_str("  rankdir=TB;\n");
    dot.push_str("  node [shape=box, style=rounded];\n");
    dot.push_str("  edge [color=gray];\n\n");

    // Limit to most important packages if not showing all
    let packages_to_show: Vec<_> = if show_all {
        graph.packages.clone()
    } else {
        // Show packages with most dependencies
        let mut pkgs = graph.packages.clone();
        pkgs.sort_by_key(|b| std::cmp::Reverse(b.required_by.len()));
        pkgs.truncate(50);
        pkgs
    };

    let package_names: std::collections::HashSet<_> =
        packages_to_show.iter().map(|p| &p.name).collect();

    // Add nodes
    for pkg in &packages_to_show {
        let color = if pkg.is_root {
            "lightblue"
        } else if pkg.is_leaf {
            "lightgreen"
        } else {
            "white"
        };

        let label = format!("{}\nv{}", pkg.name, pkg.version);
        dot.push_str(&format!(
            "  \"{}\" [label=\"{}\", fillcolor={}, style=\"filled,rounded\"];\n",
            pkg.name, label, color
        ));
    }

    dot.push('\n');

    // Add edges
    for dep in &graph.dependencies {
        if package_names.contains(&dep.from) && package_names.contains(&dep.to) {
            let style = match dep.dependency_type {
                DependencyType::Required => "solid",
                DependencyType::Recommended => "dashed",
                DependencyType::Suggested => "dotted",
                DependencyType::Conflicts => "bold",
            };

            let color = if dep.dependency_type == DependencyType::Conflicts {
                "red"
            } else {
                "gray"
            };

            dot.push_str(&format!(
                "  \"{}\" -> \"{}\" [style={}, color={}];\n",
                dep.from, dep.to, style, color
            ));
        }
    }

    // Highlight circular dependencies
    if !graph.circular_dependencies.is_empty() {
        dot.push_str("\n  // Circular dependencies\n");
        for circ in &graph.circular_dependencies {
            for i in 0..circ.cycle.len() {
                let from = &circ.cycle[i];
                let to = &circ.cycle[(i + 1) % circ.cycle.len()];
                if package_names.contains(from) && package_names.contains(to) {
                    dot.push_str(&format!(
                        "  \"{}\" -> \"{}\" [color=red, penwidth=2];\n",
                        from, to
                    ));
                }
            }
        }
    }

    dot.push_str("}\n");
    dot
}

/// Export graph as JSON
pub fn export_json(graph: &DependencyGraph) -> Result<String> {
    serde_json::to_string_pretty(graph)
        .map_err(|e| anyhow::anyhow!("Failed to serialize graph: {}", e))
}

/// Export graph as CSV (edges list)
pub fn export_csv(graph: &DependencyGraph) -> String {
    let mut csv = String::new();

    csv.push_str("From,To,Type,Optional\n");

    for dep in &graph.dependencies {
        csv.push_str(&format!(
            "\"{}\",\"{}\",{:?},{}\n",
            dep.from, dep.to, dep.dependency_type, dep.is_optional
        ));
    }

    csv
}

/// Export graph as interactive HTML
pub fn export_html(graph: &DependencyGraph) -> String {
    let mut html = String::new();

    html.push_str("<!DOCTYPE html>\n");
    html.push_str("<html>\n<head>\n");
    html.push_str("<meta charset=\"utf-8\">\n");
    html.push_str("<title>Dependency Graph</title>\n");
    html.push_str("<style>\n");
    html.push_str("body { font-family: Arial, sans-serif; margin: 20px; }\n");
    html.push_str("table { border-collapse: collapse; width: 100%; }\n");
    html.push_str("th, td { border: 1px solid #ddd; padding: 8px; text-align: left; }\n");
    html.push_str("th { background-color: #4CAF50; color: white; }\n");
    html.push_str("tr:nth-child(even) { background-color: #f2f2f2; }\n");
    html.push_str(
        ".stats { background: #e7f3fe; padding: 15px; margin: 10px 0; border-radius: 5px; }\n",
    );
    html.push_str("</style>\n");
    html.push_str("</head>\n<body>\n");

    html.push_str("<h1>Dependency Graph Analysis</h1>\n");

    // Statistics
    html.push_str("<div class=\"stats\">\n");
    html.push_str("<h2>Statistics</h2>\n");
    html.push_str(&format!(
        "<p><strong>Total Packages:</strong> {}</p>\n",
        graph.statistics.total_packages
    ));
    html.push_str(&format!(
        "<p><strong>Total Dependencies:</strong> {}</p>\n",
        graph.statistics.total_dependencies
    ));
    html.push_str(&format!(
        "<p><strong>Circular Dependencies:</strong> {}</p>\n",
        graph.statistics.circular_dependencies
    ));
    html.push_str(&format!(
        "<p><strong>Conflicts:</strong> {}</p>\n",
        graph.statistics.conflicts
    ));
    html.push_str(&format!(
        "<p><strong>Average Dependencies:</strong> {:.1}</p>\n",
        graph.statistics.average_dependencies
    ));
    html.push_str("</div>\n");

    // Top packages
    let mut top_packages: Vec<_> = graph
        .packages
        .iter()
        .filter(|p| !p.required_by.is_empty())
        .collect();
    top_packages.sort_by_key(|b| std::cmp::Reverse(b.required_by.len()));

    html.push_str("<h2>Most Depended Upon Packages</h2>\n");
    html.push_str("<table>\n");
    html.push_str("<tr><th>Package</th><th>Version</th><th>Depended By</th></tr>\n");
    for pkg in top_packages.iter().take(20) {
        html.push_str(&format!(
            "<tr><td>{}</td><td>{}</td><td>{}</td></tr>\n",
            pkg.name,
            pkg.version,
            pkg.required_by.len()
        ));
    }
    html.push_str("</table>\n");

    // Circular dependencies
    if !graph.circular_dependencies.is_empty() {
        html.push_str("<h2>Circular Dependencies</h2>\n");
        html.push_str("<ul>\n");
        for circ in &graph.circular_dependencies {
            html.push_str(&format!("<li>{}</li>\n", circ.cycle.join(" → ")));
        }
        html.push_str("</ul>\n");
    }

    // Conflicts
    if !graph.conflicts.is_empty() {
        html.push_str("<h2>Dependency Conflicts</h2>\n");
        html.push_str("<table>\n");
        html.push_str(
            "<tr><th>Package 1</th><th>Package 2</th><th>Reason</th><th>Severity</th></tr>\n",
        );
        for conflict in &graph.conflicts {
            html.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{:?}</td></tr>\n",
                conflict.package1, conflict.package2, conflict.reason, conflict.severity
            ));
        }
        html.push_str("</table>\n");
    }

    html.push_str("</body>\n</html>\n");

    html
}
