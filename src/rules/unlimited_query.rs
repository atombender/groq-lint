//! IR-based rule for detecting top-level queries against the dataset that
//! lack a slice/element to bound the result.

use crate::ir::{BinaryOp, IrGraph, Node, NodeId, NodeKind};
use crate::rules::{Hit, Rule};

/// Detects top-level queries against `*` that have no slice/element to
/// bound the result, with the exception of filters that constrain by
/// `_id ==` or `slug.current ==` (which always return at most one document).
pub struct UnlimitedQuery;

impl Rule for UnlimitedQuery {
    fn id(&self) -> &'static str {
        "unlimited_query"
    }

    fn name(&self) -> &'static str {
        "Unlimited Query"
    }

    fn description(&self) -> &'static str {
        "Top-level query without a slice or element limit may return many results."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        let root = graph.root();
        match analyse(graph, root, false, false) {
            Some(span_node) => vec![Hit::at(span_node.span)],
            None => vec![],
        }
    }
}

/// Walks the root chain looking for an unbounded `*[...]` (or bare `*`).
///
/// Returns the offending node if the top-level expression is a dataset
/// query that isn't bounded by a slice/element and isn't constrained
/// by `_id ==` / `slug.current ==`.
fn analyse<'g>(
    graph: &'g IrGraph,
    node: &'g Node,
    bounded: bool,
    has_id_constraint: bool,
) -> Option<&'g Node> {
    match &node.kind {
        // Slicing or single-element access bounds the result.
        NodeKind::Slice { base, .. } | NodeKind::Element { base, .. } => {
            analyse(graph, graph.node(*base), true, has_id_constraint)
        }
        // Pass-through wrappers that don't affect cardinality.
        NodeKind::Projection { base, .. }
        | NodeKind::ArrayCoerce { base }
        | NodeKind::Join { base } => analyse(graph, graph.node(*base), bounded, has_id_constraint),
        // Pipes like `... | order(...)` — walk into the piped expression.
        NodeKind::FunctionCall {
            pipe_base: Some(b), ..
        } => analyse(graph, graph.node(*b), bounded, has_id_constraint),
        NodeKind::Filter {
            base, predicate, ..
        } => {
            let constrained = has_id_constraint || predicate_has_id_constraint(graph, *predicate);
            let base_node = graph.node(*base);
            if matches!(base_node.kind, NodeKind::Dataset) {
                if bounded || constrained {
                    None
                } else {
                    Some(node)
                }
            } else {
                analyse(graph, base_node, bounded, constrained)
            }
        }
        NodeKind::Dataset => {
            if bounded {
                None
            } else {
                Some(node)
            }
        }
        _ => None,
    }
}

/// Returns true if the predicate is (or contains, via an AND-chain) an
/// equality on `_id` or `slug.current`, which always returns at most one
/// document.
fn predicate_has_id_constraint(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);
    match &node.kind {
        NodeKind::Binary {
            op: BinaryOp::And,
            lhs,
            rhs,
        } => predicate_has_id_constraint(graph, *lhs) || predicate_has_id_constraint(graph, *rhs),
        NodeKind::Binary {
            op: BinaryOp::Eq,
            lhs,
            rhs,
        } => is_id_or_slug(graph, *lhs) || is_id_or_slug(graph, *rhs),
        _ => false,
    }
}

/// Returns true if the expression is `_id` or `slug.current` on `this`.
fn is_id_or_slug(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);
    let NodeKind::Access { base, attribute } = &node.kind else {
        return false;
    };
    let base_node = graph.node(*base);
    if attribute == "_id" {
        return matches!(base_node.kind, NodeKind::This);
    }
    if attribute == "current" {
        if let NodeKind::Access {
            base: inner_base,
            attribute: inner_attr,
        } = &base_node.kind
        {
            if inner_attr == "slug" {
                return matches!(graph.node(*inner_base).kind, NodeKind::This);
            }
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn fires(query: &str) -> bool {
        let findings = lint(query).expect("query should parse");
        findings.iter().any(|f| f.rule_id == "unlimited_query")
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(fires(query), "expected unlimited_query to fire on: {query}");
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            !fires(query),
            "expected unlimited_query NOT to fire on: {query}"
        );
    }

    #[test]
    fn unbounded_dataset_filter_fires() {
        assert_fires(r#"*[_type == "article"]"#);
    }

    #[test]
    fn bare_dataset_fires() {
        assert_fires("*");
    }

    #[test]
    fn slice_bounds_query() {
        assert_ok(r#"*[_type == "article"][0..100]"#);
        assert_ok("*[0..10]");
    }

    #[test]
    fn single_element_bounds_query() {
        assert_ok(r#"*[_type == "article"][0]"#);
    }

    #[test]
    fn id_equality_exempts() {
        assert_ok(r#"*[_id == "doc-123"]"#);
        assert_ok("*[_id == $id]");
    }

    #[test]
    fn slug_current_equality_exempts() {
        assert_ok(r#"*[slug.current == "x"]"#);
    }

    #[test]
    fn id_in_and_chain_exempts() {
        assert_ok(r#"*[_type == "x" && _id == "y"]"#);
    }

    #[test]
    fn id_via_chained_filter_exempts() {
        assert_ok(r#"*[_type == "x"][_id == "y"]"#);
    }

    #[test]
    fn id_under_or_does_not_exempt() {
        assert_fires(r#"*[_type == "x" || _id == "y"]"#);
    }

    #[test]
    fn id_inequality_does_not_exempt() {
        assert_fires(r#"*[_id != "x"]"#);
    }

    #[test]
    fn id_in_array_does_not_exempt() {
        assert_fires("*[_id in $ids]");
    }

    #[test]
    fn projection_without_slice_fires() {
        assert_fires(r#"*[_type == "article"]{_id}"#);
    }

    #[test]
    fn projection_then_slice_is_ok() {
        assert_ok(r#"*[_type == "article"]{_id}[0..100]"#);
    }

    #[test]
    fn slice_then_projection_is_ok() {
        assert_ok(r#"*[_type == "article"][0..100]{_id}"#);
    }

    #[test]
    fn order_pipe_without_slice_fires() {
        assert_fires(r#"*[_type == "article"] | order(_createdAt desc)"#);
    }

    #[test]
    fn order_pipe_then_slice_is_ok() {
        assert_ok(r#"*[_type == "article"] | order(_createdAt desc)[0..100]"#);
    }

    #[test]
    fn array_coercion_does_not_bound() {
        assert_fires(r#"*[_type == "x"][]"#);
    }

    #[test]
    fn non_dataset_query_is_ignored() {
        assert_ok("42");
        assert_ok("$foo");
    }

    #[test]
    fn dataset_query_inside_function_is_ignored() {
        // The dataset query is inside count(); only the top-level expression
        // is checked, and that's the comparison, not a dataset query.
        assert_ok(r#"count(*[_type == "x"]) > 0"#);
    }
}
