use crate::rules::{get_pos, Context, Finding, Rule, Scope, Severity};
use groq_parser::ast::{Expr, Token};

pub struct OrderOnExpr;

impl Rule for OrderOnExpr {
    fn id(&self) -> &'static str {
        "order_on_expr"
    }

    fn name(&self) -> &'static str {
        "Order on Expression"
    }

    fn description(&self) -> &'static str {
        "Avoid ordering on computed values. Indices cannot be used to sort."
    }

    fn visit(&self, expr: &Expr, _context: &Context) -> Vec<Finding> {
        let mut findings = vec![];

        if let Expr::FunctionCall(func) = expr {
            if func.name == "order" {
                for arg in &func.arguments {
                    // Argument can be `expr` or `expr asc/desc`
                    let mut check_expr = arg;
                    if let Expr::Postfix(pf) = arg {
                        if pf.operator == Token::AscOperator || pf.operator == Token::DescOperator {
                            check_expr = &pf.lhs;
                        }
                    }

                    if !is_allowed_order_expr(check_expr) {
                        findings.push(Finding {
                            span: get_pos(arg).into(),
                            message: self.description().to_string(),
                            severity: Severity::High,
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

fn is_allowed_order_expr(expr: &Expr) -> bool {
    match expr {
        Expr::Attribute(_) => true,
        Expr::FunctionCall(func) => match func.name.as_str() {
            "lower" | "dateTime" => {
                if func.arguments.len() == 1 {
                    is_allowed_order_expr(&func.arguments[0])
                } else {
                    false
                }
            }
            _ => false,
        },
        _ => false,
    }
}
