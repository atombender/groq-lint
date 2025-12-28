//! IR-based rule for detecting queries with many joins.

use crate::ir::IrGraph;
use crate::rules::{Hit, IrRule};

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

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let join_count = graph.joins().count();

        if join_count > 10 {
            vec![Hit::global(graph.root().span).with_detail(format!("Found {} joins.", join_count))]
        } else {
            vec![]
        }
    }
}
