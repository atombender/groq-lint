//! Rule metadata loaded from rules.yaml.

use serde::Deserialize;
use std::collections::HashMap;
use std::sync::OnceLock;

/// The embedded rules.yaml content.
const RULES_YAML: &str = include_str!("../rules.yaml");

/// Parsed rule metadata.
#[derive(Debug, Deserialize)]
struct RulesFile {
    rules: Vec<RuleMeta>,
}

/// Metadata for a single rule.
#[derive(Debug, Clone, Deserialize)]
pub struct RuleMeta {
    pub id: String,
    #[serde(default)]
    pub pattern: String,
    #[serde(default)]
    pub advice: String,
    #[serde(default)]
    pub category: String,
    #[serde(default)]
    pub severity: String,
    #[serde(default)]
    pub context: String,
    #[serde(default)]
    pub supercedes: Vec<String>,
}

/// Global cache of parsed rule metadata.
static RULES_CACHE: OnceLock<HashMap<String, RuleMeta>> = OnceLock::new();

/// Get the rule metadata map, parsing it on first access.
fn get_rules_map() -> &'static HashMap<String, RuleMeta> {
    RULES_CACHE.get_or_init(|| {
        let rules_file: RulesFile =
            serde_yaml::from_str(RULES_YAML).expect("Failed to parse embedded rules.yaml");

        rules_file
            .rules
            .into_iter()
            .map(|r| (r.id.clone(), r))
            .collect()
    })
}

/// Look up the advice for a rule by its ID.
/// Returns the advice string if found, or None if the rule doesn't exist.
pub fn get_advice(rule_id: &str) -> Option<&'static str> {
    get_rules_map().get(rule_id).map(|r| r.advice.as_str())
}

/// Look up the severity for a rule by its ID.
/// Returns the severity string if found, or None if the rule doesn't exist.
pub fn get_severity(rule_id: &str) -> Option<&'static str> {
    get_rules_map().get(rule_id).map(|r| r.severity.as_str())
}

/// Look up full metadata for a rule by its ID.
pub fn get_rule_meta(rule_id: &str) -> Option<&'static RuleMeta> {
    get_rules_map().get(rule_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_rules() {
        let map = get_rules_map();
        assert!(!map.is_empty(), "Should have loaded rules");
        assert!(
            map.contains_key("join_in_filter"),
            "Should have join_in_filter rule"
        );
    }

    #[test]
    fn test_get_advice() {
        let advice = get_advice("join_in_filter");
        assert!(advice.is_some(), "Should have advice for join_in_filter");
        assert!(advice.unwrap().contains("->"), "Advice should mention ->");
    }

    #[test]
    fn test_advice_has_backticks() {
        let advice = get_advice("join_to_get_id");
        assert!(advice.is_some(), "Should have advice for join_to_get_id");
        let advice_str = advice.unwrap();
        // The advice should have backticks for code terms
        assert!(
            advice_str.contains('`'),
            "Advice should contain backticks: {}",
            advice_str
        );
    }
}
