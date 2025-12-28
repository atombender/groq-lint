use crate::rules::{get_pos, Context, Finding, Rule, RuleContext, Scope, Severity};
use groq_parser::ast::Expr;

pub struct ExtremelyLargeQuery;

impl Rule for ExtremelyLargeQuery {
    fn id(&self) -> &'static str {
        "extremely_large_query"
    }

    fn name(&self) -> &'static str {
        "Extremely Large Query"
    }

    fn description(&self) -> &'static str {
        "This query is extremely large (>100KB), and may execute very slowly."
    }

    fn context(&self) -> RuleContext {
        RuleContext::WholeQuery
    }

    fn visit(&self, expr: &Expr, context: &Context) -> Vec<Finding> {
        let mut findings = vec![];
        if context.query_len > 100_000 {
            findings.push(Finding {
                span: get_pos(expr).into(),
                message: self.description().to_string(),
                severity: Severity::High,
                rule_id: self.id().to_string(),
                scope: Scope::Global,
            });
        }
        findings
    }
}
