//! IR-based rule for detecting joins inside filters.

use crate::ir::IrGraph;
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects dereference operations inside filter constraints.
pub struct IrJoinInFilter;

impl IrRule for IrJoinInFilter {
    fn id(&self) -> &'static str {
        "join_in_filter"
    }

    fn name(&self) -> &'static str {
        "Join in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid `->` inside filters. Move the join outside and filter on a local attribute instead."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        graph
            .joins()
            .filter(|join| graph.in_filter(join))
            .map(|join| Finding {
                span: Span::from(join.span),
                message: self.advice().to_string(),
                severity: Severity::High,
                rule_id: self.id().to_string(),
                scope: Scope::Node,
            })
            .collect()
    }
}
