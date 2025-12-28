pub mod ir;
pub mod rule_meta;
pub mod rules;

use crate::ir::IrGraph;
use crate::rules::{Context, Finding, IrRule, Rule, RuleContext};
use groq_parser::ast::Expr;
use groq_parser::parser::Parser;
// Legacy AST-based rules
use crate::rules::computed_value_in_filter::ComputedValueInFilter;
use crate::rules::count_in_correlated_subquery::CountInCorrelatedSubquery;
use crate::rules::deep_pagination::DeepPagination;
use crate::rules::deep_pagination_param::DeepPaginationParam;
use crate::rules::extremely_large_query::ExtremelyLargeQuery;
use crate::rules::join_in_filter::JoinInFilter;
use crate::rules::join_to_get_id::JoinToGetId;
use crate::rules::large_pages::LargePages;
use crate::rules::many_joins::ManyJoins;
use crate::rules::match_on_id::MatchOnId;
use crate::rules::non_literal_comparison::NonLiteralComparison;
use crate::rules::order_on_expr::OrderOnExpr;
use crate::rules::repeated_dereference::RepeatedDereference;
use crate::rules::very_large_query::VeryLargeQuery;

// New IR-based rules
use crate::rules::ir_rules::{
    IrComputedValueInFilter, IrCountInCorrelatedSubquery, IrDeepPagination, IrDeepPaginationParam,
    IrExtremelyLargeQuery, IrJoinInFilter, IrJoinToGetId, IrLargePages, IrManyJoins, IrMatchOnId,
    IrNonLiteralComparison, IrOrderOnExpr, IrRepeatedDereference, IrVeryLargeQuery,
};

pub struct Linter {
    /// Legacy AST-based rules.
    rules: Vec<Box<dyn Rule>>,
    /// New IR-based rules.
    ir_rules: Vec<Box<dyn IrRule>>,
}

impl Default for Linter {
    fn default() -> Self {
        Self::new()
    }
}

impl Linter {
    pub fn new() -> Self {
        Self {
            rules: vec![],
            ir_rules: vec![],
        }
    }

