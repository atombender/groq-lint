//! IR-based rule for detecting computed values in filters.

use crate::ir::{IrGraph, Node, NodeId, NodeKind, Provenance, ScopeKind};
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
                    if !op.is_arithmetic() {
                        return false;
                    }
                    // Only flag filters that apply directly to the dataset (`*`).
                    // Filters on sub-arrays (e.g., `things[foo == "x" + y]` inside a
                    // projection) can't leverage dataset indices anyway.
                    if !in_dataset_filter(graph, node) {
                        return false;
                    }
                    // Allow if either operand involves a parent reference (correlated subquery)
                    if involves_parent(graph, *lhs) || involves_parent(graph, *rhs) {
                        return false;
                    }
                    // Allow arithmetic on `dateTime()` values (e.g., `dateTime(now()) + 1`),
                    // which is the standard way to do date math in GROQ.
                    if involves_datetime(graph, *lhs) || involves_datetime(graph, *rhs) {
                        return false;
                    }
                    return true;
                }
                false
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}

/// Check if the innermost enclosing filter for `node` filters the dataset (`*`).
fn in_dataset_filter(graph: &IrGraph, node: &Node) -> bool {
    let mut scope_id = Some(node.scope);
    while let Some(sid) = scope_id {
        let scope = graph.scope(sid);
        if scope.kind == ScopeKind::Filter {
            if let NodeKind::Filter { base, .. } = &graph.node(scope.introducing_node).kind {
                return matches!(graph.node(*base).kind, NodeKind::Dataset);
            }
            return false;
        }
        scope_id = scope.parent;
    }
    false
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

/// Check if an expression involves a `dateTime()` call (either directly or as
/// a sub-expression of nested arithmetic).
fn involves_datetime(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);
    if is_datetime_call(&node.kind) {
        return true;
    }
    graph
        .descendants(node_id)
        .any(|d| is_datetime_call(&d.kind))
}

fn is_datetime_call(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::FunctionCall { name, namespace, .. }
        if name == "dateTime" && namespace.is_none())
}
