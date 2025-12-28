use groq_parser::ast::{Expr, Position};
use serde::Serialize;

use crate::ir::IrGraph;

#[derive(Debug, Clone, Serialize)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

impl From<Position> for Span {
    fn from(pos: Position) -> Self {
        Self {
            start: pos.start,
            end: pos.end,
        }
    }
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    Node,
    Global,
}

#[derive(Debug, Clone, Serialize)]
pub struct Finding {
    pub span: Span,
    pub message: String,
    pub severity: Severity,
    pub rule_id: String,
    pub scope: Scope,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuleContext {
    Expr,
    WholeQuery,
}

/// Legacy trait for AST-based rules.
/// New rules should use IrRule instead.
pub trait Rule {
    fn id(&self) -> &'static str;
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn context(&self) -> RuleContext {
        RuleContext::Expr
    }
    fn visit(&self, expr: &Expr, context: &Context) -> Vec<Finding>;
}

/// New trait for IR-based rules.
/// These rules operate on the semantic IR graph rather than the raw AST.
pub trait IrRule: Send + Sync {
    /// Unique identifier for this rule.
    fn id(&self) -> &'static str;

    /// Human-readable name for this rule.
    fn name(&self) -> &'static str;

    /// Description of the issue and how to fix it.
    fn description(&self) -> &'static str;

    /// Advice from rules.yaml on how to fix the issue.
    /// Falls back to description() if no advice is found.
    fn advice(&self) -> &'static str {
        crate::rule_meta::get_advice(self.id()).unwrap_or(self.description())
    }

    /// Check the IR graph for violations of this rule.
    fn check(&self, graph: &IrGraph) -> Vec<Finding>;

    /// Rule IDs that this rule supercedes.
    /// If this rule fires, findings from superceded rules are filtered out.
    fn supercedes(&self) -> &'static [&'static str] {
        &[]
    }
}

pub struct Context {
    pub in_filter: bool,
    pub query_len: usize,
}

pub mod computed_value_in_filter;
pub mod count_in_correlated_subquery;
pub mod deep_pagination;
pub mod deep_pagination_param;
pub mod extremely_large_query;
pub mod ir_rules;
pub mod join_in_filter;
pub mod join_to_get_id;
pub mod large_pages;
pub mod many_joins;
pub mod match_on_id;
pub mod non_literal_comparison;
pub mod order_on_expr;
pub mod repeated_dereference;
pub mod very_large_query;

pub fn get_pos(expr: &Expr) -> Position {
    match expr {
        Expr::This(e) => e.pos,
        Expr::Everything(e) => e.pos,
        Expr::Parent(e) => e.pos,
        Expr::Literal(l) => match l {
            groq_parser::ast::Literal::Integer(i) => i.pos,
            groq_parser::ast::Literal::Float(f) => f.pos,
            groq_parser::ast::Literal::String(s) => s.pos,
            groq_parser::ast::Literal::Boolean(b) => b.pos,
            groq_parser::ast::Literal::Null(n) => n.pos,
        },
        Expr::Attribute(e) => e.pos,
        Expr::Param(e) => e.pos,
        Expr::FunctionCall(e) => e.pos,
        Expr::Filter(e) => e.pos,
        Expr::Projection(e) => e.pos,
        Expr::Slice(e) => e.pos,
        Expr::Element(e) => e.pos,
        Expr::Dot(e) => e.pos,
        Expr::Postfix(e) => e.pos,
        Expr::Prefix(e) => e.pos,
        Expr::Binary(e) => e.pos,
        Expr::Range(e) => e.pos,
        Expr::Array(e) => e.pos,
        Expr::Object(e) => e.pos,
        Expr::Tuple(e) => e.pos,
        Expr::Group(e) => e.pos,
        Expr::ArrayTraversal(e) => e.pos,
        Expr::Constraint(e) => e.pos,
        Expr::FunctionPipe(e) => e.pos,
        Expr::Subscript(e) => e.pos,
        Expr::Ellipsis(e) => e.pos,
        Expr::Pipe(e) => e.pos,
    }
}
