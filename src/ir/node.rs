//! Node types for the semantic IR.

use super::{NodeId, Provenance, ScopeId, Span};

/// A node in the semantic IR.
#[derive(Debug, Clone)]
pub struct Node {
    pub id: NodeId,
    pub span: Span,
    pub scope: ScopeId,
    pub kind: NodeKind,
    pub provenance: Provenance,
}

/// The kind of IR node.
///
/// This is a semantic representation that collapses multiple AST variants
/// into fewer, more meaningful categories.
#[derive(Debug, Clone)]
pub enum NodeKind {
    //--- Value Sources ---
    /// Literal value (null, bool, number, string).
    Literal(LiteralValue),
    /// Parameter reference (`$param`).
    Param { name: String },
    /// Current scope value (`@`, implicit this).
    This,
    /// Parent scope reference (`^`).
    Parent { depth: u32 },
    /// Dataset reference (`*`).
    Dataset,

    //--- Access Operations ---
    /// Attribute access (`base.attr` or `base["attr"]`).
    Access { base: NodeId, attribute: String },
    /// Element access (`base[index]`).
    Element { base: NodeId, index: NodeId },

    //--- Collection Operations ---
    /// Filter operation (`base[predicate]`).
    Filter {
        base: NodeId,
        predicate: NodeId,
        /// The scope introduced for the predicate.
        inner_scope: ScopeId,
    },
    /// Slice operation (`base[start..end]` or `base[start...end]`).
    Slice {
        base: NodeId,
        start: Option<NodeId>,
        end: Option<NodeId>,
        inclusive: bool,
    },
    /// Array coercion (`[]`).
    ArrayCoerce { base: NodeId },
    /// Projection (`{...}`).
    Projection {
        base: NodeId,
        fields: Vec<ProjectionField>,
        /// The scope introduced for field expressions.
        inner_scope: ScopeId,
    },

    //--- Join/Dereference ---
    /// Join/dereference operation (`->`).
    Join { base: NodeId },

    //--- Operators ---
    /// Binary operation.
    Binary {
        op: BinaryOp,
        lhs: NodeId,
        rhs: NodeId,
    },
    /// Unary operation.
    Unary { op: UnaryOp, operand: NodeId },

    //--- Function Calls ---
    FunctionCall {
        namespace: Option<String>,
        name: String,
        args: Vec<NodeId>,
        /// Pipe-style call (`base | func()`).
        pipe_base: Option<NodeId>,
    },

    //--- Structural ---
    /// Array literal `[a, b, c]`.
    ArrayLiteral { elements: Vec<NodeId> },
    /// Object literal `{a: b, c: d}`.
    ObjectLiteral { fields: Vec<ObjectField> },
    /// Range expression `a..b` or `a...b`.
    Range {
        start: NodeId,
        end: NodeId,
        inclusive: bool,
    },
}

/// A field in a projection.
#[derive(Debug, Clone)]
pub struct ProjectionField {
    /// The key name. None for spread (`...`).
    pub key: Option<String>,
    /// The value expression.
    pub value: NodeId,
    /// Whether this is a spread operator.
    pub is_spread: bool,
}

/// A field in an object literal.
#[derive(Debug, Clone)]
pub struct ObjectField {
    /// The key expression (can be dynamic).
    pub key: NodeId,
    /// The value expression.
    pub value: NodeId,
}

/// Literal values.
#[derive(Debug, Clone, PartialEq)]
pub enum LiteralValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    // Logical
    And,
    Or,
    // Comparison
    Eq,
    Neq,
    Lt,
    Lte,
    Gt,
    Gte,
    // Containment
    In,
    Match,
    // Arithmetic
    Add,
    Sub,
    Mul,
    Div,
    Mod,
    Pow,
    // Order modifiers (used in order())
    Asc,
    Desc,
    // Conditional/fat arrow (=> used in select() and conditional object entries)
    FatArrow,
}

impl BinaryOp {
    /// Returns true if this is an arithmetic operator.
    pub fn is_arithmetic(&self) -> bool {
        matches!(
            self,
            BinaryOp::Add
                | BinaryOp::Sub
                | BinaryOp::Mul
                | BinaryOp::Div
                | BinaryOp::Mod
                | BinaryOp::Pow
        )
    }

    /// Returns true if this is a comparison operator.
    pub fn is_comparison(&self) -> bool {
        matches!(
            self,
            BinaryOp::Eq
                | BinaryOp::Neq
                | BinaryOp::Lt
                | BinaryOp::Lte
                | BinaryOp::Gt
                | BinaryOp::Gte
        )
    }

    /// Returns true if this is a logical operator.
    pub fn is_logical(&self) -> bool {
        matches!(self, BinaryOp::And | BinaryOp::Or)
    }
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Not,
    Neg,
    Pos,
}
