use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::Expr;

pub struct DeepPaginationParam;

impl Rule for DeepPaginationParam {
    fn id(&self) -> &'static str {
        "deep_pagination_param"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination Param"
    }

    fn description(&self) -> &'static str {
        "Range or slice start index is a $param. If large, this causes deep pagination."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Range(range) = expr {
            if let Expr::Param(_) = &*range.start {
                findings.push(Finding {
                    span: range.pos.into(),
                    message: self.description().to_string(),
                    severity: Severity::Medium,
                    rule_id: self.id().to_string(),
                    scope: Scope::Node,
                });
            }
        }

        findings
    }
}
