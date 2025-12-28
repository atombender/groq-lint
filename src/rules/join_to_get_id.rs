use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct JoinToGetId;

impl Rule for JoinToGetId {
    fn id(&self) -> &'static str {
        "join_to_get_id"
    }

    fn name(&self) -> &'static str {
        "Join to get ID"
    }

    fn description(&self) -> &'static str {
        "Avoid using a dereference operator (->) to retrieve the _id of a document."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        // Pattern: a->._id or a->_id (which parses as Postfix(->).Dot(_id))
        // Actually a->b parses as Dot(Postfix(->), b).
        // So we look for Dot expression where LHS is Postfix(->) and RHS is Attribute("_id").

        if let Expr::Dot(dot) = expr {
            if let Expr::Attribute(attr) = &*dot.rhs {
                if attr.name == "_id" {
                    if let Expr::Postfix(pf) = &*dot.lhs {
                        if pf.operator == Token::Arrow {
                            findings.push(Finding {
                                span: dot.pos.into(),
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
