//! Semantic IR for GROQ queries.
//!
//! This module provides a higher-level representation of GROQ queries that captures
//! semantic information like scope, data flow, and traversal semantics. This makes
//! it easier to write lint rules that reason about query behavior rather than syntax.

mod lower;
mod node;
mod provenance;
mod scope;

pub use lower::lower;
pub use node::{BinaryOp, LiteralValue, Node, NodeKind, ObjectField, ProjectionField, UnaryOp};
pub use provenance::Provenance;
pub use scope::{Scope, ScopeKind};

/// Unique identifier for IR nodes within a lowered query.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub u32);

/// Unique identifier for scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ScopeId(pub u32);

/// Source location information preserved from AST.
#[derive(Debug, Clone, Copy)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl From<groq_parser::ast::Position> for Span {
    fn from(pos: groq_parser::ast::Position) -> Self {
        Self {
            start: pos.start,
            end: pos.end,
        }
    }
}

impl From<Span> for crate::rules::Span {
    fn from(span: Span) -> Self {
        crate::rules::Span {
            start: span.start,
            end: span.end,
        }
    }
}

/// The complete lowered IR for a query.
#[derive(Debug)]
pub struct IrGraph {
    /// All nodes in the graph.
    nodes: Vec<Node>,
    /// All scopes.
    scopes: Vec<Scope>,
    /// The root expression node.
    root: NodeId,
    /// Query-level metadata.
    pub query_len: usize,
}

impl IrGraph {
    /// Get a node by ID.
    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id.0 as usize]
    }

    /// Get a scope by ID.
    pub fn scope(&self, id: ScopeId) -> &Scope {
        &self.scopes[id.0 as usize]
    }

    /// Get the root node.
    pub fn root(&self) -> &Node {
        self.node(self.root)
    }

    /// Get the root node ID.
    pub fn root_id(&self) -> NodeId {
        self.root
    }

    /// Iterate over all nodes.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.nodes.iter()
    }

    /// Returns all ancestors of a scope (parent chain), from immediate parent to root.
    pub fn scope_ancestors(&self, id: ScopeId) -> impl Iterator<Item = &Scope> {
        ScopeAncestorIter {
            graph: self,
            current: self.scope(id).parent,
        }
    }

    /// Check if a node is within a scope of given kind (including ancestors).
    pub fn in_scope_kind(&self, node_id: NodeId, kind: ScopeKind) -> bool {
        let node = self.node(node_id);
        let scope = self.scope(node.scope);

        if scope.kind == kind {
            return true;
        }

        self.scope_ancestors(node.scope).any(|s| s.kind == kind)
    }

    /// Check if a node is inside a filter scope.
    pub fn in_filter(&self, node: &Node) -> bool {
        self.in_scope_kind(node.id, ScopeKind::Filter)
    }

    /// Check if a node is inside a projection scope.
    pub fn in_projection(&self, node: &Node) -> bool {
        self.in_scope_kind(node.id, ScopeKind::Projection)
    }

    /// Find all nodes matching a predicate.
    pub fn query<F>(&self, predicate: F) -> impl Iterator<Item = &Node>
    where
        F: Fn(&Node) -> bool,
    {
        self.nodes.iter().filter(move |n| predicate(n))
    }

    /// Find all nodes of a specific kind.
    pub fn nodes_by_kind<F>(&self, filter: F) -> impl Iterator<Item = &Node>
    where
        F: Fn(&NodeKind) -> bool,
    {
        self.nodes.iter().filter(move |n| filter(&n.kind))
    }

    /// Find all Join operations (dereferences).
    pub fn joins(&self) -> impl Iterator<Item = &Node> {
        self.nodes_by_kind(|k| matches!(k, NodeKind::Join { .. }))
    }

    /// Find all Filter operations.
    pub fn filters(&self) -> impl Iterator<Item = &Node> {
        self.nodes_by_kind(|k| matches!(k, NodeKind::Filter { .. }))
    }

    /// Find all Projection operations.
    pub fn projections(&self) -> impl Iterator<Item = &Node> {
        self.nodes_by_kind(|k| matches!(k, NodeKind::Projection { .. }))
    }

    /// Find all function calls.
    pub fn function_calls(&self) -> impl Iterator<Item = &Node> {
        self.nodes_by_kind(|k| matches!(k, NodeKind::FunctionCall { .. }))
    }

    /// Find all binary operations.
    pub fn binary_ops(&self) -> impl Iterator<Item = &Node> {
        self.nodes_by_kind(|k| matches!(k, NodeKind::Binary { .. }))
    }

    /// Count nodes matching a predicate.
    pub fn count_where<F>(&self, pred: F) -> usize
    where
        F: Fn(&Node) -> bool,
    {
        self.nodes.iter().filter(|n| pred(n)).count()
    }

    /// Get the base of a traversal (if any).
    pub fn traversal_base(&self, node: &Node) -> Option<&Node> {
        match &node.kind {
            NodeKind::Access { base, .. }
            | NodeKind::Element { base, .. }
            | NodeKind::Filter { base, .. }
            | NodeKind::Projection { base, .. }
            | NodeKind::Join { base }
            | NodeKind::Slice { base, .. }
            | NodeKind::ArrayCoerce { base } => Some(self.node(*base)),
            _ => None,
        }
    }

    /// Walk ancestors of a node (following base pointers up the traversal chain).
    pub fn ancestors(&self, node: &Node) -> impl Iterator<Item = &Node> {
        std::iter::successors(self.traversal_base(node), |n| self.traversal_base(n))
    }

    /// Iterate over all descendants of a node (depth-first).
    pub fn descendants(&self, node_id: NodeId) -> impl Iterator<Item = &Node> {
        DescendantIter::new(self, node_id)
    }
}

