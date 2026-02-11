//! Multi-step scenario definitions and execution context.
//!
//! This module provides the core data structures for defining and executing
//! multi-step load testing scenarios. A scenario consists of a sequence of steps
//! that can extract variables, make assertions, and maintain state across requests.

use std::collections::HashMap;
use std::time::{Duration, Instant};

/// A multi-step test scenario representing a user journey.
///
/// # Example
/// ```
/// use rust_loadtest::scenario::{Scenario, Step, RequestConfig};
///
/// let scenario = Scenario {
///     name: "Shopping Flow".to_string(),
///     weight: 1.0,
///     steps: vec![
///         Step {
///             name: "Browse Products".to_string(),
///             request: RequestConfig {
///                 method: "GET".to_string(),
///                 path: "/products".to_string(),
///                 body: None,
///                 headers: HashMap::new(),
///             },
///             extractions: vec![],
///             assertions: vec![],
///             think_time: Some(Duration::from_secs(2)),
///         },
///     ],
/// };
/// ```
#[derive(Debug, Clone)]
pub struct Scenario {
    /// Unique name for this scenario
    pub name: String,

    /// Weight for traffic distribution (higher = more traffic)
    /// Used when running multiple scenarios: weight / sum(all_weights) = traffic percentage
    pub weight: f64,

    /// Sequential steps to execute
    pub steps: Vec<Step>,
}

/// A single step within a scenario.
#[derive(Debug, Clone)]
pub struct Step {
    /// Descriptive name for this step (e.g., "Login", "Add to Cart")
    pub name: String,

    /// HTTP request configuration
    pub request: RequestConfig,

    /// Variables to extract from the response
    pub extractions: Vec<VariableExtraction>,

    /// Assertions to validate the response
    pub assertions: Vec<Assertion>,

    /// Optional delay after this step completes (think time)
    pub think_time: Option<Duration>,
}

/// HTTP request configuration for a step.
#[derive(Debug, Clone)]
pub struct RequestConfig {
    /// HTTP method (GET, POST, PUT, DELETE, etc.)
    pub method: String,

    /// Request path (can contain variable references like "/products/${product_id}")
    pub path: String,

    /// Optional request body (can contain variable references)
    pub body: Option<String>,

    /// Request headers (values can contain variable references)
    pub headers: HashMap<String, String>,
}

/// Extract a variable from the response for use in subsequent steps.
#[derive(Debug, Clone)]
pub struct VariableExtraction {
    /// Name to store the extracted value under
    pub name: String,

    /// How to extract the value from the response
    pub extractor: Extractor,
}

/// Methods for extracting values from HTTP responses.
#[derive(Debug, Clone)]
pub enum Extractor {
    /// Extract from JSON response using JSONPath (e.g., "$.user.id")
    JsonPath(String),

    /// Extract using regex with named capture group
    Regex {
        pattern: String,
        group: String,
    },

    /// Extract from response header
    Header(String),

    /// Extract from cookie
    Cookie(String),
}

/// Assert conditions on the HTTP response.
#[derive(Debug, Clone)]
pub enum Assertion {
    /// Assert response status code equals expected value
    StatusCode(u16),

    /// Assert response time is below threshold
    ResponseTime(Duration),

    /// Assert JSON path exists and optionally matches value
    JsonPath {
        path: String,
        expected: Option<String>,
    },

    /// Assert response body contains substring
    BodyContains(String),

    /// Assert response body matches regex
    BodyMatches(String),
}

/// Execution context maintained across steps in a scenario.
///
/// Each virtual user gets their own context to maintain state across
/// the steps in a scenario execution.
#[derive(Debug, Clone)]
pub struct ScenarioContext {
    /// Extracted variables from previous steps
    variables: HashMap<String, String>,

    /// When this scenario execution started
    scenario_start: Instant,

    /// Current step index being executed
    current_step: usize,
}

