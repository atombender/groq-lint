//! WebAssembly bindings for groq-lint.
//!
//! This module exposes the linter to JavaScript via wasm-bindgen.

use serde::Serialize;
use wasm_bindgen::prelude::*;

use crate::rules::{Finding, Severity};

/// Result returned to JavaScript.
#[derive(Serialize)]
struct LintResult {
    success: bool,
    findings: Vec<JsFinding>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

/// Finding serialized for JavaScript consumption.
#[derive(Serialize)]
struct JsFinding {
    start: usize,
    end: usize,
    message: String,
    severity: &'static str,
    rule_id: String,
}

impl From<Finding> for JsFinding {
    fn from(f: Finding) -> Self {
        Self {
            start: f.span.start,
            end: f.span.end,
            message: f.message,
            severity: match f.severity {
                Severity::High => "error",
                Severity::Medium => "warning",
                Severity::Low => "suggestion",
            },
            rule_id: f.rule_id,
        }
    }
}

/// Initialize panic hook for better error messages in the browser console.
#[wasm_bindgen(start)]
pub fn init() {
    console_error_panic_hook::set_once();
}

/// Lint a GROQ query string.
///
/// Returns a JSON string with the following structure:
/// ```json
/// {
///   "success": true,
///   "findings": [
///     {
///       "start": 0,
///       "end": 10,
///       "message": "...",
///       "severity": "error" | "warning" | "suggestion",
///       "rule_id": "rule_name"
///     }
///   ]
/// }
/// ```
///
/// On parse error:
/// ```json
/// {
///   "success": false,
///   "findings": [],
///   "error": "Parse error message"
/// }
/// ```
#[wasm_bindgen]
pub fn lint(query: &str) -> String {
    let result = match crate::lint(query) {
        Ok(findings) => LintResult {
            success: true,
            findings: findings.into_iter().map(JsFinding::from).collect(),
            error: None,
        },
        Err(e) => LintResult {
            success: false,
            findings: vec![],
            error: Some(e.to_string()),
        },
    };

    serde_json::to_string(&result).unwrap_or_else(|e| {
        format!(
            r#"{{"success":false,"findings":[],"error":"Serialization error: {}"}}"#,
            e
        )
    })
}
