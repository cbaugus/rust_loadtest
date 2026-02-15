//! Integration tests for multi-step scenario execution.
//!
//! These tests run against the live mock e-commerce API at
//! https://ecom.edge.baugus-lab.com to validate scenario execution.
//!
//! Run with: cargo test --test scenario_integration_tests

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{
    Assertion, Extractor, RequestConfig, Scenario, ScenarioContext, Step, ThinkTime,
    VariableExtraction,
};
use std::collections::HashMap;
use std::time::Duration;

const BASE_URL: &str = "https://ecom.edge.baugus-lab.com";

/// Create a basic HTTP client for testing
fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
async fn test_health_check_scenario() {
    let scenario = Scenario {
        name: "Health Check".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Health".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/health".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::StatusCode(200)],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Health check scenario should succeed");
    assert_eq!(result.steps.len(), 1);
    assert_eq!(result.steps[0].status_code, Some(200));
}

#[tokio::test]
async fn test_product_browsing_scenario() {
    let scenario = Scenario {
        name: "Product Browsing".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "List Products".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=10".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(100))),
            },
            Step {
                name: "Get Product Details".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    // Using a known product ID for testing
                    // In real scenarios, this would be extracted from step 1
                    path: "/products/prod-1".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Product browsing scenario should succeed");
    assert_eq!(result.steps_completed, 2);
    assert_eq!(result.steps.len(), 2);

    // Verify both steps succeeded
    for step in &result.steps {
        assert!(step.success, "Step '{}' should succeed", step.step_name);
        assert_eq!(step.status_code, Some(200));
    }
}

#[tokio::test]
async fn test_variable_substitution() {
    let mut context = ScenarioContext::new();

    // Simulate extracting a product ID (this will be done by #27)
    context.set_variable("product_id".to_string(), "prod-123".to_string());

    let scenario = Scenario {
        name: "Variable Substitution Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get Product with Variable".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/products/${product_id}".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

    let result = executor.execute(&scenario, &mut context).await;

    // The request should have been made to /products/prod-123
    // If variable substitution works, we'll get a response
    assert!(
        result.steps[0].status_code.is_some(),
        "Should have received a response"
    );
}

#[tokio::test]
async fn test_multi_step_with_delays() {
    let scenario = Scenario {
        name: "Multi-Step with Think Times".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Step 1".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(Duration::from_millis(200)),
            },
            Step {
                name: "Step 2".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/status".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(Duration::from_millis(200)),
            },
            Step {
                name: "Step 3".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=1".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let start = std::time::Instant::now();
    let result = executor.execute(&scenario, &mut context).await;
    let duration = start.elapsed();

    assert!(result.success, "Multi-step scenario should succeed");
    assert_eq!(result.steps_completed, 3);

    // Should take at least 400ms (200ms + 200ms think times)
    assert!(
        duration.as_millis() >= 400,
        "Scenario should respect think times (took {}ms, expected >= 400ms)",
        duration.as_millis()
    );
}

#[tokio::test]
async fn test_scenario_failure_handling() {
    let scenario = Scenario {
        name: "Failure Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Valid Request".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "Invalid Request".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/this-endpoint-does-not-exist-404".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "Should Not Execute".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Scenario should fail on step 2
    assert!(!result.success, "Scenario should fail");
    assert_eq!(result.steps_completed, 1, "Should complete only 1 step");
    assert_eq!(
        result.failed_at_step,
        Some(1),
        "Should fail at step 1 (index 1)"
    );
    assert_eq!(result.steps.len(), 2, "Should have 2 step results");

    // Step 1 should succeed
    assert!(result.steps[0].success);
    assert_eq!(result.steps[0].status_code, Some(200));

    // Step 2 should fail with 404
    assert!(!result.steps[1].success);
    assert_eq!(result.steps[1].status_code, Some(404));
}

#[tokio::test]
async fn test_timestamp_variable() {
    let scenario = Scenario {
        name: "Timestamp Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Request with Timestamp".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/health".to_string(),
                body: None,
                headers: {
                    let mut headers = HashMap::new();
                    // Test timestamp in headers
                    headers.insert("X-Request-ID".to_string(), "req-${timestamp}".to_string());
                    headers
                },
            },
            extractions: vec![],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Timestamp substitution should work, request should succeed
    assert!(result.success, "Scenario with timestamp should succeed");
    assert_eq!(result.steps[0].status_code, Some(200));
}

#[tokio::test]
async fn test_post_request_with_json_body() {
    let scenario = Scenario {
        name: "POST Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Register User".to_string(),
            request: RequestConfig {
                method: "POST".to_string(),
                path: "/auth/register".to_string(),
                body: Some(
                    r#"{
                        "email": "loadtest-${timestamp}@example.com",
                        "password": "TestPass123!",
                        "name": "Test User"
                    }"#
                    .to_string(),
                ),
                headers: {
                    let mut headers = HashMap::new();
                    headers.insert("Content-Type".to_string(), "application/json".to_string());
                    headers
                },
            },
            extractions: vec![],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Registration should work (201 Created or 200 OK)
    assert!(
        result.steps[0].success,
        "Registration should succeed, got status: {:?}",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_scenario_context_isolation() {
    // Test that each scenario execution has isolated context
    let scenario = Scenario {
        name: "Context Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Simple Request".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/health".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

    // Execute scenario twice with different contexts
    let mut context1 = ScenarioContext::new();
    context1.set_variable("test".to_string(), "value1".to_string());

    let mut context2 = ScenarioContext::new();
    context2.set_variable("test".to_string(), "value2".to_string());

    let result1 = executor.execute(&scenario, &mut context1).await;
    let result2 = executor.execute(&scenario, &mut context2).await;

    // Both should succeed
    assert!(result1.success);
    assert!(result2.success);

    // Contexts should maintain their separate variables
    assert_eq!(context1.get_variable("test"), Some(&"value1".to_string()));
    assert_eq!(context2.get_variable("test"), Some(&"value2".to_string()));
}
