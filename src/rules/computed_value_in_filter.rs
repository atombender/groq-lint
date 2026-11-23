//! IR-based rule for detecting computed values in filters.

use crate::ir::{IrGraph, Node, NodeId, NodeKind, ScopeKind, Span};
use crate::rules::{Hit, Rule};

/// Functions that are evaluated in memory for every candidate document. Functions
/// the index evaluates itself, such as `lower()`, `dateTime()`,
/// `string::startsWith()`, `pt::text()`, `references()` or `count(arr)`, are not
/// listed.
const IN_MEMORY_FUNCTIONS: [&str; 5] = ["upper", "length", "round", "coalesce", "select"];

/// Detects arithmetic operations and in-memory function calls inside filter constraints.
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
        let mut hits = arithmetic_hits(graph);
        hits.extend(
            graph
                .nodes()
                .filter(|node| graph.in_document_filter(node) && !graph.involves_parent(node.id))
                .filter_map(|node| in_memory_function_hit(graph, node)),
        );
        hits
    }
}

fn arithmetic_hits(graph: &IrGraph) -> Vec<Hit> {
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
                if graph.involves_parent(*lhs) || graph.involves_parent(*rhs) {
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

/// A function call (or element of `string::split()`) that is evaluated in
/// memory for every candidate document.
fn in_memory_function_hit(graph: &IrGraph, node: &Node) -> Option<Hit> {
    match &node.kind {
        NodeKind::FunctionCall { name, args, .. } => {
            if let Some(function) = IN_MEMORY_FUNCTIONS
                .iter()
                .find(|function| node.kind.is_global_call(function))
            {
                return Some(Hit::at(node.span).with_detail(format!(
                    "`{function}()` is evaluated in memory for every candidate document."
                )));
            }
            if name == "defined" && args.len() == 1 && accesses_element(graph, args[0]) {
                return Some(Hit::at(node.span).with_detail(
                    "`defined()` of an array element is evaluated in memory for every candidate document.",
                ));
            }
            None
        }
        NodeKind::Element { base, .. } => {
            let base_node = graph.node(*base);
            let is_split = matches!(&base_node.kind, NodeKind::FunctionCall { namespace: Some(ns), name, .. }
                if ns == "string" && name == "split");
            is_split.then(|| {
                Hit::at(Span {
                    start: base_node.span.start,
                    end: node.span.end,
                })
                .with_detail(
                    "An element of `string::split()` is evaluated in memory for every candidate document.",
                )
            })
        }
        _ => None,
    }
}

fn accesses_element(graph: &IrGraph, node_id: NodeId) -> bool {
    match &graph.node(node_id).kind {
        NodeKind::Element { .. } => true,
        NodeKind::Access { base, .. } => accesses_element(graph, *base),
        _ => false,
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

    fn spans(query: &str) -> Vec<&str> {
        lint(query)
            .expect("query should parse")
            .into_iter()
            .filter(|f| f.rule_id == "computed_value_in_filter")
            .map(|f| &query[f.span.start..f.span.end])
            .collect()
    }

    #[test]
    fn in_memory_functions_fire() {
        assert_eq!(
            spans(r#"*[_type == "book" && upper(title) == "X"]"#),
            vec!["upper"]
        );
        assert_fires(r#"*[_type == "book" && length(title) > 10]"#);
        assert_fires(r#"*[_type == "book" && round(rating) == 4]"#);
        assert_fires(r#"*[_type == "book" && coalesce(subtitle, title) == "X"]"#);
        assert_fires(r#"*[_type == "book" && select(rating > 3 => "good", "bad") == "good"]"#);
        assert_fires(r#"*[_type == "book" && defined(reviews[0])]"#);
    }

    #[test]
    fn in_memory_function_names_itself_in_message() {
        let findings = lint(r#"*[_type == "book" && length(title) > 10]"#).unwrap();
        let finding = findings
            .iter()
            .find(|f| f.rule_id == "computed_value_in_filter")
            .unwrap();
        assert!(finding
            .message
            .ends_with("`length()` is evaluated in memory for every candidate document."));
    }

    #[test]
    fn in_memory_function_in_chained_filter_fires() {
        assert_fires(r#"*[_type == "book"][upper(title) == "X"]"#);
        assert_fires(r#"*[_type == "book"][][length(title) > 10]"#);
    }

    #[test]
    fn element_of_split_fires() {
        assert_eq!(
            spans(r#"*[_type == "book" && string::split(title, " ")[0] == "The"]"#),
            vec![r#"string::split(title, " ")[0]"#]
        );
    }

    #[test]
    fn index_evaluated_functions_are_ok() {
        for query in [
            r#"*[_type == "book" && lower(title) == "foo"]"#,
            r#"*[_type == "book" && string::startsWith(title, "The")]"#,
            r#"*[_type == "book" && pt::text(body) match "foo"]"#,
            r#"*[_type == "book" && count(tags) > 1]"#,
            r#"*[_type == "book" && title in ["a", "b"]]"#,
            r#"*[_type == "book" && _id in path("books.**")]"#,
            r#"*[_type == "book" && _createdAt > dateTime(now()) - 60 * 60 * 24]"#,
            r#"*[_type == "book" && references("author-1")]"#,
            r#"*[_type == "book" && "x" in tags]"#,
            r#"*[_type == "book" && defined(subtitle)]"#,
        ] {
            assert_ok(query);
        }
    }

    #[test]
    fn in_memory_function_on_parent_is_ok() {
        assert_ok(r#"*[_type == "author"]{"n": *[_type == "book" && title == upper(^.name)]}"#);
    }

    #[test]
    fn functions_outside_dataset_filters_are_ok() {
        assert_ok(r#"*[_type == "book"][0...10]{"t": upper(title)}"#);
        assert_ok(r#"*[_type == "book"][0...10]{"r": reviews[length(text) > 10]}"#);
    }
}
