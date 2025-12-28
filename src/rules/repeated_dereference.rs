use crate::rules::{get_pos, Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};
use std::collections::HashSet;

pub struct RepeatedDereference;

impl Rule for RepeatedDereference {
    fn id(&self) -> &'static str {
        "repeated_dereference"
    }

    fn name(&self) -> &'static str {
        "Repeated Dereference"
    }

    fn description(&self) -> &'static str {
        "Repeatedly resolving the same reference is inefficient. Consider a single sub-projection."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::Object(obj) = expr {
            let mut dereferenced_fields = HashSet::new();

            for field in &obj.expressions {
                let value_expr = match field {
                    Expr::Binary(bin) if bin.operator == Token::Colon => &bin.rhs,
                    _ => field,
                };

                if let Some(attr_name) = get_dereferenced_attribute(value_expr) {
                    if dereferenced_fields.contains(&attr_name) {
                        findings.push(Finding {
                            span: get_pos(value_expr).into(),
                            message: self.description().to_string(),
                            severity: Severity::Low,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    } else {
                        dereferenced_fields.insert(attr_name);
                    }
                }
            }
        }

        findings
    }
}

fn get_dereferenced_attribute(expr: &Expr) -> Option<String> {
    match expr {
        Expr::Postfix(pf) if pf.operator == Token::Arrow => {
            if let Expr::Attribute(attr) = &*pf.lhs {
                return Some(attr.name.clone());
            }
        }
        Expr::Dot(dot) => {
            return get_dereferenced_attribute(&dot.lhs);
        }
        Expr::Projection(proj) => {
            return get_dereferenced_attribute(&proj.lhs);
        }
        _ => {}
    }
    None
}
