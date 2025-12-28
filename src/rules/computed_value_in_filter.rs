use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct ComputedValueInFilter;

impl Rule for ComputedValueInFilter {
    fn id(&self) -> &'static str {
        "computed_value_in_filter"
    }

    fn name(&self) -> &'static str {
        "Computed Value in Filter"
    }

    fn description(&self) -> &'static str {
        "Avoid computed values (concatenation, arithmetic, etc.) in filters. Indices cannot be used."
    }

    fn visit(&self, expr: &Expr, context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if context.in_filter {
            if let Expr::Binary(bin) = expr {
                match bin.operator {
                    Token::Plus
                    | Token::Minus
                    | Token::Asterisk
                    | Token::Slash
                    | Token::Percent
                    | Token::Exponentiation => {
                        findings.push(Finding {
                            span: bin.pos.into(),
                            message: self.description().to_string(),
                            severity: Severity::High,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                    _ => {}
                }
            }
        }

        findings
    }
}
