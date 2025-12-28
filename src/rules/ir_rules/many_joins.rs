//! IR-based rule for detecting queries with many joins.

use crate::ir::IrGraph;
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects queries with more than 10 dereference operators.
pub struct IrManyJoins;

impl IrRule for IrManyJoins {
    fn id(&self) -> &'static str {
        "many_joins"
    }

    fn name(&self) -> &'static str {
        "Many Joins"
    }

    fn description(&self) -> &'static str {
        "The query uses more than 10 `->` operators. Consider reducing the number of joins."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        let join_count = graph.joins().count();

        if join_count > 10 {
            vec![Finding {
                span: Span::from(graph.root().span),
                message: format!("{} Found {} joins.", self.advice(), join_count),
                severity: Severity::Medium,
                rule_id: self.id().to_string(),
                scope: Scope::Global,
            }]
        } else {
            vec![]
        }
    }
}
