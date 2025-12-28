//! IR-based rule for detecting joins used to get _id.

use crate::ir::IrGraph;
use crate::ir::NodeKind;
use crate::rules::{Hit, IrRule};

/// Detects `a->b._id` patterns that should be `a._ref`.
pub struct IrJoinToGetId;

impl IrRule for IrJoinToGetId {
    fn id(&self) -> &'static str {
        "join_to_get_id"
    }

    fn name(&self) -> &'static str {
        "Join to Get ID"
    }

    fn description(&self) -> &'static str {
        "Avoid using `->` to retrieve `_id` of a document. Use `._ref` instead."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        // Pattern: Access { base: Join { ... }, attribute: "_id" }
        graph
            .nodes()
            .filter_map(|node| {
                if let NodeKind::Access { base, attribute } = &node.kind {
                    if attribute == "_id" {
                        // Check if base is a Join
                        let base_node = graph.node(*base);
                        if matches!(base_node.kind, NodeKind::Join { .. }) {
                            return Some(Hit::at(node.span));
                        }
                    }
                }
                None
            })
            .collect()
    }
}
