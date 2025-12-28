//! IR-based rule for detecting count() on correlated subqueries.

use crate::ir::{IrGraph, NodeId};
use crate::ir::{NodeKind, Provenance};
use crate::rules::{Hit, IrRule};

/// Detects count() function calls on correlated subqueries.
pub struct IrCountInCorrelatedSubquery;

impl IrRule for IrCountInCorrelatedSubquery {
    fn id(&self) -> &'static str {
        "count_in_correlated_subquery"
    }

    fn name(&self) -> &'static str {
        "Count in Correlated Subquery"
    }

    fn description(&self) -> &'static str {
        "Using `count()` on a correlated subquery does not execute as an efficient aggregation."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .function_calls()
            .filter_map(|node| {
                if let NodeKind::FunctionCall { name, args, .. } = &node.kind {
                    if name == "count" {
                        // Check if any argument is a correlated subquery
                        for &arg_id in args {
                            if is_correlated(graph, arg_id) {
                                return Some(Hit::at(node.span));
                            }
                        }
                    }
                }
                None
            })
            .collect()
    }
}

/// Check if an expression references parent scope (^), making it correlated.
fn is_correlated(graph: &IrGraph, node_id: NodeId) -> bool {
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
