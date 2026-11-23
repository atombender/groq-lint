//! IR-based rule for detecting queries with many joins.
//!
//! Joins on the same reference within a scope are merged into one resolve, so
//! `author->name` repeated in several fields counts once.

use std::collections::HashSet;

use crate::ir::{IrGraph, LiteralValue, NodeId, NodeKind, ScopeId};
use crate::rules::{Hit, Rule};

const MAX_JOINS: usize = 10;

/// Detects queries with more than 10 distinct joins.
pub struct ManyJoins;

impl Rule for ManyJoins {
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
        let distinct: HashSet<(ScopeId, Result<String, NodeId>)> = graph
            .joins()
            .filter_map(|join| match &join.kind {
                NodeKind::Join { base } => {
                    Some((join.scope, reference_path(graph, *base).ok_or(join.id)))
                }
                _ => None,
            })
            .collect();

        if distinct.len() > MAX_JOINS {
            vec![Hit::global(graph.root().span)
                .with_detail(format!("Found {} distinct joins.", distinct.len()))]
        } else {
            vec![]
        }
    }
}

/// A key for the reference being joined, or `None` when the path is computed
/// and each join must count on its own.
fn reference_path(graph: &IrGraph, node_id: NodeId) -> Option<String> {
    match &graph.node(node_id).kind {
        NodeKind::This => Some("@".to_string()),
        NodeKind::Parent { depth } => Some("^".repeat(*depth as usize)),
        NodeKind::Access { base, attribute } => {
            Some(format!("{}.{}", reference_path(graph, *base)?, attribute))
        }
        NodeKind::Join { base } => Some(format!("{}->", reference_path(graph, *base)?)),
        NodeKind::ArrayCoerce { base } => Some(format!("{}[]", reference_path(graph, *base)?)),
        NodeKind::Element { base, index } => match &graph.node(*index).kind {
            NodeKind::Literal(LiteralValue::Int(i)) => {
                Some(format!("{}[{}]", reference_path(graph, *base)?, i))
            }
            _ => None,
        },
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn findings(query: &str) -> Vec<crate::rules::Finding> {
        lint(query)
            .expect("query should parse")
            .into_iter()
            .filter(|f| f.rule_id == "many_joins")
            .collect()
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(
            !findings(query).is_empty(),
            "expected many_joins to fire on: {query}"
        );
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            findings(query).is_empty(),
            "expected many_joins NOT to fire on: {query}"
        );
    }

    #[test]
    fn repeated_join_on_same_reference_counts_once() {
        let fields: Vec<String> = (0..11).map(|i| format!("\"a{i}\": author->name")).collect();
        assert_ok(&format!(
            r#"*[_type == "book"][0...10]{{{}}}"#,
            fields.join(", ")
        ));

        let fields: Vec<String> = (0..11)
            .map(|i| format!("\"a{i}\": coalesce(author->name, author->_id)"))
            .collect();
        assert_ok(&format!(
            r#"*[_type == "book"][0...10]{{{}}}"#,
            fields.join(", ")
        ));
    }

    #[test]
    fn chain_of_distinct_joins_fires() {
        let chain = format!("author{}->name", "->bestFriend".repeat(11));
        let findings = findings(&format!(r#"*[_type == "book"][0...10]{{"x": {chain}}}"#));
        assert_eq!(findings.len(), 1);
        assert!(findings[0].message.ends_with("Found 12 distinct joins."));
    }

    #[test]
    fn many_distinct_references_fire() {
        assert_fires(
            r#"*[_type == "x"][0...10]{ a->, b->, c->, d->, e->, f->, g->, h->, i->, j->, k-> }"#,
        );
        assert_ok(r#"*[_type == "x"][0...10]{ a->, b->, c->, d->, e->, f->, g->, h->, i->, j-> }"#);
    }

    #[test]
    fn same_reference_in_different_scopes_counts_separately() {
        let fields: Vec<String> = (0..11)
            .map(|i| format!("\"a{i}\": *[_type == \"t{i}\"][0]{{\"n\": author->name}}"))
            .collect();
        assert_fires(&format!(
            r#"*[_type == "book"][0]{{{}}}"#,
            fields.join(", ")
        ));
    }
}
