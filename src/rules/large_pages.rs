use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Literal};

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

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Range(range) = expr {
            if let Expr::Literal(Literal::Integer(start)) = &*range.start {
                if start.value == 0 {
                    if let Expr::Literal(Literal::Integer(end)) = &*range.end {
                        if end.value > 100 {
                            findings.push(Finding {
                                span: range.pos.into(),
                                message: self.description().to_string(),
                                severity: Severity::Medium,
                                rule_id: self.id().to_string(),
                                scope: Scope::Node,
                            });
                        }
                    }
                }
            }
        }

        findings
    }
}
