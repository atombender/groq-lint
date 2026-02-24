pub mod ir;
pub mod rule_meta;
pub mod rules;

#[cfg(feature = "wasm")]
mod wasm;

use crate::rules::{Finding, Rule};
use groq_parser::ast::Expr;
use groq_parser::parser::Parser;

use crate::rules::ir_rules::{
    ComputedValueInFilter, CountInCorrelatedSubquery, DeepPagination, DeepPaginationParam,
    ExtremelyLargeQuery, JoinInFilter, JoinToGetId, LargePages, ManyJoins, MatchOnId,
    NonLiteralComparison, OrderOnExpr, RepeatedDereference, VeryLargeQuery,
};

pub struct Linter {
    rules: Vec<Box<dyn Rule>>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        Self { rules: vec![] }
    }

    pub fn with_all_rules() -> Self {
        let mut linter = Self::new();
        linter.add_rule(Box::new(JoinInFilter));
        linter.add_rule(Box::new(JoinToGetId));
        linter.add_rule(Box::new(ComputedValueInFilter));
        linter.add_rule(Box::new(MatchOnId));
        linter.add_rule(Box::new(OrderOnExpr));
        linter.add_rule(Box::new(DeepPagination));
        linter.add_rule(Box::new(DeepPaginationParam));
        linter.add_rule(Box::new(LargePages));
        linter.add_rule(Box::new(NonLiteralComparison));
        linter.add_rule(Box::new(RepeatedDereference));
        linter.add_rule(Box::new(CountInCorrelatedSubquery));
        linter.add_rule(Box::new(VeryLargeQuery));
        linter.add_rule(Box::new(ExtremelyLargeQuery));
        linter.add_rule(Box::new(ManyJoins));
        linter
    }

    pub fn add_rule(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    pub fn lint(&self, expr: &Expr, query_len: usize) -> Vec<Finding> {
        let ir_graph = ir::lower(expr, query_len);
        let mut findings = vec![];
        let mut superceded_rules: std::collections::HashSet<&str> =
            std::collections::HashSet::new();

        for rule in &self.rules {
            let hits = rule.check(&ir_graph);
            if !hits.is_empty() {
                for &superceded in rule.supercedes() {
                    superceded_rules.insert(superceded);
                }
            }

            for hit in hits {
                let message = match hit.detail {
                    Some(detail) => format!("{} {}", rule.advice(), detail),
                    None => rule.advice().to_string(),
                };
                findings.push(Finding {
                    span: hit.span,
                    message,
                    severity: rule.severity(),
                    rule_id: rule.id().to_string(),
                    scope: hit.scope,
                });
            }
        }

        if !superceded_rules.is_empty() {
            findings.retain(|f| !superceded_rules.contains(f.rule_id.as_str()));
        }

        findings
    }
}

/// Lint a GROQ query string.
pub fn lint(query: &str) -> Result<Vec<Finding>, Box<dyn std::error::Error>> {
    let mut parser = Parser::new(query);
    let result = parser.parse()?;
    let linter = Linter::with_all_rules();
    Ok(linter.lint(&result.expr, query.len()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supercession_extremely_large_supercedes_very_large() {
        // Create a query larger than 100KB (will trigger both rules without supercession)
        let query = format!("*[_type == \"test\"]{{{}}}", "a,".repeat(60000));
        assert!(query.len() > 100 * 1024, "Query should be > 100KB");

        let findings = lint(&query).unwrap();

        // Should only have extremely_large_query, not very_large_query
        let rule_ids: Vec<&str> = findings.iter().map(|f| f.rule_id.as_str()).collect();
        assert!(
            rule_ids.contains(&"extremely_large_query"),
            "Should have extremely_large_query finding"
        );
        assert!(
            !rule_ids.contains(&"very_large_query"),
            "Should NOT have very_large_query finding (superceded)"
        );
    }

    #[test]
    fn test_very_large_query_not_superceded_when_under_100kb() {
        // Create a query between 10KB and 100KB
        let query = format!("*[_type == \"test\"]{{{}}}", "a,".repeat(6000));
        assert!(query.len() > 10 * 1024, "Query should be > 10KB");
        assert!(query.len() < 100 * 1024, "Query should be < 100KB");

        let findings = lint(&query).unwrap();

        // Should have very_large_query but not extremely_large_query
        let rule_ids: Vec<&str> = findings.iter().map(|f| f.rule_id.as_str()).collect();
        assert!(
            rule_ids.contains(&"very_large_query"),
            "Should have very_large_query finding"
        );
        assert!(
            !rule_ids.contains(&"extremely_large_query"),
            "Should NOT have extremely_large_query finding"
        );
    }
}
