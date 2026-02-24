//! IR-based rule for detecting computed values in filters.

use crate::ir::{IrGraph, NodeId, NodeKind, Provenance};
use crate::rules::{Hit, Rule};

/// Detects arithmetic operations inside filter constraints.
pub struct ComputedValueInFilter;

impl Rule for ComputedValueInFilter {
    fn id(&self) -> &'static str {
        "computed_value_in_filter"
    }

    fn name(&self) -> &'static str {
        "Computed Value in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid computed values (concatenation, arithmetic, etc.) in filters. Indices cannot be used."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    // Check if it's an arithmetic operator
                    if op.is_arithmetic() {
                        // Check if we're inside a filter
                        if graph.in_filter(node) {
                            // Allow if either operand involves a parent reference (correlated subquery)
                            if involves_parent(graph, *lhs) || involves_parent(graph, *rhs) {
                                return false;
                            }
                            return true;
                        }
                    }
                }
                false
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}

/// Check if an expression involves a parent scope reference (^).
fn involves_parent(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    // Check provenance
    if matches!(node.provenance, Provenance::Parent { .. }) {
        return true;
    }

    // Check node kind
    if matches!(node.kind, NodeKind::Parent { .. }) {
        return true;
    }

    // Recursively check descendants
    for descendant in graph.descendants(node_id) {
        if matches!(descendant.kind, NodeKind::Parent { .. }) {
            return true;
        }
        if matches!(descendant.provenance, Provenance::Parent { .. }) {
            return true;
        }
    }

    false
}
