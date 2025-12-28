//! IR-based rule for detecting computed values in filters.

use crate::ir::IrGraph;
use crate::ir::NodeKind;
use crate::rules::{Hit, IrRule};

/// Detects arithmetic operations inside filter constraints.
pub struct IrComputedValueInFilter;

impl IrRule for IrComputedValueInFilter {
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
                if let NodeKind::Binary { op, .. } = &node.kind {
                    // Check if it's an arithmetic operator
                    if op.is_arithmetic() {
                        // Check if we're inside a filter
                        return graph.in_filter(node);
                    }
                }
                false
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}
