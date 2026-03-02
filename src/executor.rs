//! Scenario execution engine.
//!
//! This module provides the execution engine for running multi-step scenarios.
//! It handles sequential step execution, context management, variable substitution,
//! and metrics tracking.

use crate::assertions;
use crate::extractor;
use crate::metrics::{
    CONCURRENT_SCENARIOS, SCENARIO_ASSERTIONS_TOTAL, SCENARIO_DURATION_SECONDS,
    SCENARIO_EXECUTIONS_TOTAL, SCENARIO_STEPS_TOTAL, SCENARIO_STEP_DURATION_SECONDS,
    SCENARIO_STEP_STATUS_CODES,
};
use crate::scenario::{Scenario, ScenarioContext, Step};
use std::collections::HashMap;
use std::time::Instant;
use tokio::time::sleep;
use tracing::{debug, error, info, warn};

/// Cached variables from a single step, kept alive until `expires_at`.
pub struct SessionEntry {
    pub variables: HashMap<String, String>,
    pub expires_at: Instant,
}

/// Per-worker session store: step name → cached result.
///
/// Lives for the lifetime of the worker (outside the scenario iteration loop)
/// so extracted variables survive across iterations until their TTL expires.
pub type SessionStore = HashMap<String, SessionEntry>;

/// Result of executing a single step.
#[derive(Debug)]
pub struct StepResult {
    /// Name of the step that was executed
    pub step_name: String,

    /// Whether the step succeeded
    pub success: bool,

    /// HTTP status code received
    pub status_code: Option<u16>,

    /// Response time in milliseconds
    pub response_time_ms: u64,

    /// Error message if step failed
    pub error: Option<String>,

    /// Assertions that passed
    pub assertions_passed: usize,

    /// Assertions that failed
    pub assertions_failed: usize,

    /// True when the step result was served from the session cache (no HTTP request made).
    pub cache_hit: bool,
}

/// Result of executing an entire scenario.
#[derive(Debug)]
pub struct ScenarioResult {
    /// Name of the scenario
    pub scenario_name: String,

    /// Whether all steps succeeded
    pub success: bool,

    /// Results from each step
    pub steps: Vec<StepResult>,

    /// Total scenario execution time in milliseconds
    pub total_time_ms: u64,

    /// Number of steps completed
    pub steps_completed: usize,

    /// Step index where execution stopped (if failed)
    pub failed_at_step: Option<usize>,
}

/// Executor for running scenarios.
///
/// # Cookie and Session Management
///
/// The executor automatically handles cookies when the provided client has
/// cookie support enabled. Each client instance maintains its own cookie jar,
/// providing session isolation per virtual user.
///
/// To enable automatic cookie handling:
/// ```rust,no_run
/// let client = reqwest::Client::builder()
///     .cookie_store(true)  // Enable automatic cookie management
///     .build()
///     .unwrap();
/// ```
///
/// Cookies are automatically:
/// - Stored from Set-Cookie response headers
/// - Sent with subsequent requests to the same domain
/// - Isolated per client instance (per virtual user)
pub struct ScenarioExecutor {
    /// Base URL for requests (e.g., "https://api.example.com")
    base_url: String,

    /// HTTP client for making requests
    /// Should have cookie_store(true) enabled for session management
    client: reqwest::Client,
}

impl ScenarioExecutor {
    /// Create a new scenario executor.
    ///
    /// # Arguments
    /// * `base_url` - Base URL for all requests in the scenario
    /// * `client` - HTTP client to use for requests. Should have `cookie_store(true)`
    ///   enabled for automatic cookie and session management.
    ///
    /// # Example
    /// ```rust
    /// use rust_loadtest::executor::ScenarioExecutor;
    ///
    /// let client = reqwest::Client::builder()
    ///     .cookie_store(true)  // Enable cookies
    ///     .build()
    ///     .unwrap();
    ///
    /// let executor = ScenarioExecutor::new(
    ///     "https://api.example.com".to_string(),
    ///     client
    /// );
    /// ```
    pub fn new(base_url: String, client: reqwest::Client) -> Self {
        Self { base_url, client }
    }

