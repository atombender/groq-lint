use groq_parser::ast::Position;
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

/// A hit returned by rules - minimal data that the rule knows.
/// The linter fills in the rest (rule_id, severity, base message) from metadata.
#[derive(Debug, Clone)]
pub struct Hit {
    pub span: Span,
    pub detail: Option<String>,
    pub scope: Scope,
}

impl Hit {
    /// Create a node-scoped hit at the given span.
    pub fn at(span: impl Into<Span>) -> Self {
        Self {
            span: span.into(),
            detail: None,
            scope: Scope::Node,
        }
    }

    /// Create a global-scoped hit at the given span.
    pub fn global(span: impl Into<Span>) -> Self {
        Self {
            span: span.into(),
            detail: None,
            scope: Scope::Global,
        }
    }

    /// Add extra detail to append to the advice message.
    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

/// Rules operate on the semantic IR graph.
pub trait Rule: Send + Sync {
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

    /// Severity from rules.yaml. Falls back to Medium if not found.
    fn severity(&self) -> Severity {
        match crate::rule_meta::get_severity(self.id()) {
            Some("high") => Severity::High,
            Some("low") => Severity::Low,
            _ => Severity::Medium,
        }
    }

    /// Check the IR graph for violations of this rule.
    /// Returns hits that the linter will convert to findings.
    fn check(&self, graph: &IrGraph) -> Vec<Hit>;

    /// Rule IDs that this rule supercedes.
    /// If this rule fires, findings from superceded rules are filtered out.
    fn supercedes(&self) -> &'static [&'static str] {
        &[]
    }
}

pub mod ir_rules;
