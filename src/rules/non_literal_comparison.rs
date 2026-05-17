//! IR-based rule for detecting non-literal comparisons.

use crate::ir::{IrGraph, NodeId};
use crate::ir::{NodeKind, Provenance};
use crate::rules::{Hit, Rule};

/// Detects comparisons where both sides are non-literal (can't use indices).
pub struct NonLiteralComparison;

impl Rule for NonLiteralComparison {
    fn id(&self) -> &'static str {
        "non_literal_comparison"
    }

    fn name(&self) -> &'static str {
        "Non-Literal Comparison"
    }

    fn description(&self) -> &'static str {
        "Comparisons between two non-literal fields cannot use indices efficiently."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter_map(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    // Only check comparison operators
                    if !op.is_comparison() {
                        return None;
                    }

                    let lhs_safe = is_literal_or_parent(graph, *lhs);
                    let rhs_safe = is_literal_or_parent(graph, *rhs);

                    // Per rules.yaml: parent refs (^) and subqueries (rooted in `*`)
                    // on either side are allowed. Both sides must be non-literal
                    // for this to trigger.
                    if !lhs_safe && !rhs_safe {
                        return Some(Hit::at(node.span));
                    }
                }
                None
            })
            .collect()
    }
}

/// Check if expression is a literal, references parent scope, or is rooted
/// in a subquery (`*`).
/// Per rules.yaml:
/// - Literal expressions (including computed literals like 2+1) are literals
/// - now() is considered a literal
/// - Params ($foo) are considered literals
/// - Parent scope refs (^.bar) on either side make the comparison allowed
/// - Subqueries rooted in `*` (e.g., `*[0].foo`) on either side are allowed
fn is_literal_or_parent(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    // Check provenance first
    match &node.provenance {
        Provenance::Literal => return true,
        Provenance::Param { .. } => return true,
        Provenance::Parent { .. } => return true,
        Provenance::Dataset => return true,
        _ => {}
    }

    // Check node kind
    match &node.kind {
        NodeKind::Literal(_) => true,
        NodeKind::Param { .. } => true,
        NodeKind::Parent { .. } => true,
        NodeKind::Dataset => true,
        // now() is considered a literal
        NodeKind::FunctionCall { name, .. } if name == "now" => true,
        // Walk through attribute access (e.g., ^.foo or ^.foo.bar)
        NodeKind::Access { base, .. } => is_literal_or_parent(graph, *base),
        // Walk through dereferences (e.g., ^.library->_id) — a join rooted
        // in a parent ref still resolves through the parent scope.
        NodeKind::Join { base } => is_literal_or_parent(graph, *base),
        // Walk through subquery traversals so that subqueries rooted in `*`
        // (e.g., `*[0].foo`, `*[_type == "x"][0]`, `*{...}`) are recognised.
        NodeKind::Element { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::Filter { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::Slice { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::ArrayCoerce { base } => is_literal_or_parent(graph, *base),
        NodeKind::Projection { base, .. } => is_literal_or_parent(graph, *base),
        // Binary expressions: check if either operand involves parent ref (e.g., "drafts." + ^._id)
        NodeKind::Binary { lhs, rhs, .. } => {
            is_literal_or_parent(graph, *lhs) || is_literal_or_parent(graph, *rhs)
        }
        _ => false,
    }
}
