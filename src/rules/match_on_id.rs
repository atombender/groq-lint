use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Literal, Token};

pub struct MatchOnId;

impl Rule for MatchOnId {
    fn id(&self) -> &'static str {
        "match_on_id"
    }

    fn name(&self) -> &'static str {
        "Match on ID"
    }

    fn description(&self) -> &'static str {
        "`match` is intended for full-text matching and may not work as expected on _id."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Binary(bin) = expr {
            if bin.operator == Token::MatchOperator {
                // Check LHS is _id
                if let Expr::Attribute(attr) = &*bin.lhs {
                    if attr.name == "_id" {
                        // Check RHS is string with wildcard
                        if let Expr::Literal(Literal::String(s)) = &*bin.rhs {
                            if s.value.contains('*') {
                                findings.push(Finding {
                                    span: bin.pos.into(),
                                    message: self.description().to_string(),
                                    severity: Severity::Low,
                                    rule_id: self.id().to_string(),
                                    scope: Scope::Node,
                                });
                            }
                        }
                    }
                }
            }
        }

        findings
    }
}
