//! IR-based rule for detecting match operator on _id.

use crate::ir::IrGraph;
use crate::ir::{BinaryOp, LiteralValue, NodeKind};
use crate::rules::{Hit, IrRule};

/// Detects `_id match "*pattern*"` usage.
pub struct IrMatchOnId;

impl IrRule for IrMatchOnId {
    fn id(&self) -> &'static str {
        "match_on_id"
    }

    fn name(&self) -> &'static str {
        "Match on ID"
    }

    fn description(&self) -> &'static str {
        "`match` is intended for full-text matching and may not work as expected on _id."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter_map(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    if *op == BinaryOp::Match {
                        // Check LHS is access to _id
                        let lhs_node = graph.node(*lhs);
                        let is_id_access = matches!(
                            &lhs_node.kind,
                            NodeKind::Access { attribute, .. } if attribute == "_id"
                        );

                        if is_id_access {
                            // Check RHS is string literal with wildcard
                            let rhs_node = graph.node(*rhs);
                            if let NodeKind::Literal(LiteralValue::String(s)) = &rhs_node.kind {
                                if s.contains('*') {
                                    return Some(Hit::at(node.span));
                                }
                            }
                        }
                    }
                }
                None
            })
            .collect()
    }
}