    pub fn with_all_rules() -> Self {
        let mut linter = Self::new();
        // Legacy AST-based rules
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

    /// Create a linter with IR-based rules.
    ///
    /// This uses the new semantic IR for rule checking, providing:
    /// - Structural scope tracking (no need for `in_filter` context flag)
    /// - Cleaner rule implementations using graph queries
    /// - Data flow analysis capabilities
    pub fn with_ir_rules() -> Self {
        let mut linter = Self::new();

        // All rules ported to IR
        linter.add_ir_rule(Box::new(IrJoinInFilter));
        linter.add_ir_rule(Box::new(IrJoinToGetId));
        linter.add_ir_rule(Box::new(IrComputedValueInFilter));
        linter.add_ir_rule(Box::new(IrMatchOnId));
        linter.add_ir_rule(Box::new(IrOrderOnExpr));
        linter.add_ir_rule(Box::new(IrDeepPagination));
        linter.add_ir_rule(Box::new(IrDeepPaginationParam));
        linter.add_ir_rule(Box::new(IrLargePages));
        linter.add_ir_rule(Box::new(IrNonLiteralComparison));
        linter.add_ir_rule(Box::new(IrRepeatedDereference));
        linter.add_ir_rule(Box::new(IrCountInCorrelatedSubquery));
        linter.add_ir_rule(Box::new(IrVeryLargeQuery));
        linter.add_ir_rule(Box::new(IrExtremelyLargeQuery));
        linter.add_ir_rule(Box::new(IrManyJoins));

        linter
    }

    pub fn add_rule(&mut self, rule: Box<dyn Rule>) {
        self.rules.push(rule);
    }

    pub fn add_ir_rule(&mut self, rule: Box<dyn IrRule>) {
        self.ir_rules.push(rule);
    }

    pub fn lint(&self, expr: &Expr, query_len: usize) -> Vec<Finding> {
        let mut findings = vec![];
        let context = Context {
            in_filter: false,
            query_len,
        };

        // 1. Run legacy WholeQuery rules once on the root expression
        for rule in &self.rules {
            if rule.context() == RuleContext::WholeQuery {
                findings.extend(rule.visit(expr, &context));
            }
        }

        // 2. Traverse AST for legacy Expr rules
        self.traverse(expr, &context, &mut findings);

        // 3. Run IR-based rules on the semantic graph
        if !self.ir_rules.is_empty() {
            let ir_graph = ir::lower(expr, query_len);
            findings.extend(self.lint_ir(&ir_graph));
        }

        findings
    }

    /// Run all IR-based rules on the semantic graph.
    fn lint_ir(&self, graph: &IrGraph) -> Vec<Finding> {
        let mut findings = vec![];
        let mut superceded_rules: std::collections::HashSet<&str> =
            std::collections::HashSet::new();

        // Run all rules and collect findings + supercession info
        for rule in &self.ir_rules {
            let rule_findings = rule.check(graph);
            if !rule_findings.is_empty() {
                // This rule fired, so collect its superceded rules
                for &superceded in rule.supercedes() {
                    superceded_rules.insert(superceded);
                }
            }
            findings.extend(rule_findings);
        }

        // Filter out findings from superceded rules
        if !superceded_rules.is_empty() {
            findings.retain(|f| !superceded_rules.contains(f.rule_id.as_str()));
        }

        findings
    }

    fn traverse(&self, expr: &Expr, context: &Context, findings: &mut Vec<Finding>) {
        // Run Expr rules on current node
        for rule in &self.rules {
            if rule.context() == RuleContext::Expr {
                findings.extend(rule.visit(expr, context));
            }
        }

        // Recurse children
        match expr {
            Expr::Filter(filter) => {
                let inner_ctx = Context {
                    in_filter: true,
                    query_len: context.query_len,
                };
                // Traverse constraint with in_filter = true
                self.traverse(&filter.constraint.expression, &inner_ctx, findings);
                // Traverse base with original context
                self.traverse(&filter.lhs, context, findings);
            }
            Expr::Projection(proj) => {
                self.traverse(&proj.lhs, context, findings);
                for e in &proj.object.expressions {
                    self.traverse(e, context, findings);
                }
            }
            Expr::Binary(bin) => {
                self.traverse(&bin.lhs, context, findings);
                self.traverse(&bin.rhs, context, findings);
            }
            Expr::Prefix(prefix) => {
                self.traverse(&prefix.rhs, context, findings);
            }
            Expr::Postfix(postfix) => {
                self.traverse(&postfix.lhs, context, findings);
            }
            Expr::Dot(dot) => {
                self.traverse(&dot.lhs, context, findings);
                self.traverse(&dot.rhs, context, findings);
            }
            Expr::Slice(slice) => {
                self.traverse(&slice.lhs, context, findings);
                self.traverse(&slice.range.value, context, findings);
            }
            Expr::Element(elem) => {
                self.traverse(&elem.lhs, context, findings);
                self.traverse(&elem.idx.value, context, findings);
            }
            Expr::ArrayTraversal(at) => {
                self.traverse(&at.expr, context, findings);
            }
            Expr::Array(arr) => {
                for e in &arr.expressions {
                    self.traverse(e, context, findings);
                }
            }
            Expr::Object(obj) => {
                for e in &obj.expressions {
                    self.traverse(e, context, findings);
                }
            }
            Expr::FunctionCall(func) => {
                for arg in &func.arguments {
                    self.traverse(arg, context, findings);
                }
            }
            Expr::FunctionPipe(fp) => {
                self.traverse(&fp.lhs, context, findings);
                for arg in &fp.func.arguments {
                    self.traverse(arg, context, findings);
                }
            }
            Expr::Group(g) => {
                self.traverse(&g.expression, context, findings);
            }
            Expr::Tuple(t) => {
                for member in &t.members {
                    self.traverse(member, context, findings);
                }
            }
            Expr::Constraint(c) => {
                self.traverse(&c.expression, context, findings);
            }
            _ => {} // Leaf nodes
        }
    }
}

/// Lint a GROQ query string using IR-based rules.
pub fn lint(query: &str) -> Result<Vec<Finding>, Box<dyn std::error::Error>> {
    let mut parser = Parser::new(query);
    let ast = parser.parse()?;
    let linter = Linter::with_ir_rules();
    Ok(linter.lint(&ast, query.len()))
}

/// Lint a GROQ query string using legacy AST-based rules.
pub fn lint_legacy(query: &str) -> Result<Vec<Finding>, Box<dyn std::error::Error>> {
    let mut parser = Parser::new(query);
    let ast = parser.parse()?;
    let linter = Linter::with_all_rules();
    Ok(linter.lint(&ast, query.len()))
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
