//! IR-based rules for pagination issues.
//!
//! Only slices and element accesses of a document stream (`*[...]`) are
//! pagination. A slice of one document's attribute array
//! (`reviews[1500..1600]`) is taken in memory.

use crate::ir::{IrGraph, Node, NodeId};
use crate::ir::{LiteralValue, NodeKind};
use crate::rules::{Hit, Rule};

const DEEP_PAGINATION_START: i64 = 1000;
const LARGE_PAGE_END: i64 = 100;

/// The start of each slice, or the index of each element access, on a document stream.
fn document_offsets(graph: &IrGraph) -> impl Iterator<Item = (&Node, NodeId)> {
    graph.nodes().filter_map(move |node| match &node.kind {
        NodeKind::Slice {
            base,
            start: Some(start),
            ..
        } if graph.is_document_stream(*base) => Some((node, *start)),
        NodeKind::Element { base, index } if graph.is_document_stream(*base) => {
            Some((node, *index))
        }
        _ => None,
    })
}

/// Detects deep pagination (start index >= 1000) on a document stream.
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
        document_offsets(graph)
            .filter(|(_, offset)| {
                matches!(graph.node(*offset).kind,
                    NodeKind::Literal(LiteralValue::Int(start)) if start >= DEEP_PAGINATION_START)
            })
            .map(|(node, _)| Hit::at(node.span))
            .collect()
    }
}

/// Detects a `$param` start index on a document stream.
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
        document_offsets(graph)
            .filter(|(_, offset)| matches!(graph.node(*offset).kind, NodeKind::Param { .. }))
            .map(|(node, _)| Hit::at(node.span))
            .collect()
    }
}

/// Detects large page fetches (> 100 documents starting from 0) on a document stream.
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
        graph
            .nodes()
            .filter(|node| {
                let NodeKind::Slice {
                    base, start, end, ..
                } = &node.kind
                else {
                    return false;
                };
                let starts_at_zero = start.is_none_or(|start| {
                    matches!(
                        graph.node(start).kind,
                        NodeKind::Literal(LiteralValue::Int(0))
                    )
                });
                let large_end = end.is_some_and(|end| {
                    matches!(graph.node(end).kind,
                        NodeKind::Literal(LiteralValue::Int(end)) if end > LARGE_PAGE_END)
                });
                starts_at_zero && large_end && graph.is_document_stream(*base)
            })
            .map(|node| Hit::at(node.span))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use crate::lint;

    const DEEP: &str = "deep_pagination";
    const PARAM: &str = "deep_pagination_param";
    const LARGE: &str = "large_pages";

    fn spans<'a>(rule_id: &str, query: &'a str) -> Vec<&'a str> {
        lint(query)
            .expect("query should parse")
            .into_iter()
            .filter(|f| f.rule_id == rule_id)
            .map(|f| &query[f.span.start..f.span.end])
            .collect()
    }

    #[track_caller]
    fn assert_fires(rule_id: &str, query: &str) {
        assert!(
            !spans(rule_id, query).is_empty(),
            "expected {rule_id} to fire on: {query}"
        );
    }

    #[track_caller]
    fn assert_ok(rule_id: &str, query: &str) {
        let hits = spans(rule_id, query);
        assert!(
            hits.is_empty(),
            "expected {rule_id} NOT to fire on: {query}, got {hits:?}"
        );
    }

    #[test]
    fn deep_slice_of_documents_fires() {
        assert_fires(DEEP, r#"*[_type == "book"][1500...1600]"#);
        assert_fires(DEEP, r#"*[_type == "book"] | order(title)[2000...2010]"#);
        assert_fires(DEEP, r#"*[_type == "book"]{title}[1000...1010]"#);
    }

    #[test]
    fn deep_slice_spans_the_slice() {
        assert_eq!(
            spans(DEEP, r#"*[_type == "book"][1500...1600]"#),
            vec!["[1500...1600]"]
        );
    }

    #[test]
    fn deep_slice_of_flattened_documents_fires() {
        assert_fires(DEEP, "*[][1000...1010]");
        assert_fires(LARGE, "*[][0...200]");
        assert_ok(
            DEEP,
            r#"*[_type == "book"][0...10]{"r": reviews[][1500...1600]}"#,
        );
    }

    #[test]
    fn start_of_exactly_1000_fires() {
        assert_fires(DEEP, r#"*[_type == "book"][1000...1010]"#);
        assert_ok(DEEP, r#"*[_type == "book"][999...1010]"#);
    }

    #[test]
    fn deep_element_access_fires() {
        assert_eq!(
            spans(DEEP, r#"*[_type == "book"][5000]{title}"#),
            vec!["[5000]"]
        );
        assert_ok(DEEP, r#"*[_type == "book"][0]"#);
    }

    #[test]
    fn slice_of_attribute_array_is_ok() {
        assert_ok(
            DEEP,
            r#"*[_type == "book"][0...10]{"r": reviewsInline[1500..1600]}"#,
        );
        assert_ok(
            DEEP,
            r#"*[_type == "book"][0...10]{"r": reviewsInline[200000...200010]}"#,
        );
        assert_ok(
            DEEP,
            r#"*[_type == "book"][0...10]{"r": reviews[1500..1600]->title}"#,
        );
        assert_ok(DEEP, r#"*[_type == "book"][0...10]{"r": reviews[5000]}"#);
    }

    #[test]
    fn range_outside_a_slice_is_ok() {
        assert_ok(
            DEEP,
            r#"*[_type == "book" && rating in 2000..3000][0...10]"#,
        );
        assert_ok(PARAM, r#"*[_type == "book" && rating in $min..10][0...10]"#);
        assert_ok(LARGE, r#"*[_type == "book" && rating in 0..200][0...10]"#);
    }

    #[test]
    fn param_slice_of_documents_fires() {
        assert_fires(PARAM, r#"*[_type == "book"][$start...$end]"#);
        assert_eq!(
            spans(PARAM, r#"*[_type == "book"][$start]"#),
            vec!["[$start]"]
        );
    }

    #[test]
    fn param_slice_of_attribute_array_is_ok() {
        assert_ok(
            PARAM,
            r#"*[_type == "book"][0...10]{"r": reviewsInline[$start...$end]}"#,
        );
        assert_ok(
            PARAM,
            r#"*[_type == "book"][0...10]{"r": reviewsInline[$start]}"#,
        );
    }

    #[test]
    fn large_page_of_documents_fires() {
        assert_fires(LARGE, r#"*[_type == "book"][0...101]"#);
        assert_fires(LARGE, r#"*[_type == "book"]{title}[0..200]"#);
        assert_ok(LARGE, r#"*[_type == "book"][0...100]"#);
    }

    #[test]
    fn large_slice_of_attribute_array_is_ok() {
        assert_ok(
            LARGE,
            r#"*[_type == "book"][0...10]{"r": reviewsInline[0..200]}"#,
        );
    }
}
