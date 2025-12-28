use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct JoinInFilter;

impl Rule for JoinInFilter {
    fn id(&self) -> &'static str {
        "join_in_filter"
    }

    fn name(&self) -> &'static str {
        "Join in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid joins (->) inside filters. It prevents optimization."
    }

    fn visit(&self, expr: &Expr, context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if context.in_filter {
            if let Expr::Postfix(node) = expr {
                if node.operator == Token::Arrow {
                    findings.push(Finding {
                        span: node.pos.into(),
                        message: self.description().to_string(),
                        severity: Severity::High,
                        rule_id: self.id().to_string(),
                        scope: Scope::Node,
                    });
                }
            }
        }

        findings
    }
}
