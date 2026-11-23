//! IR-based rule for detecting count() on correlated subqueries.
//!
//! A correlated subquery whose `^` conditions are all equalities
//! (`author._ref == ^._id`) or `references(^._id)` is batched: one index read
//! for every row, keyed on the parent value (or a real aggregation under a
//! single-row parent). Any other `^` condition (`>`, `<`, `in`, `match`, ...)
//! reads every document matched by the rest of the subquery once, then filters
//! that batch in memory for each row.

use crate::ir::{BinaryOp, IrGraph, NodeId, NodeKind};
use crate::rules::{Hit, Rule};

/// Detects count() function calls on correlated subqueries that cannot be batched.
pub struct CountInCorrelatedSubquery;

impl Rule for CountInCorrelatedSubquery {
    fn id(&self) -> &'static str {
        "count_in_correlated_subquery"
    }

    fn name(&self) -> &'static str {
        "Count in Correlated Subquery"
    }

    fn description(&self) -> &'static str {
        "Using `count()` on a correlated subquery does not execute as an efficient aggregation."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .function_calls()
            .filter(|node| {
                node.kind.is_global_call("count")
                    && matches!(&node.kind, NodeKind::FunctionCall { args, .. }
                        if args.iter().any(|&arg| has_unbatched_correlation(graph, arg)))
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}

/// Check if a document filter in an expression has a `^` condition that cannot be batched.
fn has_unbatched_correlation(graph: &IrGraph, node_id: NodeId) -> bool {
    std::iter::once(graph.node(node_id))
        .chain(graph.descendants(node_id))
        .any(|node| match &node.kind {
            NodeKind::Filter {
                base, predicate, ..
            } => {
                graph.is_document_stream(*base)
                    && conjuncts(graph, *predicate)
                        .into_iter()
                        .any(|term| graph.involves_parent(term) && !is_batch_key(graph, term))
            }
            _ => false,
        })
}

/// Split an `&&` chain into its terms.
fn conjuncts(graph: &IrGraph, node_id: NodeId) -> Vec<NodeId> {
    match &graph.node(node_id).kind {
        NodeKind::Binary {
            op: BinaryOp::And,
            lhs,
            rhs,
        } => {
            let mut terms = conjuncts(graph, *lhs);
            terms.extend(conjuncts(graph, *rhs));
            terms
        }
        _ => vec![node_id],
    }
}

/// `x == ^.y` (with `^` on exactly one side) or `references(^.y)`.
fn is_batch_key(graph: &IrGraph, node_id: NodeId) -> bool {
    match &graph.node(node_id).kind {
        NodeKind::Binary {
            op: BinaryOp::Eq,
            lhs,
            rhs,
        } => graph.involves_parent(*lhs) != graph.involves_parent(*rhs),
        kind => kind.is_global_call("references"),
    }
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn fires(query: &str) -> bool {
        let findings = lint(query).expect("query should parse");
        findings
            .iter()
            .any(|f| f.rule_id == "count_in_correlated_subquery")
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(
            fires(query),
            "expected count_in_correlated_subquery to fire on: {query}"
        );
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            !fires(query),
            "expected count_in_correlated_subquery NOT to fire on: {query}"
        );
    }

    #[test]
    fn equality_on_parent_is_batched() {
        assert_ok(
            r#"*[_type == "author"][0...3]{name, "books": count(*[_type == "book" && author._ref == ^._id])}"#,
        );
        assert_ok(r#"*[_type == "author"][0...3]{"n": count(*[_id == ^._id])}"#);
        assert_ok(r#"*[_type == "author"][0...3]{"n": count(*[^._id == author._ref])}"#);
        assert_ok(
            r#"*[_type == "author"][0...3]{"n": count(*[_type == "book" && lower(title) == lower(^.name)])}"#,
        );
    }

    #[test]
    fn references_parent_is_batched() {
        assert_ok(r#"*[_type == "author"][0...3]{"n": count(*[references(^._id)])}"#);
        assert_ok(
            r#"*[_type == "author"][0...3]{"n": count(*[_type == "book" && references(^._id)])}"#,
        );
    }

    #[test]
    fn projected_ordered_or_sliced_equality_is_batched() {
        assert_ok(r#"*[_type == "author"][0...3]{"n": count(*[author._ref == ^._id]{title})}"#);
        assert_ok(
            r#"*[_type == "author"][0...3]{"n": count(*[author._ref == ^._id] | order(title))}"#,
        );
        assert_ok(r#"*[_type == "author"][0...3]{"n": count(*[author._ref == ^._id][0...10])}"#);
    }

    #[test]
    fn equality_in_filter_and_order_is_batched() {
        assert_ok(
            r#"*[_type == "author" && count(*[_type == "book" && author._ref == ^._id]) > 2][0...3]"#,
        );
        assert_ok(
            r#"*[_type == "author"] | order(count(*[_type == "book" && author._ref == ^._id]) desc)[0...3]"#,
        );
    }

    #[test]
    fn non_equality_on_parent_fires() {
        assert_fires(
            r#"*[_type == "book"][0...3]{"n": count(*[_type == "book" && _createdAt > ^._createdAt])}"#,
        );
        assert_fires(
            r#"*[_type == "author"][0...3]{"n": count(*[_type == "book" && ^._id in reviews[]._ref])}"#,
        );
        assert_fires(
            r#"*[_type == "author"][0...3]{"n": count(*[_type == "book" && title match ^.name])}"#,
        );
        assert_fires(
            r#"*[_type == "author"][0...3]{"n": count(*[author._ref == ^._id || editor._ref == ^._id])}"#,
        );
    }

    #[test]
    fn non_equality_after_flatten_fires() {
        assert_fires(r#"*[_type == "book"][0...3]{"n": count(*[][_createdAt > ^._createdAt])}"#);
    }

    #[test]
    fn non_equality_alongside_equality_fires() {
        assert_fires(
            r#"*[_type == "author"][0...3]{"n": count(*[author._ref == ^._id && _createdAt > ^._createdAt])}"#,
        );
    }

    #[test]
    fn finding_spans_the_count_call() {
        let query = r#"*[_type == "book"][0...3]{"n": count(*[_createdAt > ^._createdAt])}"#;
        let spans: Vec<&str> = lint(query)
            .unwrap()
            .into_iter()
            .filter(|f| f.rule_id == "count_in_correlated_subquery")
            .map(|f| &query[f.span.start..f.span.end])
            .collect();
        assert_eq!(spans, vec!["count"]);
    }

    #[test]
    fn uncorrelated_count_is_ok() {
        assert_ok(r#"count(*[_type == "book"])"#);
        assert_ok(r#"*[_type == "book"][0...3]{"n": count(tags)}"#);
    }
}
