//! IR-based rules for detecting oversized queries.

use crate::ir::IrGraph;
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects queries larger than 10KB.
pub struct IrVeryLargeQuery;

impl IrRule for IrVeryLargeQuery {
    fn id(&self) -> &'static str {
        "very_large_query"
    }

    fn name(&self) -> &'static str {
        "Very Large Query"
    }

    fn description(&self) -> &'static str {
        "The query is larger than 10KB. Consider breaking it into smaller queries."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        const THRESHOLD: usize = 10 * 1024; // 10KB

        if graph.query_len > THRESHOLD {
            vec![Finding {
                span: Span::from(graph.root().span),
                message: format!("{} Query size: {} bytes.", self.advice(), graph.query_len),
                severity: Severity::High,
                rule_id: self.id().to_string(),
                scope: Scope::Global,
            }]
        } else {
            vec![]
        }
    }
}

/// Detects queries larger than 100KB.
pub struct IrExtremelyLargeQuery;

impl IrRule for IrExtremelyLargeQuery {
    fn id(&self) -> &'static str {
        "extremely_large_query"
    }

    fn name(&self) -> &'static str {
        "Extremely Large Query"
    }

    fn description(&self) -> &'static str {
        "The query is larger than 100KB. This may cause performance issues."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        const THRESHOLD: usize = 100 * 1024; // 100KB

        if graph.query_len > THRESHOLD {
            vec![Finding {
                span: Span::from(graph.root().span),
                message: format!("{} Query size: {} bytes.", self.advice(), graph.query_len),
                severity: Severity::High,
                rule_id: self.id().to_string(),
                scope: Scope::Global,
            }]
        } else {
            vec![]
        }
    }

    fn supercedes(&self) -> &'static [&'static str] {
        &["very_large_query"]
    }
}
