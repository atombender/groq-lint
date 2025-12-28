use crate::rules::{Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::Expr;

pub struct CountInCorrelatedSubquery;

impl Rule for CountInCorrelatedSubquery {
    fn id(&self) -> &'static str {
        "count_in_correlated_subquery"
    }

    fn name(&self) -> &'static str {
        "Count in Correlated Subquery"
    }

    fn description(&self) -> &'static str {
        "Using count() on a correlated subquery does not execute as an efficient aggregation."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::FunctionCall(func) = expr {
            if func.name == "count" {
                // Check argument for correlated subquery
                for arg in &func.arguments {
                    if is_correlated(arg) {
                        findings.push(Finding {
                            span: func.pos.into(),
                            message: self.description().to_string(),
                            severity: Severity::Low,
                            rule_id: self.id().to_string(),
                            scope: Scope::Node,
                        });
                    }
                }
            }
        }

        findings
    }
}

fn is_correlated(expr: &Expr) -> bool {
    // Check if expression uses parent scope `^`
    // This requires recursive traversal.
    // Since `visit` is shallow, we need a helper walker.
    let mut correlated = false;
    visit_recursive(expr, &mut |e| {
        if let Expr::Parent(_) = e {
            correlated = true;
        }
    });
    correlated
}

fn visit_recursive<F>(expr: &Expr, cb: &mut F)
where
    F: FnMut(&Expr),
{
    cb(expr);
    // Naive recursion for common types relevant to filtering
    match expr {
        Expr::Filter(f) => {
            visit_recursive(&f.lhs, cb);
            visit_recursive(&f.constraint.expression, cb);
        }
        Expr::Binary(b) => {
            visit_recursive(&b.lhs, cb);
            visit_recursive(&b.rhs, cb);
        }
        Expr::Dot(d) => {
            visit_recursive(&d.lhs, cb);
            visit_recursive(&d.rhs, cb);
        }
        // ... Add more if needed, but Parent is usually in filter constraint
        _ => {}
    }
}
