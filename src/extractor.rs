//! Variable extraction from HTTP responses.
//!
//! This module provides functionality to extract values from HTTP responses
//! using various methods: JSONPath, Regex, HTTP headers, and cookies.

use crate::scenario::{Extractor, VariableExtraction};
use regex::Regex;
use serde_json::Value;
use std::collections::HashMap;
use thiserror::Error;
use tracing::{debug, warn};

/// Errors that can occur during variable extraction.
#[derive(Error, Debug)]
pub enum ExtractionError {
    #[error("JSONPath query failed: {0}")]
    JsonPathError(String),

    #[error("Invalid JSON response: {0}")]
    InvalidJson(String),

    #[error("Regex compilation failed: {0}")]
    RegexError(#[from] regex::Error),

    #[error("Regex pattern did not match")]
    RegexNoMatch,

    #[error("Named capture group '{0}' not found in regex")]
    RegexGroupNotFound(String),

    #[error("Header '{0}' not found in response")]
    HeaderNotFound(String),

    #[error("Cookie '{0}' not found in response")]
    CookieNotFound(String),

    #[error("Extraction failed: {0}")]
    Other(String),
}

/// Extract variables from an HTTP response.
///
/// # Arguments
/// * `extractions` - List of variable extractions to perform
/// * `response_body` - Response body as string
/// * `response_headers` - Response headers
///
/// # Returns
/// HashMap of extracted variable names to values
pub fn extract_variables(
    extractions: &[VariableExtraction],
    response_body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> HashMap<String, String> {
    let mut variables = HashMap::new();

    for extraction in extractions {
        debug!(
            variable_name = %extraction.name,
            extractor = ?extraction.extractor,
            "Attempting variable extraction"
        );

        match extract_value(&extraction.extractor, response_body, response_headers) {
            Ok(value) => {
                debug!(
                    variable_name = %extraction.name,
                    value = %value,
                    "Successfully extracted variable"
                );
                variables.insert(extraction.name.clone(), value);
            }
            Err(e) => {
                warn!(
                    variable_name = %extraction.name,
                    error = %e,
                    "Failed to extract variable"
                );
                // Don't insert the variable if extraction fails
            }
        }
    }

    variables
}

/// Extract a single value using the specified extractor.
fn extract_value(
    extractor: &Extractor,
    response_body: &str,
    response_headers: &reqwest::header::HeaderMap,
) -> Result<String, ExtractionError> {
    match extractor {
        Extractor::JsonPath(path) => extract_json_path(response_body, path),
        Extractor::Regex { pattern, group } => extract_regex(response_body, pattern, group),
        Extractor::Header(header_name) => extract_header(response_headers, header_name),
        Extractor::Cookie(cookie_name) => extract_cookie(response_headers, cookie_name),
    }
}

/// Extract value using JSONPath query.
///
/// # Example
/// ```
/// use rust_loadtest::extractor::extract_json_path;
///
/// let json = r#"{"user": {"id": "123", "name": "Alice"}}"#;
/// let result = extract_json_path(json, "$.user.id").unwrap();
/// assert_eq!(result, "123");
/// ```
pub fn extract_json_path(json_body: &str, path: &str) -> Result<String, ExtractionError> {
    // Parse JSON
    let json: Value =
        serde_json::from_str(json_body).map_err(|e| ExtractionError::InvalidJson(e.to_string()))?;

    // Use serde_json_path to query
    use serde_json_path::JsonPath;

    let json_path = JsonPath::parse(path)
        .map_err(|e| ExtractionError::JsonPathError(format!("Invalid JSONPath: {}", e)))?;

    let node_list = json_path.query(&json);

    // Get first match
    if let Some(value) = node_list.exactly_one().ok() {
        // Convert value to string
        match value {
            Value::String(s) => Ok(s.clone()),
            Value::Number(n) => Ok(n.to_string()),
            Value::Bool(b) => Ok(b.to_string()),
            Value::Null => Ok("null".to_string()),
            Value::Array(_) | Value::Object(_) => {
                // Return JSON representation for complex types
                Ok(value.to_string())
            }
        }
    } else {
        // No match or multiple matches
        Err(ExtractionError::JsonPathError(format!(
            "JSONPath '{}' did not match exactly one value",
            path
        )))
    }
}

/// Extract value using regex with named capture group.
///
/// # Example
/// ```
/// use rust_loadtest::extractor::extract_regex;
///
/// let html = r#"<div id="user-123">Alice</div>"#;
/// let result = extract_regex(html, r#"id="user-(?P<id>\d+)""#, "id").unwrap();
/// assert_eq!(result, "123");
/// ```
pub fn extract_regex(text: &str, pattern: &str, group: &str) -> Result<String, ExtractionError> {
    let re = Regex::new(pattern)?;

    if let Some(captures) = re.captures(text) {
        if let Some(matched) = captures.name(group) {
            Ok(matched.as_str().to_string())
        } else {
            Err(ExtractionError::RegexGroupNotFound(group.to_string()))
        }
    } else {
        Err(ExtractionError::RegexNoMatch)
    }
}

/// Extract value from response header.
///
/// # Example
/// ```
/// use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE};
/// use rust_loadtest::extractor::extract_header;
///
/// let mut headers = HeaderMap::new();
/// headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
///
/// let result = extract_header(&headers, "content-type").unwrap();
/// assert_eq!(result, "application/json");
/// ```
pub fn extract_header(
    headers: &reqwest::header::HeaderMap,
    header_name: &str,
) -> Result<String, ExtractionError> {
    headers
        .get(header_name)
        .ok_or_else(|| ExtractionError::HeaderNotFound(header_name.to_string()))?
        .to_str()
        .map(|s| s.to_string())
        .map_err(|e| ExtractionError::Other(format!("Invalid header value: {}", e)))
}

/// Extract value from Set-Cookie header.
///
/// Parses Set-Cookie headers and extracts the specified cookie value.
///
/// # Example
/// ```
/// use reqwest::header::{HeaderMap, HeaderValue, SET_COOKIE};
/// use rust_loadtest::extractor::extract_cookie;
///
/// let mut headers = HeaderMap::new();
/// headers.insert(SET_COOKIE, HeaderValue::from_static("session_id=abc123; Path=/; HttpOnly"));
///
/// let result = extract_cookie(&headers, "session_id").unwrap();
/// assert_eq!(result, "abc123");
/// ```
pub fn extract_cookie(
    headers: &reqwest::header::HeaderMap,
    cookie_name: &str,
) -> Result<String, ExtractionError> {
    // Look through all Set-Cookie headers
    for value in headers.get_all(reqwest::header::SET_COOKIE) {
        if let Ok(cookie_str) = value.to_str() {
            // Parse cookie: "name=value; attributes..."
            if let Some(cookie_part) = cookie_str.split(';').next() {
                if let Some((name, val)) = cookie_part.split_once('=') {
                    if name.trim() == cookie_name {
                        return Ok(val.trim().to_string());
                    }
                }
            }
        }
    }

    Err(ExtractionError::CookieNotFound(cookie_name.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use reqwest::header::{HeaderMap, HeaderValue, CONTENT_TYPE, SET_COOKIE};

    #[test]
    fn test_extract_json_path_simple() {
        let json = r#"{"user": {"id": "123", "name": "Alice"}}"#;

        let result = extract_json_path(json, "$.user.id").unwrap();
        assert_eq!(result, "123");

        let result = extract_json_path(json, "$.user.name").unwrap();
        assert_eq!(result, "Alice");
    }

    #[test]
    fn test_extract_json_path_array() {
        let json = r#"{"products": [{"id": "prod-1", "name": "Laptop"}, {"id": "prod-2", "name": "Mouse"}]}"#;

        let result = extract_json_path(json, "$.products[0].id").unwrap();
        assert_eq!(result, "prod-1");

        let result = extract_json_path(json, "$.products[1].name").unwrap();
        assert_eq!(result, "Mouse");
    }

    #[test]
    fn test_extract_json_path_number() {
        let json = r#"{"price": 99.99, "quantity": 5}"#;

        let result = extract_json_path(json, "$.price").unwrap();
        assert_eq!(result, "99.99");

        let result = extract_json_path(json, "$.quantity").unwrap();
        assert_eq!(result, "5");
    }

    #[test]
    fn test_extract_json_path_bool() {
        let json = r#"{"active": true, "deleted": false}"#;

        let result = extract_json_path(json, "$.active").unwrap();
        assert_eq!(result, "true");

        let result = extract_json_path(json, "$.deleted").unwrap();
        assert_eq!(result, "false");
    }

    #[test]
    fn test_extract_json_path_not_found() {
        let json = r#"{"user": {"id": "123"}}"#;

        let result = extract_json_path(json, "$.user.email");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_json_path_invalid_json() {
        let invalid_json = r#"{"user": "broken"#;

        let result = extract_json_path(invalid_json, "$.user");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_regex_named_group() {
        let html = r#"<div id="user-123">Alice</div>"#;

        let result = extract_regex(html, r#"id="user-(?P<id>\d+)""#, "id").unwrap();
        assert_eq!(result, "123");
    }

    #[test]
    fn test_extract_regex_multiple_groups() {
        let text = "Order #12345 for user-678";

        let result = extract_regex(text, r#"Order #(?P<order>\d+)"#, "order").unwrap();
        assert_eq!(result, "12345");

        let result = extract_regex(text, r#"user-(?P<user>\d+)"#, "user").unwrap();
        assert_eq!(result, "678");
    }

    #[test]
    fn test_extract_regex_no_match() {
        let text = "No order here";

        let result = extract_regex(text, r#"Order #(?P<order>\d+)"#, "order");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_regex_group_not_found() {
        let text = "Order #12345";

        let result = extract_regex(text, r#"Order #(?P<order>\d+)"#, "missing_group");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_header() {
        let mut headers = HeaderMap::new();
        headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
        headers.insert("X-Request-ID", HeaderValue::from_static("req-123"));

        let result = extract_header(&headers, "content-type").unwrap();
        assert_eq!(result, "application/json");

        let result = extract_header(&headers, "x-request-id").unwrap();
        assert_eq!(result, "req-123");
    }

    #[test]
    fn test_extract_header_not_found() {
        let headers = HeaderMap::new();

        let result = extract_header(&headers, "missing-header");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_cookie() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_static("session_id=abc123; Path=/; HttpOnly"),
        );
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("user_pref=dark_mode; Path=/"),
        );

        let result = extract_cookie(&headers, "session_id").unwrap();
        assert_eq!(result, "abc123");

        let result = extract_cookie(&headers, "user_pref").unwrap();
        assert_eq!(result, "dark_mode");
    }

    #[test]
    fn test_extract_cookie_not_found() {
        let mut headers = HeaderMap::new();
        headers.insert(
            SET_COOKIE,
            HeaderValue::from_static("session_id=abc123; Path=/"),
        );

        let result = extract_cookie(&headers, "missing_cookie");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_cookie_no_cookies() {
        let headers = HeaderMap::new();

        let result = extract_cookie(&headers, "any_cookie");
        assert!(result.is_err());
    }

    #[test]
    fn test_extract_variables_multiple() {
        let extractions = vec![
            VariableExtraction {
                name: "user_id".to_string(),
                extractor: Extractor::JsonPath("$.user.id".to_string()),
            },
            VariableExtraction {
                name: "user_name".to_string(),
                extractor: Extractor::JsonPath("$.user.name".to_string()),
            },
        ];

        let json = r#"{"user": {"id": "123", "name": "Alice"}}"#;
        let headers = HeaderMap::new();

        let result = extract_variables(&extractions, json, &headers);

        assert_eq!(result.get("user_id"), Some(&"123".to_string()));
        assert_eq!(result.get("user_name"), Some(&"Alice".to_string()));
    }

    #[test]
    fn test_extract_variables_partial_failure() {
        let extractions = vec![
            VariableExtraction {
                name: "user_id".to_string(),
                extractor: Extractor::JsonPath("$.user.id".to_string()),
            },
            VariableExtraction {
                name: "missing".to_string(),
                extractor: Extractor::JsonPath("$.does.not.exist".to_string()),
            },
        ];

        let json = r#"{"user": {"id": "123"}}"#;
        let headers = HeaderMap::new();

        let result = extract_variables(&extractions, json, &headers);

        // Should extract user_id successfully
        assert_eq!(result.get("user_id"), Some(&"123".to_string()));
        // Should not include 'missing' since it failed
        assert_eq!(result.get("missing"), None);
        // Should have exactly 1 variable
        assert_eq!(result.len(), 1);
    }
}
