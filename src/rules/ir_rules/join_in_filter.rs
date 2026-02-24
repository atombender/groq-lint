//! IR-based rule for detecting joins inside filters.

use crate::ir::IrGraph;
use crate::rules::{Hit, Rule};

/// Detects dereference operations inside filter constraints.
pub struct JoinInFilter;

impl Rule for JoinInFilter {
    fn id(&self) -> &'static str {
        "join_in_filter"
    }

    fn name(&self) -> &'static str {
        "Join in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid `->` inside filters. Move the join outside and filter on a local attribute instead."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .joins()
            .filter(|join| graph.in_filter(join))
            .map(|join| Hit::at(join.span))
            .collect()
    }
}
