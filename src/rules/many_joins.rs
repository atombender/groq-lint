use crate::rules::{get_pos, Context, Finding, Rule, RuleContext, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct ManyJoins;

impl Rule for ManyJoins {
    fn id(&self) -> &'static str {
        "many_joins"
    }

    fn name(&self) -> &'static str {
        "Many Joins"
    }

    fn description(&self) -> &'static str {
        "The query uses more than 10 dereference operators (->)."
    }

    fn context(&self) -> RuleContext {
        RuleContext::WholeQuery
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];
        let mut count = 0;

        count_joins(expr, &mut count);

        if count > 10 {
            findings.push(Finding {
                span: get_pos(expr).into(),
                message: self.description().to_string(),
                severity: Severity::Medium,
                rule_id: self.id().to_string(),
                scope: Scope::Global,
            });
        }
        findings
    }
}

fn count_joins(expr: &Expr, count: &mut usize) {
    if let Expr::Postfix(pf) = expr {
        if pf.operator == Token::Arrow {
            *count += 1;
        }
    }

    match expr {
        Expr::Binary(b) => {
            count_joins(&b.lhs, count);
            count_joins(&b.rhs, count);
        }
        Expr::Filter(f) => {
            count_joins(&f.lhs, count);
            count_joins(&f.constraint.expression, count);
        }
        Expr::Projection(p) => {
            count_joins(&p.lhs, count);
            for e in &p.object.expressions {
                count_joins(e, count);
            }
        }
        Expr::Postfix(p) => count_joins(&p.lhs, count),
        Expr::Dot(d) => {
            count_joins(&d.lhs, count);
            count_joins(&d.rhs, count);
        }
        _ => {}
    }
}
