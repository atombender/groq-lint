//! IR-based rule for detecting repeated dereferences on the same attribute.

use std::collections::HashSet;

use crate::ir::NodeKind;
use crate::ir::{IrGraph, NodeId};
use crate::rules::{Finding, IrRule, Scope, Severity, Span};

/// Detects multiple dereferences on the same attribute within a projection.
pub struct IrRepeatedDereference;

impl IrRule for IrRepeatedDereference {
    fn id(&self) -> &'static str {
        "repeated_dereference"
    }

    fn name(&self) -> &'static str {
        "Repeated Dereference"
    }

    fn description(&self) -> &'static str {
        "Repeatedly resolving the same reference is inefficient. Consider a single sub-projection."
    }

    fn check(&self, graph: &IrGraph) -> Vec<Finding> {
        let mut findings = vec![];

        // Find all projections
        for node in graph.projections() {
            if let NodeKind::Projection { fields, .. } = &node.kind {
                let mut dereferenced_attrs: HashSet<String> = HashSet::new();

                for field in fields {
                    // Get the dereferenced attribute name and join node from the field value
                    if let Some((attr_name, join_node_id)) =
                        get_dereferenced_attribute(graph, field.value)
                    {
                        if dereferenced_attrs.contains(&attr_name) {
                            // Found repeated dereference - report the join node span
                            let join_node = graph.node(join_node_id);
                            findings.push(Finding {
                                span: Span::from(join_node.span),
                                message: self.advice().to_string(),
                                severity: Severity::Low,
                                rule_id: self.id().to_string(),
                                scope: Scope::Node,
                            });
                        } else {
                            dereferenced_attrs.insert(attr_name);
                        }
                    }
                }
            }
        }

        findings
    }
}

/// Get the name of the attribute being dereferenced and the join node, if any.
/// Handles patterns like: author->, author->name, author->{...}
/// Returns (attribute_name, join_node_id) for reporting purposes.
fn get_dereferenced_attribute(graph: &IrGraph, node_id: NodeId) -> Option<(String, NodeId)> {
    let node = graph.node(node_id);

    match &node.kind {
        // Direct join: author->
        NodeKind::Join { base } => {
            let base_node = graph.node(*base);
            if let NodeKind::Access { attribute, .. } = &base_node.kind {
                return Some((attribute.clone(), node.id));
            }
        }
        // Access after join: author->name
        NodeKind::Access { base, .. } => {
            return get_dereferenced_attribute(graph, *base);
        }
        // Projection after join: author->{...}
        NodeKind::Projection { base, .. } => {
            return get_dereferenced_attribute(graph, *base);
        }
        _ => {}
    }

    None
}
