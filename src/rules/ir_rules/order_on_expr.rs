//! IR-based rule for detecting ordering on computed expressions.

use crate::ir::{BinaryOp, NodeKind};
use crate::ir::{IrGraph, NodeId};
use crate::rules::{Hit, IrRule};

/// Detects order() calls with arguments that aren't plain attributes or allowed functions.
pub struct IrOrderOnExpr;

impl IrRule for IrOrderOnExpr {
    fn id(&self) -> &'static str {
        "order_on_expr"
    }

    fn name(&self) -> &'static str {
        "Order on Expression"
    }

    fn description(&self) -> &'static str {
        "Avoid ordering on computed values. Indices cannot be used to sort."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let mut hits = vec![];

        for node in graph.function_calls() {
            if let NodeKind::FunctionCall { name, args, .. } = &node.kind {
                if name == "order" {
                    for &arg_id in args {
                        // Unwrap asc/desc modifier if present
                        let check_id = unwrap_order_modifier(graph, arg_id);

                        if !is_allowed_order_expr(graph, check_id) {
                            let arg_node = graph.node(arg_id);
                            hits.push(Hit::at(arg_node.span));
                        }
                    }
                }
            }
        }

        hits
    }
}

/// Unwrap asc/desc modifier to get the inner expression.
fn unwrap_order_modifier(graph: &IrGraph, node_id: NodeId) -> NodeId {
    let node = graph.node(node_id);
    if let NodeKind::Binary { op, lhs, .. } = &node.kind {
        if *op == BinaryOp::Asc || *op == BinaryOp::Desc {
            return *lhs;
        }
    }
    node_id
}

/// Check if expression is allowed for ordering.
/// Allowed:
/// - Plain attributes
/// - lower(attr)
/// - dateTime(attr)
/// - geo::distance(attr, constant)
fn is_allowed_order_expr(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    match &node.kind {
        // Plain attribute access is allowed
        NodeKind::Access { base, .. } => {
            // Check base is This (implicit or explicit)
            let base_node = graph.node(*base);
            matches!(base_node.kind, NodeKind::This)
        }

        // Allowed functions: lower, dateTime, geo::distance
        NodeKind::FunctionCall {
            namespace,
            name,
            args,
            ..
        } => {
            match (namespace.as_deref(), name.as_str()) {
                // lower(attr) and dateTime(attr)
                (None, "lower") | (None, "dateTime") => {
                    args.len() == 1 && is_allowed_order_expr(graph, args[0])
                }
                // geo::distance(attr, constant)
                (Some("geo"), "distance") => {
                    args.len() == 2 && is_allowed_order_expr(graph, args[0])
                    // Second arg should be a constant, but we're lenient here
                }
                _ => false,
            }
        }

        _ => false,
    }
}
