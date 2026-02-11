//! Response assertion validation.
//!
//! This module provides functionality to validate HTTP responses against
//! assertions defined in scenarios.

use crate::scenario::Assertion;
use regex::Regex;
use serde_json::Value;
use std::time::Duration;
use thiserror::Error;
use tracing::{debug, warn};

/// Result of running an assertion.
#[derive(Debug, Clone)]
pub struct AssertionResult {
    /// The assertion that was checked
    pub assertion: Assertion,

    /// Whether the assertion passed
    pub passed: bool,

    /// Actual value observed (for debugging)
    pub actual: String,

    /// Expected value (for debugging)
    pub expected: String,

    /// Error message if assertion failed
    pub error_message: Option<String>,
}

/// Errors that can occur during assertion validation.
#[derive(Error, Debug)]
pub enum AssertionError {
    #[error("Status code mismatch: expected {expected}, got {actual}")]
    StatusCodeMismatch { expected: u16, actual: u16 },

    #[error("Response time {actual_ms}ms exceeds threshold {threshold_ms}ms")]
    ResponseTimeTooSlow {
        actual_ms: u64,
        threshold_ms: u64,
    },

    #[error("JSONPath assertion failed: {0}")]
    JsonPathFailed(String),

    #[error("Body does not contain expected substring: {0}")]
    BodyNotContains(String),

    #[error("Body does not match regex: {0}")]
    BodyNotMatches(String),

    #[error("Header '{0}' not found in response")]
    HeaderNotFound(String),

    #[error("Regex compilation failed: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Invalid JSON: {0}")]
    InvalidJson(String),
}

