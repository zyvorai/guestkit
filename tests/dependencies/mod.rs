// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Dependencies module tests

use guestkit::cli::dependencies::*;

#[cfg(test)]
mod graph_tests {
    use super::*;

    fn create_test_graph() -> DependencyGraph {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "2.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-c".to_string(),
                version: "1.5.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![
            Dependency {
                package: "pkg-a".to_string(),
                depends_on: "pkg-b".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "pkg-b".to_string(),
                depends_on: "pkg-c".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
        ];

        DependencyGraph {
            packages,
            dependencies,
            circular_dependencies: vec![],
            conflicts: vec![],
            statistics: GraphStatistics {
                total_packages: 3,
                total_dependencies: 2,
                leaf_packages: 1,
                root_packages: 1,
                average_dependencies: 0.67,
                max_dependency_depth: 2,
            },
        }
    }

    #[test]
    fn test_dependency_graph_structure() {
        let graph = create_test_graph();

        assert_eq!(graph.packages.len(), 3);
        assert_eq!(graph.dependencies.len(), 2);
        assert_eq!(graph.statistics.total_packages, 3);
        assert_eq!(graph.statistics.total_dependencies, 2);
    }

    #[test]
    fn test_graph_statistics() {
        let graph = create_test_graph();

        assert_eq!(graph.statistics.leaf_packages, 1); // pkg-c
        assert_eq!(graph.statistics.root_packages, 1); // pkg-a
        assert!(graph.statistics.average_dependencies > 0.0);
        assert_eq!(graph.statistics.max_dependency_depth, 2);
    }
}

#[cfg(test)]
mod analyzer_tests {
    use super::*;

    #[test]
    fn test_detect_circular_dependencies_simple() {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![
            Dependency {
                package: "pkg-a".to_string(),
                depends_on: "pkg-b".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "pkg-b".to_string(),
                depends_on: "pkg-a".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
        ];

        let circular = analyzer::detect_circular_dependencies(&packages, &dependencies);

        assert!(!circular.is_empty());
        assert!(circular[0].cycle.contains(&"pkg-a".to_string()));
        assert!(circular[0].cycle.contains(&"pkg-b".to_string()));
    }

    #[test]
    fn test_detect_circular_dependencies_none() {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![Dependency {
            package: "pkg-a".to_string(),
            depends_on: "pkg-b".to_string(),
            version_constraint: None,
            dependency_type: DependencyType::Required,
        }];

        let circular = analyzer::detect_circular_dependencies(&packages, &dependencies);

        assert!(circular.is_empty());
    }

    #[test]
    fn test_detect_conflicts_version() {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "lib".to_string(),
                version: "2.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![
            Dependency {
                package: "pkg-a".to_string(),
                depends_on: "lib".to_string(),
                version_constraint: Some(">= 2.0.0".to_string()),
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "pkg-b".to_string(),
                depends_on: "lib".to_string(),
                version_constraint: Some("< 2.0.0".to_string()),
                dependency_type: DependencyType::Required,
            },
        ];

        let conflicts = analyzer::detect_conflicts(&packages, &dependencies);

        // Should detect version conflict on lib
        assert!(!conflicts.is_empty());
    }

    #[test]
    fn test_detect_conflicts_none() {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![];

        let conflicts = analyzer::detect_conflicts(&packages, &dependencies);

        assert!(conflicts.is_empty());
    }

    #[test]
    fn test_find_dependency_chain() {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-c".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![
            Dependency {
                package: "pkg-a".to_string(),
                depends_on: "pkg-b".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "pkg-b".to_string(),
                depends_on: "pkg-c".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
        ];

        let chain = analyzer::find_dependency_chain(&dependencies, "pkg-a", "pkg-c");

        assert_eq!(chain.len(), 3);
        assert_eq!(chain[0], "pkg-a");
        assert_eq!(chain[1], "pkg-b");
        assert_eq!(chain[2], "pkg-c");
    }

    #[test]
    fn test_find_reverse_dependencies() {
        let dependencies = vec![
            Dependency {
                package: "pkg-a".to_string(),
                depends_on: "lib".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "pkg-b".to_string(),
                depends_on: "lib".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
        ];

        let reverse = analyzer::find_reverse_dependencies(&dependencies, "lib");

        assert_eq!(reverse.len(), 2);
        assert!(reverse.contains(&"pkg-a".to_string()));
        assert!(reverse.contains(&"pkg-b".to_string()));
    }
}

#[cfg(test)]
mod graph_export_tests {
    use super::*;

