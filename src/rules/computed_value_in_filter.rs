//! IR-based rule for detecting computed values in filters.

use crate::ir::{IrGraph, Node, NodeId, NodeKind, Provenance, ScopeKind};
use crate::rules::{Hit, Rule};

/// Detects arithmetic operations inside filter constraints.
pub struct ComputedValueInFilter;

impl Rule for ComputedValueInFilter {
    fn id(&self) -> &'static str {
        "computed_value_in_filter"
    }

    fn name(&self) -> &'static str {
        "Computed Value in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid computed values (concatenation, arithmetic, etc.) in filters. Indices cannot be used."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Hit> {
        graph
            .binary_ops()
            .filter(|node| {
                if let NodeKind::Binary { op, lhs, rhs } = &node.kind {
                    // Check if it's an arithmetic operator
                    if !op.is_arithmetic() {
                        return false;
                    }
                    // Only flag filters that apply directly to the dataset (`*`).
                    // Filters on sub-arrays (e.g., `things[foo == "x" + y]` inside a
                    // projection) can't leverage dataset indices anyway.
                    if !in_dataset_filter(graph, node) {
                        return false;
                    }
                    // Allow if either operand involves a parent reference (correlated subquery)
                    if involves_parent(graph, *lhs) || involves_parent(graph, *rhs) {
                        return false;
                    }
                    // Allow arithmetic on `dateTime()` values (e.g., `dateTime(now()) + 1`),
                    // which is the standard way to do date math in GROQ.
                    if involves_datetime(graph, *lhs) || involves_datetime(graph, *rhs) {
                        return false;
                    }
                    return true;
                }
                false
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}

/// Check if the innermost enclosing filter for `node` filters the dataset (`*`).
fn in_dataset_filter(graph: &IrGraph, node: &Node) -> bool {
    let mut scope_id = Some(node.scope);
    while let Some(sid) = scope_id {
        let scope = graph.scope(sid);
        if scope.kind == ScopeKind::Filter {
            if let NodeKind::Filter { base, .. } = &graph.node(scope.introducing_node).kind {
                return matches!(graph.node(*base).kind, NodeKind::Dataset);
            }
            return false;
        }
        scope_id = scope.parent;
    }
    false
}

/// Check if an expression involves a parent scope reference (^).
fn involves_parent(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);

    // Check provenance
    if matches!(node.provenance, Provenance::Parent { .. }) {
        return true;
    }

    // Check node kind
    if matches!(node.kind, NodeKind::Parent { .. }) {
        return true;
    }

    // Recursively check descendants
    for descendant in graph.descendants(node_id) {
        if matches!(descendant.kind, NodeKind::Parent { .. }) {
            return true;
        }
        if matches!(descendant.provenance, Provenance::Parent { .. }) {
            return true;
        }
    }

    false
}

/// Check if an expression involves a `dateTime()` call (either directly or as
/// a sub-expression of nested arithmetic).
fn involves_datetime(graph: &IrGraph, node_id: NodeId) -> bool {
    let node = graph.node(node_id);
    if is_datetime_call(&node.kind) {
        return true;
    }
    graph
        .descendants(node_id)
        .any(|d| is_datetime_call(&d.kind))
}

fn is_datetime_call(kind: &NodeKind) -> bool {
    matches!(kind, NodeKind::FunctionCall { name, namespace, .. }
        if name == "dateTime" && namespace.is_none())
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn fires(query: &str) -> bool {
        let findings = lint(query).expect("query should parse");
        findings
            .iter()
            .any(|f| f.rule_id == "computed_value_in_filter")
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(
            fires(query),
            "expected computed_value_in_filter to fire on: {query}"
        );
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            !fires(query),
            "expected computed_value_in_filter NOT to fire on: {query}"
        );
    }

    #[test]
    fn concat_in_dataset_filter_fires() {
        assert_fires(r#"*[foo == "bar" + baz]"#);
    }

    #[test]
    fn arithmetic_in_dataset_filter_fires() {
        assert_fires("*[count > price + 1]");
        assert_fires("*[a + b == c]");
    }

    #[test]
    fn no_arithmetic_is_ok() {
        assert_ok(r#"*[foo == "bar"]"#);
    }

    #[test]
    fn parent_ref_in_arithmetic_exempts() {
        // `"drafts." + ^._id` is a correlated subquery pattern.
        assert_ok(r#"*[_type == "x"]{ refs[_id == "drafts." + ^._id] }"#);
    }

    #[test]
    fn arithmetic_in_sub_array_filter_is_ok() {
        // `things[...]` is a sub-array filter inside a projection, not a
        // dataset filter — no index applies anyway.
        assert_ok(r#"*{"x": things[foo == "bar" + baz]}"#);
    }

    #[test]
    fn arithmetic_in_nested_dataset_filter_is_ok() {
        // The inner filter's base is the outer filter's result, not `*`.
        assert_ok(r#"*[_type == "x"][foo == "bar" + baz]"#);
    }

    #[test]
    fn datetime_arithmetic_is_ok() {
        assert_ok("*[_createdAt > dateTime(now()) + 1]");
    }

    #[test]
    fn datetime_field_arithmetic_is_ok() {
        assert_ok("*[_createdAt > dateTime(_updatedAt) + 60]");
    }

    #[test]
    fn nested_datetime_arithmetic_is_ok() {
        assert_ok("*[_createdAt > dateTime(now()) - 60 * 60 * 24]");
    }

    #[test]
    fn namespaced_datetime_does_not_exempt() {
        // Only the global `dateTime(...)` is exempted, not `dt::dateTime(...)`.
        assert_fires("*[a == dt::dateTime(now()) + 1]");
    }

    #[test]
    fn comparison_is_not_arithmetic() {
        // Comparisons (==, !=, <, ...) are not flagged by this rule.
        assert_ok(r#"*[foo == bar]"#);
    }

    #[test]
    fn arithmetic_outside_any_filter_is_ok() {
        // Arithmetic in a projection field, not inside a filter, isn't flagged.
        assert_ok(r#"*{"sum": a + b}"#);
    }
}