/// Run all assertions against a response.
///
/// # Arguments
/// * `assertions` - List of assertions to check
/// * `status_code` - HTTP status code from response
/// * `response_time_ms` - Response time in milliseconds
/// * `response_body` - Response body as string
/// * `response_headers` - Response headers
///
/// # Returns
/// Vector of assertion results (one per assertion)
pub fn run_assertions(
    assertions: &[Assertion],
    status_code: u16,
    response_time_ms: u64,
    response_body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> Vec<AssertionResult> {
    let mut results = Vec::new();

    for assertion in assertions {
        debug!(assertion = ?assertion, "Running assertion");

        let result = match run_single_assertion(
            assertion,
            status_code,
            response_time_ms,
            response_body,
            response_headers,
        ) {
            Ok(()) => {
                debug!(assertion = ?assertion, "Assertion passed");
                AssertionResult {
                    assertion: assertion.clone(),
                    passed: true,
                    actual: format_actual_value(assertion, status_code, response_time_ms, response_body),
                    expected: format_expected_value(assertion),
                    error_message: None,
                }
            }
            Err(e) => {
                warn!(assertion = ?assertion, error = %e, "Assertion failed");
                AssertionResult {
                    assertion: assertion.clone(),
                    passed: false,
                    actual: format_actual_value(assertion, status_code, response_time_ms, response_body),
                    expected: format_expected_value(assertion),
                    error_message: Some(e.to_string()),
                }
            }
        };

        results.push(result);
    }

    results
}

/// Run a single assertion.
fn run_single_assertion(
    assertion: &Assertion,
    status_code: u16,
    response_time_ms: u64,
    response_body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> Result<(), AssertionError> {
    match assertion {
        Assertion::StatusCode(expected) => {
            if status_code == *expected {
                Ok(())
            } else {
                Err(AssertionError::StatusCodeMismatch {
                    expected: *expected,
                    actual: status_code,
                })
            }
        }

        Assertion::ResponseTime(threshold) => {
            let threshold_ms = threshold.as_millis() as u64;
            if response_time_ms <= threshold_ms {
                Ok(())
            } else {
                Err(AssertionError::ResponseTimeTooSlow {
                    actual_ms: response_time_ms,
                    threshold_ms,
                })
            }
        }

        Assertion::JsonPath { path, expected } => {
            assert_json_path(response_body, path, expected.as_deref())
        }

        Assertion::BodyContains(substring) => {
            if response_body.contains(substring) {
                Ok(())
            } else {
                Err(AssertionError::BodyNotContains(substring.clone()))
            }
        }

        Assertion::BodyMatches(pattern) => {
            let re = Regex::new(pattern)?;
            if re.is_match(response_body) {
                Ok(())
            } else {
                Err(AssertionError::BodyNotMatches(pattern.clone()))
            }
        }

        Assertion::HeaderExists(header_name) => {
            if response_headers.contains_key(header_name) {
                Ok(())
            } else {
                Err(AssertionError::HeaderNotFound(header_name.clone()))
            }
        }
    }
}

/// Assert JSONPath condition.
fn assert_json_path(
    json_body: &str,
    path: &str,
    expected: Option<&str>,
) -> Result<(), AssertionError> {
    use serde_json_path::JsonPath;

    // Parse JSON
    let json: Value = serde_json::from_str(json_body)
        .map_err(|e| AssertionError::InvalidJson(e.to_string()))?;

    // Parse JSONPath
    let json_path = JsonPath::parse(path)
        .map_err(|e| AssertionError::JsonPathFailed(format!("Invalid JSONPath '{}': {}", path, e)))?;

    // Query
    let node_list = json_path.query(&json);

    // Check if path exists
    if let Some(value) = node_list.exactly_one().ok() {
        // Path exists, now check expected value if provided
        if let Some(expected_value) = expected {
            let actual_str = match value {
                Value::String(s) => s.clone(),
                Value::Number(n) => n.to_string(),
                Value::Bool(b) => b.to_string(),
                Value::Null => "null".to_string(),
                _ => value.to_string(),
            };

            if actual_str == expected_value {
                Ok(())
            } else {
                Err(AssertionError::JsonPathFailed(format!(
                    "JSONPath '{}' value mismatch: expected '{}', got '{}'",
                    path, expected_value, actual_str
                )))
            }
        } else {
            // No expected value, just checking existence
            Ok(())
        }
    } else {
        Err(AssertionError::JsonPathFailed(format!(
            "JSONPath '{}' did not match exactly one value",
            path
        )))
    }
}

/// Format actual value for display.
fn format_actual_value(
    assertion: &Assertion,
    status_code: u16,
    response_time_ms: u64,
    response_body: &str,
) -> String {
    match assertion {
        Assertion::StatusCode(_) => status_code.to_string(),
        Assertion::ResponseTime(_) => format!("{}ms", response_time_ms),
        Assertion::JsonPath { path, .. } => {
            format!("JSONPath: {}", path)
        }
        Assertion::BodyContains(_) => {
            if response_body.len() > 100 {
                format!("{}...", &response_body[..100])
            } else {
                response_body.to_string()
            }
        }
        Assertion::BodyMatches(_) => {
            if response_body.len() > 100 {
                format!("{}...", &response_body[..100])
            } else {
                response_body.to_string()
            }
        }
        Assertion::HeaderExists(header) => format!("header '{}'", header),
    }
}

/// Format expected value for display.
fn format_expected_value(assertion: &Assertion) -> String {
    match assertion {
        Assertion::StatusCode(code) => code.to_string(),
        Assertion::ResponseTime(duration) => format!("<{}ms", duration.as_millis()),
        Assertion::JsonPath { path, expected } => {
            if let Some(exp) = expected {
                format!("{} = {}", path, exp)
            } else {
                format!("{} exists", path)
            }
        }
        Assertion::BodyContains(substring) => format!("contains '{}'", substring),
        Assertion::BodyMatches(pattern) => format!("matches /{}/", pattern),
        Assertion::HeaderExists(header) => format!("header '{}' exists", header),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::HeaderMap;

    #[test]
    fn test_status_code_assertion_pass() {
        let assertion = Assertion::StatusCode(200);
        let result = run_single_assertion(&assertion, 200, 100, "", &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_status_code_assertion_fail() {
        let assertion = Assertion::StatusCode(200);
        let result = run_single_assertion(&assertion, 404, 100, "", &HeaderMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_response_time_assertion_pass() {
        let assertion = Assertion::ResponseTime(Duration::from_millis(500));
        let result = run_single_assertion(&assertion, 200, 300, "", &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_response_time_assertion_fail() {
        let assertion = Assertion::ResponseTime(Duration::from_millis(500));
        let result = run_single_assertion(&assertion, 200, 700, "", &HeaderMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_json_path_existence() {
        let json = r#"{"user": {"id": "123"}}"#;
        let assertion = Assertion::JsonPath {
            path: "$.user.id".to_string(),
            expected: None,
        };
        let result = run_single_assertion(&assertion, 200, 100, json, &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_value_match() {
        let json = r#"{"status": "ok"}"#;
        let assertion = Assertion::JsonPath {
            path: "$.status".to_string(),
            expected: Some("ok".to_string()),
        };
        let result = run_single_assertion(&assertion, 200, 100, json, &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_json_path_value_mismatch() {
        let json = r#"{"status": "error"}"#;
        let assertion = Assertion::JsonPath {
            path: "$.status".to_string(),
            expected: Some("ok".to_string()),
        };
        let result = run_single_assertion(&assertion, 200, 100, json, &HeaderMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_body_contains_pass() {
        let body = "Hello, world!";
        let assertion = Assertion::BodyContains("world".to_string());
        let result = run_single_assertion(&assertion, 200, 100, body, &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_body_contains_fail() {
        let body = "Hello, world!";
        let assertion = Assertion::BodyContains("missing".to_string());
        let result = run_single_assertion(&assertion, 200, 100, body, &HeaderMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_body_matches_regex_pass() {
        let body = "Order #12345 confirmed";
        let assertion = Assertion::BodyMatches(r"Order #\d+".to_string());
        let result = run_single_assertion(&assertion, 200, 100, body, &HeaderMap::new());
        assert!(result.is_ok());
    }

    #[test]
    fn test_body_matches_regex_fail() {
        let body = "No order here";
        let assertion = Assertion::BodyMatches(r"Order #\d+".to_string());
        let result = run_single_assertion(&assertion, 200, 100, body, &HeaderMap::new());
        assert!(result.is_err());
    }

    #[test]
    fn test_run_multiple_assertions() {
        let json = r#"{"status": "ok", "count": 5}"#;
        let assertions = vec![
            Assertion::StatusCode(200),
            Assertion::ResponseTime(Duration::from_millis(500)),
            Assertion::JsonPath {
                path: "$.status".to_string(),
                expected: Some("ok".to_string()),
            },
            Assertion::BodyContains("count".to_string()),
        ];

        let results = run_assertions(&assertions, 200, 300, json, &HeaderMap::new());

        assert_eq!(results.len(), 4);
        assert!(results.iter().all(|r| r.passed));
    }

    #[test]
    fn test_run_assertions_with_failures() {
        let assertions = vec![
            Assertion::StatusCode(200),        // Pass
            Assertion::StatusCode(404),        // Fail
            Assertion::BodyContains("test".to_string()), // Pass
        ];

        let body = "This is a test";
        let results = run_assertions(&assertions, 200, 100, body, &HeaderMap::new());

        assert_eq!(results.len(), 3);
        assert!(results[0].passed); // StatusCode 200
        assert!(!results[1].passed); // StatusCode 404
        assert!(results[2].passed); // BodyContains
    }
}
