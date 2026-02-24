//! IR-based rules for pagination issues.

use crate::ir::IrGraph;
use crate::ir::{LiteralValue, NodeKind};
use crate::rules::{Hit, Rule};

/// Detects deep pagination (start index > 1000).
pub struct DeepPagination;

impl Rule for DeepPagination {
    fn id(&self) -> &'static str {
        "deep_pagination"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination"
    }

    fn description(&self) -> &'static str {
        "Deep pagination is slow. Consider cursor-based pagination using `_id`."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let mut hits = vec![];

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
                            hits.push(Hit::at(node.span));
                        }
                    }
                }
                // Check Range nodes
                NodeKind::Range { start, .. } => {
                    let start_node = graph.node(*start);
                    if let NodeKind::Literal(LiteralValue::Int(val)) = &start_node.kind {
                        if *val > 1000 {
                            hits.push(Hit::at(node.span));
                        }
                    }
                }
                _ => {}
            }
        }

        hits
    }
}

/// Detects pagination with $param start index.
pub struct DeepPaginationParam;

impl Rule for DeepPaginationParam {
    fn id(&self) -> &'static str {
        "deep_pagination_param"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination Param"
    }

    fn description(&self) -> &'static str {
        "If given a large value, this can cause deep pagination, which is slow. Consider cursor-based pagination."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let mut hits = vec![];

        for node in graph.nodes() {
            match &node.kind {
                // Check Slice nodes
                NodeKind::Slice {
                    start: Some(start_id),
                    ..
                } => {
                    let start_node = graph.node(*start_id);
                    if matches!(start_node.kind, NodeKind::Param { .. }) {
                        hits.push(Hit::at(node.span));
                    }
                }
                // Check Range nodes
                NodeKind::Range { start, .. } => {
                    let start_node = graph.node(*start);
                    if matches!(start_node.kind, NodeKind::Param { .. }) {
                        hits.push(Hit::at(node.span));
                    }
                }
                _ => {}
            }
        }

        hits
    }
}

/// Detects large page fetches (> 100 items starting from 0).
pub struct LargePages;

impl Rule for LargePages {
    fn id(&self) -> &'static str {
        "large_pages"
    }

    fn name(&self) -> &'static str {
        "Large Pages"
    }

    fn description(&self) -> &'static str {
        "Fetching many results at once can be slow. Consider breaking into smaller batches."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let mut hits = vec![];

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
                                    hits.push(Hit::at(node.span));
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
                                hits.push(Hit::at(node.span));
                            }
                        }
                    }
                }
                _ => {}
            }
        }

        hits
    }
}
