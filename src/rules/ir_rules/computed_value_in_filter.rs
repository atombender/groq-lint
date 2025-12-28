//! IR-based rule for detecting computed values in filters.

use crate::ir::IrGraph;
use crate::ir::NodeKind;
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

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

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
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
            .map(|node| Finding {
                span: Span::from(node.span),
                message: self.advice().to_string(),
                severity: Severity::High,
                rule_id: self.id().to_string(),
                scope: Scope::Node,
            })
            .collect()
    }
}