    /// Execute a scenario with the given context.
    ///
    /// Steps are executed sequentially. If any step fails, execution stops
    /// and returns the partial results.
    ///
    /// # Arguments
    /// * `scenario` - The scenario to execute
    /// * `context` - Execution context (will be modified with extracted variables)
    ///
    /// # Returns
    /// Results from scenario execution including per-step metrics
    pub async fn execute(
        &self,
        scenario: &Scenario,
        context: &mut ScenarioContext,
        session: &mut SessionStore,
    ) -> ScenarioResult {
        let scenario_start = Instant::now();
        let mut step_results = Vec::new();
        let mut all_success = true;
        let mut failed_at_step = None;

        // Track concurrent scenario execution
        CONCURRENT_SCENARIOS.inc();

        info!(
            scenario = %scenario.name,
            steps = scenario.steps.len(),
            "Starting scenario execution"
        );

        for (idx, step) in scenario.steps.iter().enumerate() {
            debug!(
                scenario = %scenario.name,
                step = %step.name,
                step_idx = idx,
                "Executing step"
            );

            let step_result = self.execute_step(&scenario.name, step, context, session).await;

            let success = step_result.success;
            step_results.push(step_result);

            if !success {
                all_success = false;
                failed_at_step = Some(idx);
                error!(
                    scenario = %scenario.name,
                    step = %step.name,
                    step_idx = idx,
                    "Step failed, stopping scenario execution"
                );
                break;
            }

            context.next_step();

            // Apply think time if configured (simulates user delay between actions)
            if let Some(ref think_time) = step.think_time {
                let delay = think_time.calculate_delay();
                debug!(
                    scenario = %scenario.name,
                    step = %step.name,
                    think_time_ms = delay.as_millis(),
                    think_time_type = ?think_time,
                    "Applying think time"
                );
                sleep(delay).await;
            }
        }

        let total_time_ms = scenario_start.elapsed().as_millis() as u64;
        let total_time_secs = total_time_ms as f64 / 1000.0;

        let result = ScenarioResult {
            scenario_name: scenario.name.clone(),
            success: all_success,
            steps: step_results,
            total_time_ms,
            steps_completed: context.current_step(),
            failed_at_step,
        };

        // Record scenario metrics
        CONCURRENT_SCENARIOS.dec();
        SCENARIO_DURATION_SECONDS
            .with_label_values(&[&scenario.name])
            .observe(total_time_secs);

        let status = if all_success { "success" } else { "failed" };
        SCENARIO_EXECUTIONS_TOTAL
            .with_label_values(&[&scenario.name, status])
            .inc();

        if all_success {
            info!(
                scenario = %scenario.name,
                total_time_ms,
                steps_completed = result.steps_completed,
                "Scenario completed successfully"
            );
        } else {
            warn!(
                scenario = %scenario.name,
                total_time_ms,
                steps_completed = result.steps_completed,
                failed_at_step = ?failed_at_step,
                "Scenario failed"
            );
        }

        result
    }

