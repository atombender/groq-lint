//! IR-based rules for pagination issues.

use crate::ir::IrGraph;
use crate::ir::{LiteralValue, NodeKind};
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects deep pagination (start index > 1000).
pub struct IrDeepPagination;

impl IrRule for IrDeepPagination {
    fn id(&self) -> &'static str {
        "deep_pagination"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination"
    }

    fn description(&self) -> &'static str {
        "Deep pagination is slow. Consider cursor-based pagination using `_id`."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        let mut findings = vec![];

        for node in graph.nodes() {
            match &node.kind {
                // Check Slice nodes
                NodeKind::Slice {
                    start: Some(start_id),
                    ..
                } => {
                    let start_node = graph.node(*start_id);
                    if let NodeKind::Literal(LiteralValue::Int(val)) = &start_node.kind {
                        if *val > 1000 {
                            findings.push(Finding {
                                span: Span::from(node.span),
                                message: self.advice().to_string(),
                                severity: Severity::Medium,
                                rule_id: self.id().to_string(),
                                scope: Scope::Node,
                            });
                        }
                    }
                }
                // Check Range nodes
                NodeKind::Range { start, .. } => {
                    let start_node = graph.node(*start);
                    if let NodeKind::Literal(LiteralValue::Int(val)) = &start_node.kind {
                        if *val > 1000 {
                            findings.push(Finding {
                                span: Span::from(node.span),
                                message: self.advice().to_string(),
                                severity: Severity::Medium,
                                rule_id: self.id().to_string(),
                                scope: Scope::Node,
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        findings
    }
}

/// Detects pagination with $param start index.
pub struct IrDeepPaginationParam;

impl IrRule for IrDeepPaginationParam {
    fn id(&self) -> &'static str {
        "deep_pagination_param"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination Param"
    }

    fn description(&self) -> &'static str {
        "If given a large value, this can cause deep pagination, which is slow. Consider cursor-based pagination."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        let mut findings = vec![];

        for node in graph.nodes() {
            match &node.kind {
                // Check Slice nodes
                NodeKind::Slice {
                    start: Some(start_id),
                    ..
                } => {
                    let start_node = graph.node(*start_id);
                    if matches!(start_node.kind, NodeKind::Param { .. }) {
                        findings.push(Finding {
                            span: Span::from(node.span),
                            message: self.advice().to_string(),
                            severity: Severity::Medium,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                }
                // Check Range nodes
                NodeKind::Range { start, .. } => {
                    let start_node = graph.node(*start);
                    if matches!(start_node.kind, NodeKind::Param { .. }) {
                        findings.push(Finding {
                            span: Span::from(node.span),
                            message: self.advice().to_string(),
                            severity: Severity::Medium,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                }
                _ => {}
            }
        }

        findings
    }
}

/// Detects large page fetches (> 100 items starting from 0).
pub struct IrLargePages;

impl IrRule for IrLargePages {
    fn id(&self) -> &'static str {
        "large_pages"
    }

    fn name(&self) -> &'static str {
        "Large Pages"
    }

    fn description(&self) -> &'static str {
        "Fetching many results at once can be slow. Consider breaking into smaller batches."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        let mut findings = vec![];

        for node in graph.nodes() {
            match &node.kind {
                // Check Slice nodes [0..N] where N > 100
                NodeKind::Slice { start, end, .. } => {
                    let start_is_zero = match start {
                        Some(start_id) => {
                            let start_node = graph.node(*start_id);
                            matches!(&start_node.kind, NodeKind::Literal(LiteralValue::Int(0)))
                        }
                        None => true, // Implicit 0
                    };

                    if start_is_zero {
                        if let Some(end_id) = end {
                            let end_node = graph.node(*end_id);
                            if let NodeKind::Literal(LiteralValue::Int(val)) = &end_node.kind {
                                if *val > 100 {
                                    findings.push(Finding {
                                        span: Span::from(node.span),
                                        message: self.advice().to_string(),
                                        severity: Severity::Medium,
                                        rule_id: self.id().to_string(),
                                        scope: Scope::Node,
                                    });
                                }
                            }
                        }
                    }
                }
                // Check Range nodes
                NodeKind::Range { start, end, .. } => {
                    let start_node = graph.node(*start);
                    let start_is_zero =
                        matches!(&start_node.kind, NodeKind::Literal(LiteralValue::Int(0)));

                    if start_is_zero {
                        let end_node = graph.node(*end);
                        if let NodeKind::Literal(LiteralValue::Int(val)) = &end_node.kind {
                            if *val > 100 {
                                findings.push(Finding {
                                    span: Span::from(node.span),
                                    message: self.advice().to_string(),
                                    severity: Severity::Medium,
                                    rule_id: self.id().to_string(),
                                    scope: Scope::Node,
                                });
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        findings
    }
}
