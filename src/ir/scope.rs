//! Scope representation for the semantic IR.

use super::{NodeId, ScopeId};

/// Represents a lexical scope in the query.
#[derive(Debug, Clone)]
pub struct Scope {
    pub id: ScopeId,
    pub kind: ScopeKind,
    pub parent: Option<ScopeId>,
    /// The node that introduced this scope.
    pub introducing_node: NodeId,
}

/// The kind of scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScopeKind {
    /// Root query scope.
    Root,
    /// Inside a filter constraint: `*[_type == "foo"]`
    Filter,
    /// Inside a projection: `{...}`
    Projection,
    /// Inside a function call argument.
    FunctionArg,
}
