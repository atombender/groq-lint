//! IR-based rule for detecting non-literal comparisons.

use crate::ir::{IrGraph, NodeId};
use crate::ir::{NodeKind, Provenance};
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects comparisons where both sides are non-literal (can't use indices).
pub struct IrNonLiteralComparison;

impl IrRule for IrNonLiteralComparison {
    fn id(&self) -> &'static str {
        "non_literal_comparison"
    }

    fn name(&self) -> &'static str {
        "Non-Literal Comparison"
    }

    fn description(&self) -> &'static str {
        "Comparisons between two non-literal fields cannot use indices efficiently."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        graph
            .binary_ops()
            .filter_map(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    // Only check comparison operators
                    if !op.is_comparison() {
                        return None;
                    }

                    let lhs_literal = is_literal_or_parent(graph, *lhs);
                    let rhs_literal = is_literal_or_parent(graph, *rhs);

                    // Per rules.yaml: parent refs (^) on either side are allowed
                    // Both sides must be non-literal for this to trigger
                    if !lhs_literal && !rhs_literal {
                        return Some(Finding {
                            span: Span::from(node.span),
                            message: self.advice().to_string(),
                            severity: Severity::High, // Per rules.yaml
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                }
                None
            })
            .collect()
    }
}

/// Check if expression is a literal or references parent scope.
/// Per rules.yaml:
/// - Literal expressions (including computed literals like 2+1) are literals
/// - now() is considered a literal
/// - Params ($foo) are considered literals
/// - Parent scope refs (^.bar) on either side make the comparison allowed
fn is_literal_or_parent(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    // Check provenance first
    match &node.provenance {
        Provenance::Literal => return true,
        Provenance::Param { .. } => return true,
        Provenance::Parent { .. } => return true,
        _ => {}
    }

    // Check node kind
    match &node.kind {
        NodeKind::Literal(_) => true,
        NodeKind::Param { .. } => true,
        NodeKind::Parent { .. } => true,
        // now() is considered a literal
        NodeKind::FunctionCall { name, .. } if name == "now" => true,
        // Check if any descendant is a parent ref (for ^.foo patterns)
        NodeKind::Access { base, .. } => {
            let base_node = graph.node(*base);
            matches!(base_node.kind, NodeKind::Parent { .. }) || is_literal_or_parent(graph, *base)
        }
        _ => false,
    }
}
