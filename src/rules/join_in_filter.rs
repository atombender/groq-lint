//! IR-based rule for detecting joins inside filters.
//!
//! A join in a filter over documents read from the dataset is resolved for
//! every candidate document, and the filter runs in memory. A filter over one
//! document's attribute array inside a projection (`reviews[@->title == "x"]`)
//! has no index read to lose.

use crate::ir::{IrGraph, Node, NodeKind, ScopeKind};
use crate::rules::{Hit, Rule};

/// Detects dereference operations inside filter constraints over documents.
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
            .filter(|join| in_filter_over_documents(graph, join))
            .map(|join| Hit::at(join.span))
            .collect()
    }
}

/// Check if a node is inside a filter over documents, looking through filters
/// over attribute arrays but not past a projection.
fn in_filter_over_documents(graph: &IrGraph, node: &Node) -> bool {
    let mut scope_id = Some(node.scope);
    while let Some(id) = scope_id {
        let scope = graph.scope(id);
        match scope.kind {
            ScopeKind::Filter => {
                if let NodeKind::Filter { base, .. } = &graph.node(scope.introducing_node).kind {
                    if graph.is_document_stream(*base) {
                        return true;
                    }
                }
            }
            ScopeKind::Projection | ScopeKind::Root => return false,
            ScopeKind::FunctionArg => {}
        }
        scope_id = scope.parent;
    }
    false
}

#[cfg(test)]
mod tests {
    use crate::lint;

    fn fires(query: &str) -> bool {
        let findings = lint(query).expect("query should parse");
        findings.iter().any(|f| f.rule_id == "join_in_filter")
    }

    #[track_caller]
    fn assert_fires(query: &str) {
        assert!(fires(query), "expected join_in_filter to fire on: {query}");
    }

    #[track_caller]
    fn assert_ok(query: &str) {
        assert!(
            !fires(query),
            "expected join_in_filter NOT to fire on: {query}"
        );
    }

    #[test]
    fn join_in_dataset_filter_fires() {
        assert_fires(r#"*[_type == "book" && author->name == "Ada"][0...10]"#);
        assert_fires(r#"*[_type == "book"][author->name == "Ada"][0...10]"#);
        assert_fires(
            r#"*[_type == "author"][0...3]{"books": *[_type == "book" && author->name == "Ada"]}"#,
        );
    }

    #[test]
    fn join_in_filter_after_flatten_fires() {
        assert_fires(r#"*[_type == "book"][][author->name == "x"][0...10]"#);
    }

    #[test]
    fn join_in_array_filter_inside_dataset_filter_fires() {
        assert_fires(r#"*[_type == "book" && count(reviews[@->rating > 3]) > 0][0...10]"#);
    }

    #[test]
    fn finding_spans_the_join() {
        let query = r#"*[author->name == "Ada"][0...10]"#;
        let spans: Vec<&str> = lint(query)
            .unwrap()
            .into_iter()
            .filter(|f| f.rule_id == "join_in_filter")
            .map(|f| &query[f.span.start..f.span.end])
            .collect();
        assert_eq!(spans, vec!["author->"]);
    }

    #[test]
    fn join_in_attribute_array_filter_in_projection_is_ok() {
        assert_ok(r#"*[_type == "book"][0...10]{"r": reviews[@->title == "x"]}"#);
        assert_ok(r#"*[_type == "book"][0...10]{"r": reviewsInline[@->name == "x"]}"#);
    }

    #[test]
    fn advice_suggests_index_served_rewrite() {
        let findings = lint(r#"*[author->name == "Ada"][0...10]"#).unwrap();
        let finding = findings
            .iter()
            .find(|f| f.rule_id == "join_in_filter")
            .unwrap();
        assert!(finding.message.contains(r#"a._ref in *[b == "c"]._id"#));
        assert!(!finding.message.contains("count("));
    }

    #[test]
    fn suggested_rewrite_has_no_join_in_filter() {
        assert_ok(
            r#"*[_type == "book" && author._ref in *[_type == "author" && name == "Ada"]._id][0...10]"#,
        );
    }
}