    /// Execute a single step.
    async fn execute_step(
        &self,
        scenario_name: &str,
        step: &Step,
        context: &mut ScenarioContext,
        session: &mut SessionStore,
    ) -> StepResult {
        // ── Session cache check ────────────────────────────────────────────
        if step.cache.is_some() {
            if let Some(entry) = session.get(&step.name) {
                if entry.expires_at > Instant::now() {
                    for (name, value) in &entry.variables {
                        context.set_variable(name.clone(), value.clone());
                    }
                    debug!(step = %step.name, "Session cache hit — skipping HTTP request");
                    return StepResult {
                        step_name: step.name.clone(),
                        success: true,
                        status_code: None,
                        response_time_ms: 0,
                        error: None,
                        assertions_passed: 0,
                        assertions_failed: 0,
                        cache_hit: true,
                    };
                }
                // Entry expired — evict it so we make a fresh request
                session.remove(&step.name);
            }
        }

        let step_start = Instant::now();

        // Build the full URL with variable substitution
        let path = context.substitute_variables(&step.request.path);
        let url = if path.starts_with("http://") || path.starts_with("https://") {
            path
        } else {
            format!("{}{}", self.base_url, path)
        };

        debug!(
            step = %step.name,
            method = %step.request.method,
            url = %url,
            "Making HTTP request"
        );

        // Build the request
        let mut request_builder = match step.request.method.to_uppercase().as_str() {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PUT" => self.client.put(&url),
            "DELETE" => self.client.delete(&url),
            "PATCH" => self.client.patch(&url),
            "HEAD" => self.client.head(&url),
            "OPTIONS" => self.client.request(reqwest::Method::OPTIONS, &url),
            method => {
                error!(step = %step.name, method = %method, "Unsupported HTTP method");
                return StepResult {
                    step_name: step.name.clone(),
                    success: false,
                    status_code: None,
                    response_time_ms: 0,
                    error: Some(format!("Unsupported HTTP method: {}", method)),
                    assertions_passed: 0,
                    assertions_failed: 0,
                    cache_hit: false,
                };
            }
        };

        // Add headers with variable substitution
        for (key, value) in &step.request.headers {
            let substituted_value = context.substitute_variables(value);
            request_builder = request_builder.header(key, substituted_value);
        }

        // Add body if present with variable substitution
        if let Some(body) = &step.request.body {
            let substituted_body = context.substitute_variables(body);
            request_builder = request_builder.body(substituted_body);
        }

        // Execute the request
        let response_result = request_builder.send().await;

        let response_time_ms = step_start.elapsed().as_millis() as u64;

        match response_result {
            Ok(response) => {
                let status = response.status();
                let headers = response.headers().clone();

                debug!(
                    step = %step.name,
                    status = status.as_u16(),
                    response_time_ms,
                    "Received response"
                );

                // Get response body for extraction and assertions
                let body_result = response.text().await;

                let body_result_data = match body_result {
                    Ok(body) => {
                        // Extract variables from response (#27 - IMPLEMENTED)
                        let extracted_count = if !step.extractions.is_empty() {
                            debug!(
                                step = %step.name,
                                extractions = step.extractions.len(),
                                "Extracting variables from response"
                            );

                            let extracted =
                                extractor::extract_variables(&step.extractions, &body, &headers);

                            let count = extracted.len();

                            // If this step has a cache config, keep a copy for the session store
                            let for_session: Option<HashMap<String, String>> =
                                if step.cache.is_some() {
                                    Some(extracted.clone())
                                } else {
                                    None
                                };

                            // Store extracted variables in context
                            for (name, value) in &extracted {
                                debug!(
                                    step = %step.name,
                                    variable = %name,
                                    value = %value,
                                    "Stored extracted variable"
                                );
                                context.set_variable(name.clone(), value.clone());
                            }

                            // Cache the extracted variables for future iterations
                            if let (Some(cache_cfg), Some(vars)) = (&step.cache, for_session) {
                                let expires_at = Instant::now() + cache_cfg.ttl;
                                debug!(
                                    step = %step.name,
                                    ttl_secs = cache_cfg.ttl.as_secs(),
                                    "Caching step result in session store"
                                );
                                session.insert(step.name.clone(), SessionEntry { variables: vars, expires_at });
                            }

                            count
                        } else {
                            0
                        };

                        // Run assertions on response (#30 - IMPLEMENTED)
                        let (assertions_passed, assertions_failed) = if !step.assertions.is_empty()
                        {
                            debug!(
                                step = %step.name,
                                assertions = step.assertions.len(),
                                "Running assertions on response"
                            );

                            let assertion_results = assertions::run_assertions(
                                &step.assertions,
                                status.as_u16(),
                                response_time_ms,
                                &body,
                                &headers,
                            );

                            let passed = assertion_results.iter().filter(|r| r.passed).count();
                            let failed = assertion_results.iter().filter(|r| !r.passed).count();

                            // Log assertion results
                            for result in &assertion_results {
                                if result.passed {
                                    debug!(
                                        step = %step.name,
                                        assertion = ?result.assertion,
                                        "Assertion passed"
                                    );
                                } else {
                                    warn!(
                                        step = %step.name,
                                        assertion = ?result.assertion,
                                        error = ?result.error_message,
                                        "Assertion failed"
                                    );
                                }

                                // Record assertion metrics
                                let result_label = if result.passed { "passed" } else { "failed" };
                                SCENARIO_ASSERTIONS_TOTAL
                                    .with_label_values(&[scenario_name, &step.name, result_label])
                                    .inc();
                            }

                            (passed, failed)
                        } else {
                            (0, 0)
                        };

                        // Step succeeds if HTTP status is success/redirect AND all assertions pass
                        let http_success = status.is_success() || status.is_redirection();
                        let all_assertions_pass = assertions_failed == 0;
                        let success = http_success && all_assertions_pass;

                        let error_msg = if !success {
                            if !http_success {
                                Some(format!("HTTP {}", status.as_u16()))
                            } else if !all_assertions_pass {
                                Some(format!("{} assertion(s) failed", assertions_failed))
                            } else {
                                None
                            }
                        } else {
                            None
                        };

                        (
                            success,
                            extracted_count,
                            assertions_passed,
                            assertions_failed,
                            error_msg,
                        )
                    }
                    Err(e) => {
                        warn!(
                            step = %step.name,
                            error = %e,
                            "Failed to read response body"
                        );
                        (
                            false,
                            0,
                            0,
                            0,
                            Some(format!("Failed to read response body: {}", e)),
                        )
                    }
                };

                let (success, _extracted_count, assertions_passed, assertions_failed, error_msg) =
                    body_result_data;

                // Record step metrics
                let response_time_secs = response_time_ms as f64 / 1000.0;
                SCENARIO_STEP_DURATION_SECONDS
                    .with_label_values(&[scenario_name, &step.name])
                    .observe(response_time_secs);

                let status_code_str = status.as_u16().to_string();
                SCENARIO_STEP_STATUS_CODES
                    .with_label_values(&[scenario_name, &step.name, &status_code_str])
                    .inc();

                let step_status = if success { "success" } else { "failed" };
                SCENARIO_STEPS_TOTAL
                    .with_label_values(&[scenario_name, &step.name, step_status])
                    .inc();

                debug!(
                    step = %step.name,
                    status_code = status.as_u16(),
                    success = success,
                    assertions_passed = assertions_passed,
                    assertions_failed = assertions_failed,
                    "Step execution complete"
                );

                StepResult {
                    step_name: step.name.clone(),
                    success,
                    status_code: Some(status.as_u16()),
                    response_time_ms,
                    error: error_msg,
                    assertions_passed,
                    assertions_failed,
                    cache_hit: false,
                }
            }
            Err(e) => {
                error!(
                    step = %step.name,
                    error = %e,
                    response_time_ms,
                    "Request failed"
                );

                // Record failed step metrics
                SCENARIO_STEPS_TOTAL
                    .with_label_values(&[scenario_name, &step.name, "failed"])
                    .inc();

                StepResult {
                    step_name: step.name.clone(),
                    success: false,
                    status_code: None,
                    response_time_ms,
                    error: Some(e.to_string()),
                    assertions_passed: 0,
                    assertions_failed: 0,
                    cache_hit: false,
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_scenario_result_success() {
        let result = ScenarioResult {
            scenario_name: "Test".to_string(),
            success: true,
            steps: vec![],
            total_time_ms: 100,
            steps_completed: 3,
            failed_at_step: None,
        };

        assert!(result.success);
        assert_eq!(result.steps_completed, 3);
        assert_eq!(result.failed_at_step, None);
    }

    #[test]
    fn test_scenario_result_failure() {
        let result = ScenarioResult {
            scenario_name: "Test".to_string(),
            success: false,
            steps: vec![],
            total_time_ms: 50,
            steps_completed: 1,
            failed_at_step: Some(1),
        };

        assert!(!result.success);
        assert_eq!(result.steps_completed, 1);
        assert_eq!(result.failed_at_step, Some(1));
    }

    #[test]
    fn test_step_result_success() {
        let result = StepResult {
            step_name: "Login".to_string(),
            success: true,
            status_code: Some(200),
            response_time_ms: 150,
            error: None,
            assertions_passed: 2,
            assertions_failed: 0,
        };

        assert!(result.success);
        assert_eq!(result.status_code, Some(200));
        assert_eq!(result.error, None);
    }

    #[tokio::test]
    async fn test_executor_creation() {
        let client = reqwest::Client::new();
        let executor = ScenarioExecutor::new("https://example.com".to_string(), client);

        assert_eq!(executor.base_url, "https://example.com");
    }

    // Integration tests with actual HTTP calls would go here
    // For now, keeping tests simple to avoid external dependencies
}