/// Iterator over scope ancestors.
struct ScopeAncestorIter<'a> {
    graph: &'a IrGraph,
    current: Option<ScopeId>,
}

impl<'a> Iterator for ScopeAncestorIter<'a> {
    type Item = &'a Scope;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.current?;
        let scope = self.graph.scope(id);
        self.current = scope.parent;
        Some(scope)
    }
}

/// Iterator for depth-first traversal of descendants.
struct DescendantIter<'a> {
    graph: &'a IrGraph,
    stack: Vec<NodeId>,
}

impl<'a> DescendantIter<'a> {
    fn new(graph: &'a IrGraph, root: NodeId) -> Self {
        let mut stack = Vec::new();
        Self::push_children(graph, root, &mut stack);
        Self { graph, stack }
    }

    fn push_children(graph: &'a IrGraph, node_id: NodeId, stack: &mut Vec<NodeId>) {
        let node = graph.node(node_id);
        match &node.kind {
            NodeKind::Access { base, .. } => {
                stack.push(*base);
            }
            NodeKind::Element { base, index } => {
                stack.push(*index);
                stack.push(*base);
            }
            NodeKind::Filter {
                base, predicate, ..
            } => {
                stack.push(*predicate);
                stack.push(*base);
            }
            NodeKind::Slice {
                base, start, end, ..
            } => {
                if let Some(e) = end {
                    stack.push(*e);
                }
                if let Some(s) = start {
                    stack.push(*s);
                }
                stack.push(*base);
            }
            NodeKind::ArrayCoerce { base } => {
                stack.push(*base);
            }
            NodeKind::Projection { base, fields, .. } => {
                for field in fields.iter().rev() {
                    stack.push(field.value);
                }
                stack.push(*base);
            }
            NodeKind::Join { base } => {
                stack.push(*base);
            }
            NodeKind::Binary { lhs, rhs, .. } => {
                stack.push(*rhs);
                stack.push(*lhs);
            }
            NodeKind::Unary { operand, .. } => {
                stack.push(*operand);
            }
            NodeKind::FunctionCall {
                args, pipe_base, ..
            } => {
                for arg in args.iter().rev() {
                    stack.push(*arg);
                }
                if let Some(base) = pipe_base {
                    stack.push(*base);
                }
            }
            NodeKind::ArrayLiteral { elements } => {
                for elem in elements.iter().rev() {
                    stack.push(*elem);
                }
            }
            NodeKind::ObjectLiteral { fields } => {
                for field in fields.iter().rev() {
                    stack.push(field.value);
                    stack.push(field.key);
                }
            }
            NodeKind::Range { start, end, .. } => {
                stack.push(*end);
                stack.push(*start);
            }
            // Leaf nodes have no children
            NodeKind::Literal(_)
            | NodeKind::Param { .. }
            | NodeKind::This
            | NodeKind::Parent { .. }
            | NodeKind::Dataset => {}
        }
    }
}

impl<'a> Iterator for DescendantIter<'a> {
    type Item = &'a Node;

    fn next(&mut self) -> Option<Self::Item> {
        let id = self.stack.pop()?;
        let node = self.graph.node(id);
        Self::push_children(self.graph, id, &mut self.stack);
        Some(node)
    }
}
