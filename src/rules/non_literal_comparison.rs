//! IR-based rule for detecting non-literal comparisons.

use crate::ir::{IrGraph, NodeId};
use crate::ir::{NodeKind, Provenance};
use crate::rules::{Hit, Rule};

const INDEX_COERCIONS: [&str; 2] = ["dateTime", "lower"];

/// Detects comparisons where both sides are non-literal (can't use indices).
pub struct NonLiteralComparison;

impl Rule for NonLiteralComparison {
    fn id(&self) -> &'static str {
        "non_literal_comparison"
    }

    fn name(&self) -> &'static str {
        "Non-Literal Comparison"
    }

    fn description(&self) -> &'static str {
        "Comparisons between two non-literal fields cannot use indices efficiently."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter_map(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    // Only check comparison operators
                    if !op.is_comparison() {
                        return None;
                    }

                    let lhs_safe = is_literal_or_parent(graph, *lhs);
                    let rhs_safe = is_literal_or_parent(graph, *rhs);

                    // Per rules.yaml: parent refs (^) and subqueries (rooted in `*`)
                    // on either side are allowed. Both sides must be non-literal
                    // for this to trigger.
                    if !lhs_safe && !rhs_safe {
                        return Some(Hit::at(node.span));
                    }
                }
                None
            })
            .collect()
    }
}

/// Check if expression is a literal, references parent scope, or is rooted
/// in a subquery (`*`).
/// Per rules.yaml:
/// - Literal expressions (including computed literals like 2+1) are literals
/// - now() is considered a literal
/// - `dateTime(x)` and `lower(x)` are literal (or parent) when `x` is
/// - Params ($foo) are considered literals
/// - Parent scope refs (^.bar) on either side make the comparison allowed
/// - Subqueries rooted in `*` (e.g., `*[0].foo`) on either side are allowed
fn is_literal_or_parent(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    // Check provenance first
    match &node.provenance {
        Provenance::Literal => return true,
        Provenance::Param { .. } => return true,
        Provenance::Parent { .. } => return true,
        Provenance::Dataset => return true,
        _ => {}
    }

    // Check node kind
    match &node.kind {
        NodeKind::Literal(_) => true,
        NodeKind::Param { .. } => true,
        NodeKind::Parent { .. } => true,
        NodeKind::Dataset => true,
        // now() is considered a literal
        kind if kind.is_global_call("now") => true,
        // The index evaluates `dateTime()` and `lower()` itself, so a coercion of
        // a literal is a literal, and a coercion of a parent ref is a batch key.
        NodeKind::FunctionCall {
            args,
            pipe_base: None,
            ..
        } if args.len() == 1
            && INDEX_COERCIONS
                .iter()
                .any(|name| node.kind.is_global_call(name)) =>
        {
            is_literal_or_parent(graph, args[0])
        }
        // Walk through attribute access (e.g., ^.foo or ^.foo.bar)
        NodeKind::Access { base, .. } => is_literal_or_parent(graph, *base),
        // Walk through dereferences (e.g., ^.library->_id) — a join rooted
        // in a parent ref still resolves through the parent scope.
        NodeKind::Join { base } => is_literal_or_parent(graph, *base),
        // Walk through subquery traversals so that subqueries rooted in `*`
        // (e.g., `*[0].foo`, `*[_type == "x"][0]`, `*{...}`) are recognised.
        NodeKind::Element { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::Filter { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::Slice { base, .. } => is_literal_or_parent(graph, *base),
        NodeKind::ArrayCoerce { base } => is_literal_or_parent(graph, *base),
        NodeKind::Projection { base, .. } => is_literal_or_parent(graph, *base),
        // Binary expressions: check if either operand involves parent ref (e.g., "drafts." + ^._id)
        NodeKind::Binary { lhs, rhs, .. } => {
            is_literal_or_parent(graph, *lhs) || is_literal_or_parent(graph, *rhs)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn fires(query: &str) -> bool {
        let findings = lint(query).expect("query should parse");
        findings
            .iter()
            .any(|f| f.rule_id == "non_literal_comparison")
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(
            fires(query),
            "expected non_literal_comparison to fire on: {query}"
        );
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            !fires(query),
            "expected non_literal_comparison NOT to fire on: {query}"
        );
    }

    #[test]
    fn two_non_literal_attributes_fires() {
        assert_fires(r#"*[foo == bar]"#);
    }

    #[test]
    fn attribute_vs_literal_is_ok() {
        assert_ok(r#"*[foo == "bar"]"#);
    }

    #[test]
    fn attribute_vs_param_is_ok() {
        assert_ok("*[foo == $bar]");
    }

    #[test]
    fn attribute_vs_now_is_ok() {
        assert_ok("*[foo == now()]");
    }

    #[test]
    fn parent_ref_exempts() {
        assert_ok("*[_type == 'x']{ refs[_ref == ^._id] }");
    }

    #[test]
    fn parent_ref_in_dereference_exempts() {
        assert_ok("*[_type == 'x']{ refs[foo._ref == ^.library->_id] }");
    }

    #[test]
    fn parent_ref_inside_binary_exempts() {
        assert_ok(r#"*[_type == 'x']{ refs[_id == "drafts." + ^._id] }"#);
    }

    #[test]
    fn subquery_root_exempts() {
        assert_fires(r#"*[foo == bar]"#); // sanity: baseline
        assert_ok(r#"*[foo == *[0].title]"#);
    }

    #[test]
    fn filtered_subquery_exempts() {
        assert_ok(r#"*[foo == *[_type == "author"][0].name]"#);
    }

    #[test]
    fn projected_subquery_exempts() {
        assert_ok(r#"*[foo == *{title}[0].title]"#);
    }

    #[test]
    fn inequality_between_attributes_fires() {
        assert_fires("*[foo != bar]");
    }

    #[test]
    fn less_than_between_attributes_fires() {
        assert_fires("*[foo < bar]");
    }

    #[test]
    fn folded_arithmetic_is_literal() {
        assert_ok(r#"*[foo == 2 + 3]"#);
    }

    #[test]
    fn two_attributes_fire() {
        assert_fires("*[_createdAt == _updatedAt]");
        assert_fires("*[title == slug.current]");
    }

    #[test]
    fn datetime_of_literal_is_a_literal() {
        assert_ok(
            r#"*[_type == "book" && dateTime(_createdAt) > dateTime("2020-01-01T00:00:00Z")]"#,
        );
        assert_fires("*[dateTime(_createdAt) > dateTime(_updatedAt)]");
    }

    #[test]
    fn coercion_of_parent_is_a_batch_key() {
        assert_ok(
            r#"*[_type == "author"][0...3]{"n": count(*[_type == "book" && lower(title) == lower(^.name)])}"#,
        );
        assert_fires("*[lower(title) == lower(subtitle)]");
    }

    #[test]
    fn other_functions_of_literals_are_not_literals() {
        assert_fires(r#"*[upper(title) == upper("x")]"#);
    }
}
