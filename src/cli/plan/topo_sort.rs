// Copyright 2026 Zyvor AI Labs · https://zyvor.dev
// SPDX-License-Identifier: Apache-2.0

//! Topological sort for fix plan operations.

use super::types::{FixPlan, Operation};
use std::collections::{HashMap, VecDeque};

/// Sort operations respecting `depends_on` edges.
pub fn topological_sort(plan: &FixPlan) -> Vec<&Operation> {
    if plan.operations.is_empty() {
        return Vec::new();
    }

    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adj: HashMap<&str, Vec<&str>> = HashMap::new();
    let mut op_map: HashMap<&str, &Operation> = HashMap::new();

    for op in &plan.operations {
        in_degree.entry(op.id.as_str()).or_insert(0);
        adj.entry(op.id.as_str()).or_default();
        op_map.insert(op.id.as_str(), op);
    }

    for op in &plan.operations {
        for dep_id in &op.depends_on {
            adj.entry(dep_id.as_str()).or_default().push(op.id.as_str());
            *in_degree.entry(op.id.as_str()).or_insert(0) += 1;
        }
    }

    let mut queue: VecDeque<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(&node, _)| node)
        .collect();

    let mut sorted = Vec::new();

    while let Some(node) = queue.pop_front() {
        if let Some(&op) = op_map.get(node) {
            sorted.push(op);
        }
        if let Some(neighbors) = adj.get(node) {
            for &neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push_back(neighbor);
                    }
                }
            }
        }
    }

    sorted
}
