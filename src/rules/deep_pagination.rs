use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Literal};

pub struct DeepPagination;

const PAGINATION_THRESHOLD: i64 = 1000;

impl Rule for DeepPagination {
    fn id(&self) -> &'static str {
        "deep_pagination"
    }

    fn name(&self) -> &'static str {
        "Deep Pagination"
    }

    fn description(&self) -> &'static str {
        "Deep pagination is slow. Consider using cursor-based pagination (e.g., using _id)."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Range(range) = expr {
            if let Expr::Literal(Literal::Integer(i)) = &*range.start {
                if i.value > PAGINATION_THRESHOLD {
                    findings.push(Finding {
                        span: range.pos.into(),
                        message: self.description().to_string(),
                        severity: Severity::Medium,
                        rule_id: self.id().to_string(),
                        scope: Scope::Node,
                    });
                }
            }
        } else if let Expr::Slice(_slice) = expr {
            // Slice logic mirrors range mostly, but slice.range.value is the Range expr.
            // Usually Range covers it.
        }

        findings
    }
}
