use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct NonLiteralComparison;

impl Rule for NonLiteralComparison {
    fn id(&self) -> &'static str {
        "non_literal_comparison"
    }

    fn name(&self) -> &'static str {
        "Non-Literal Comparison"
    }

    fn description(&self) -> &'static str {
        "Comparisons between two non-literal fields cannot use indices."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Binary(bin) = expr {
            match bin.operator {
                Token::Equals | Token::NEQ | Token::LT | Token::LTE | Token::GT | Token::GTE => {
                    if !is_literal(&bin.lhs) && !is_literal(&bin.rhs) {
                        findings.push(Finding {
                            span: bin.pos.into(),
                            message: self.description().to_string(),
                            severity: Severity::Medium,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                }
                _ => {}
            }
        }

        findings
    }
}

fn is_literal(expr: &Expr) -> bool {
    match expr {
        Expr::Literal(_) => true,
        Expr::Parent(_) => false,
        Expr::This(_) => false,
        Expr::Param(_) => true,
        _ => false,
    }
}