    fn create_test_graph() -> DependencyGraph {
        let packages = vec![
            Package {
                name: "pkg-a".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "pkg-b".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![Dependency {
            package: "pkg-a".to_string(),
            depends_on: "pkg-b".to_string(),
            version_constraint: None,
            dependency_type: DependencyType::Required,
        }];

        DependencyGraph {
            packages,
            dependencies,
            circular_dependencies: vec![],
            conflicts: vec![],
            statistics: GraphStatistics {
                total_packages: 2,
                total_dependencies: 1,
                leaf_packages: 1,
                root_packages: 1,
                average_dependencies: 0.5,
                max_dependency_depth: 1,
            },
        }
    }

    #[test]
    fn test_export_dot_format() {
        let graph = create_test_graph();
        let dot = graph::export_dot(&graph);

        assert!(dot.contains("digraph dependencies"));
        assert!(dot.contains("pkg-a"));
        assert!(dot.contains("pkg-b"));
        assert!(dot.contains("->"));
    }

    #[test]
    fn test_export_json_format() {
        let graph = create_test_graph();
        let json = graph::export_json(&graph);

        assert!(json.is_ok());
        let json_str = json.unwrap();
        assert!(json_str.contains("\"packages\""));
        assert!(json_str.contains("\"dependencies\""));
        assert!(json_str.contains("pkg-a"));
    }

    #[test]
    fn test_export_csv_format() {
        let graph = create_test_graph();
        let csv = graph::export_csv(&graph);

        assert!(csv.starts_with("Package,Version,Depends On"));
        assert!(csv.contains("pkg-a"));
        assert!(csv.contains("pkg-b"));
    }

    #[test]
    fn test_export_html_format() {
        let graph = create_test_graph();
        let html = graph::export_html(&graph);

        assert!(html.contains("<!DOCTYPE html>"));
        assert!(html.contains("<html>"));
        assert!(html.contains("Dependency Graph"));
        assert!(html.contains("pkg-a"));
        assert!(html.contains("pkg-b"));
    }
}

#[cfg(test)]
mod visualizer_tests {
    use super::*;

    fn create_test_graph() -> DependencyGraph {
        let packages = vec![
            Package {
                name: "root-pkg".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "mid-pkg".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
            Package {
                name: "leaf-pkg".to_string(),
                version: "1.0.0".to_string(),
                architecture: "amd64".to_string(),
            },
        ];

        let dependencies = vec![
            Dependency {
                package: "root-pkg".to_string(),
                depends_on: "mid-pkg".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
            Dependency {
                package: "mid-pkg".to_string(),
                depends_on: "leaf-pkg".to_string(),
                version_constraint: None,
                dependency_type: DependencyType::Required,
            },
        ];

        DependencyGraph {
            packages,
            dependencies,
            circular_dependencies: vec![],
            conflicts: vec![],
            statistics: GraphStatistics {
                total_packages: 3,
                total_dependencies: 2,
                leaf_packages: 1,
                root_packages: 1,
                average_dependencies: 0.67,
                max_dependency_depth: 2,
            },
        }
    }

    #[test]
    fn test_format_text_report() {
        let graph = create_test_graph();
        let report = visualizer::format_text_report(&graph);

        assert!(report.contains("Dependency Graph Report"));
        assert!(report.contains("Total Packages: 3"));
        assert!(report.contains("Total Dependencies: 2"));
        assert!(report.contains("root-pkg"));
    }

    #[test]
    fn test_format_dependency_tree() {
        let graph = create_test_graph();
        let tree = visualizer::format_dependency_tree(&graph, "root-pkg");

        assert!(tree.contains("root-pkg"));
        assert!(tree.contains("mid-pkg"));
        assert!(tree.contains("leaf-pkg"));
    }

    #[test]
    fn test_format_reverse_tree() {
        let graph = create_test_graph();
        let tree = visualizer::format_reverse_tree(&graph, "leaf-pkg");

        assert!(tree.contains("leaf-pkg"));
        assert!(tree.contains("mid-pkg"));
        assert!(tree.contains("root-pkg"));
    }

    #[test]
    fn test_report_contains_circular_dependencies() {
        let mut graph = create_test_graph();
        graph.circular_dependencies.push(CircularDependency {
            cycle: vec!["pkg-a".to_string(), "pkg-b".to_string()],
            severity: "High".to_string(),
        });

        let report = visualizer::format_text_report(&graph);

        assert!(report.contains("Circular Dependencies"));
        assert!(report.contains("pkg-a"));
    }

    #[test]
    fn test_report_contains_conflicts() {
        let mut graph = create_test_graph();
        graph.conflicts.push(Conflict {
            package1: "pkg-a".to_string(),
            package2: "pkg-b".to_string(),
            reason: "Version mismatch".to_string(),
            severity: "Medium".to_string(),
        });

        let report = visualizer::format_text_report(&graph);

        assert!(report.contains("Conflicts"));
        assert!(report.contains("pkg-a"));
        assert!(report.contains("Version mismatch"));
    }
}

#[cfg(test)]
mod dependency_type_tests {
    use super::*;

    #[test]
    fn test_dependency_type_variants() {
        let types = vec![
            DependencyType::Required,
            DependencyType::Recommended,
            DependencyType::Suggested,
            DependencyType::Conflicts,
        ];

        assert_eq!(types.len(), 4);
    }
}