impl ScenarioContext {
    /// Create a new scenario context.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            scenario_start: Instant::now(),
            current_step: 0,
        }
    }

    /// Store a variable for use in subsequent steps.
    pub fn set_variable(&mut self, name: String, value: String) {
        self.variables.insert(name, value);
    }

    /// Get a previously stored variable.
    pub fn get_variable(&self, name: &str) -> Option<&String> {
        self.variables.get(name)
    }

    /// Replace variable references in a string with their values.
    ///
    /// Supports syntax:
    /// - ${variable_name} or $variable_name - Replace with stored variable
    /// - ${timestamp} - Replace with current Unix timestamp in milliseconds
    ///
    /// # Example
    /// ```
    /// use rust_loadtest::scenario::ScenarioContext;
    ///
    /// let mut ctx = ScenarioContext::new();
    /// ctx.set_variable("user_id".to_string(), "12345".to_string());
    ///
    /// let result = ctx.substitute_variables("/users/${user_id}/profile");
    /// assert_eq!(result, "/users/12345/profile");
    /// ```
    pub fn substitute_variables(&self, input: &str) -> String {
        let mut result = input.to_string();

        // Replace special ${timestamp} variable with current timestamp
        if result.contains("${timestamp}") {
            let timestamp = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
                .to_string();
            result = result.replace("${timestamp}", &timestamp);
        }

        // Replace ${var} syntax
        for (name, value) in &self.variables {
            let pattern = format!("${{{}}}", name);
            result = result.replace(&pattern, value);
        }

        // Replace $var syntax (for simple variable names)
        for (name, value) in &self.variables {
            let pattern = format!("${}", name);
            // Only replace if not followed by { (to avoid replacing ${var} twice)
            result = result.replace(&pattern, value);
        }

        result
    }

    /// Get elapsed time since scenario started.
    pub fn elapsed(&self) -> Duration {
        self.scenario_start.elapsed()
    }

    /// Get current step index.
    pub fn current_step(&self) -> usize {
        self.current_step
    }

    /// Advance to next step.
    pub fn next_step(&mut self) {
        self.current_step += 1;
    }

    /// Reset context for a new scenario execution.
    pub fn reset(&mut self) {
        self.variables.clear();
        self.scenario_start = Instant::now();
        self.current_step = 0;
    }
}

impl Default for ScenarioContext {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_context_variables() {
        let mut ctx = ScenarioContext::new();

        ctx.set_variable("user_id".to_string(), "123".to_string());
        ctx.set_variable("token".to_string(), "abc-def".to_string());

        assert_eq!(ctx.get_variable("user_id"), Some(&"123".to_string()));
        assert_eq!(ctx.get_variable("token"), Some(&"abc-def".to_string()));
        assert_eq!(ctx.get_variable("missing"), None);
    }

    #[test]
    fn test_variable_substitution_braces() {
        let mut ctx = ScenarioContext::new();
        ctx.set_variable("product_id".to_string(), "prod-456".to_string());
        ctx.set_variable("user_id".to_string(), "user-789".to_string());

        let result = ctx.substitute_variables("/users/${user_id}/cart/items/${product_id}");
        assert_eq!(result, "/users/user-789/cart/items/prod-456");
    }

    #[test]
    fn test_variable_substitution_dollar() {
        let mut ctx = ScenarioContext::new();
        ctx.set_variable("id".to_string(), "42".to_string());

        let result = ctx.substitute_variables("/items/$id");
        assert_eq!(result, "/items/42");
    }

    #[test]
    fn test_variable_substitution_in_json() {
        let mut ctx = ScenarioContext::new();
        ctx.set_variable("cart_id".to_string(), "cart-999".to_string());
        ctx.set_variable("quantity".to_string(), "3".to_string());

        let json = r#"{"cart_id": "${cart_id}", "quantity": ${quantity}}"#;
        let result = ctx.substitute_variables(json);

        assert_eq!(result, r#"{"cart_id": "cart-999", "quantity": 3}"#);
    }

    #[test]
    fn test_step_counter() {
        let mut ctx = ScenarioContext::new();

        assert_eq!(ctx.current_step(), 0);

        ctx.next_step();
        assert_eq!(ctx.current_step(), 1);

        ctx.next_step();
        assert_eq!(ctx.current_step(), 2);

        ctx.reset();
        assert_eq!(ctx.current_step(), 0);
    }

    #[test]
    fn test_reset_clears_variables() {
        let mut ctx = ScenarioContext::new();
        ctx.set_variable("test".to_string(), "value".to_string());
        ctx.next_step();

        ctx.reset();

        assert_eq!(ctx.get_variable("test"), None);
        assert_eq!(ctx.current_step(), 0);
    }

    #[test]
    fn test_timestamp_substitution() {
        let ctx = ScenarioContext::new();

        let email = ctx.substitute_variables("user-${timestamp}@example.com");

        // Should contain a numeric timestamp
        assert!(email.starts_with("user-"));
        assert!(email.ends_with("@example.com"));
        assert!(email.contains(char::is_numeric));

        // Verify it's different each time (timestamps advance)
        std::thread::sleep(std::time::Duration::from_millis(2));
        let email2 = ctx.substitute_variables("user-${timestamp}@example.com");
        assert_ne!(email, email2);
    }

    #[test]
    fn test_scenario_creation() {
        let scenario = Scenario {
            name: "Test Scenario".to_string(),
            weight: 1.5,
            steps: vec![
                Step {
                    name: "Step 1".to_string(),
                    request: RequestConfig {
                        method: "GET".to_string(),
                        path: "/api/test".to_string(),
                        body: None,
                        headers: HashMap::new(),
                    },
                    extractions: vec![],
                    assertions: vec![],
                    think_time: None,
                },
            ],
        };

        assert_eq!(scenario.name, "Test Scenario");
        assert_eq!(scenario.weight, 1.5);
        assert_eq!(scenario.steps.len(), 1);
        assert_eq!(scenario.steps[0].name, "Step 1");
    }
}
