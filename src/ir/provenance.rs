//! Data flow provenance tracking for the semantic IR.

use super::NodeId;

/// Describes where a value originates.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Provenance {
    /// Value comes from current scope's `this` (`@` or implicit).
    This,
    /// Value comes from parent scope (`^`, `^^`, etc.).
    Parent { depth: u32 },
    /// Value comes from a parameter (`$param`).
    Param { name: String },
    /// Value is a literal constant.
    Literal,
    /// Value comes from the dataset (`*`).
    Dataset,
    /// Value comes from a join/dereference operation.
    Join { target: NodeId },
    /// Value is computed from other values.
    Computed { sources: Vec<NodeId> },
    /// Unknown provenance (conservative default).
    Unknown,
}
