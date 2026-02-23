//! Integration tests for response assertions framework (Issue #30).
//!
//! These tests validate that assertions work correctly against a live API,
//! including proper failure detection, metrics tracking, and mixed scenarios.
//!
//! **NOTE**: Most tests use httpbin.org (public testing API).
//! E-commerce specific tests require ecom.edge.baugus-lab.com and are marked #[ignore].

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{Assertion, RequestConfig, Scenario, ScenarioContext, Step};
use std::collections::HashMap;
use std::time::Duration;

// Public testing API - always available
const HTTPBIN_URL: &str = "https://httpbin.org";
// E-commerce test API - may not be accessible in all environments
const ECOM_URL: &str = "https://ecom.edge.baugus-lab.com";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
async fn test_status_code_assertion_pass() {
    let scenario = Scenario {
        name: "Status Code Assertion - Pass".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get 200 Response".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/status/200".to_string(), // httpbin returns 200
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::StatusCode(200)],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps.len(), 1);
    assert!(result.steps[0].success);
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ Status code assertion passed");
}

#[tokio::test]
async fn test_status_code_assertion_fail() {
    let scenario = Scenario {
        name: "Status Code Assertion - Fail".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Expect 404".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/status/200".to_string(), // Returns 200, not 404
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::StatusCode(404)],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(!result.success, "Scenario should fail due to assertion");
    assert_eq!(result.steps.len(), 1);
    assert!(!result.steps[0].success);
    assert_eq!(result.steps[0].assertions_passed, 0);
    assert_eq!(result.steps[0].assertions_failed, 1);
    assert!(result.steps[0].error.is_some());

    println!("✅ Status code assertion correctly failed");
}

#[tokio::test]

async fn test_response_time_assertion_pass() {
    let scenario = Scenario {
        name: "Response Time Assertion - Pass".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Fast Response".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::ResponseTime(Duration::from_secs(5))],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!(
        "✅ Response time assertion passed ({}ms < 5000ms)",
        result.steps[0].response_time_ms
    );
}

#[tokio::test]
async fn test_response_time_assertion_fail() {
    let scenario = Scenario {
        name: "Response Time Assertion - Fail".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Unrealistic Threshold".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::ResponseTime(Duration::from_millis(1))],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(!result.success, "Scenario should fail due to slow response");
    assert_eq!(result.steps[0].assertions_passed, 0);
    assert_eq!(result.steps[0].assertions_failed, 1);

    println!(
        "✅ Response time assertion correctly failed ({}ms > 1ms)",
        result.steps[0].response_time_ms
    );
}

#[tokio::test]

async fn test_json_path_assertion_existence() {
    let scenario = Scenario {
        name: "JSONPath Existence".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Field Exists".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::JsonPath {
                path: "$.slideshow".to_string(),
                expected: None, // Just check it exists
            }],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ JSONPath existence assertion passed");
}

#[tokio::test]

async fn test_json_path_assertion_value_match() {
    let scenario = Scenario {
        name: "JSONPath Value Match".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check JSON Value".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::JsonPath {
                path: "$.slideshow.title".to_string(),
                expected: Some("Sample Slide Show".to_string()),
            }],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ JSONPath value match assertion passed");
}

#[tokio::test]
async fn test_json_path_assertion_value_mismatch() {
    let scenario = Scenario {
        name: "JSONPath Value Mismatch".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Wrong Value".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::JsonPath {
                path: "$.slideshow.title".to_string(),
                expected: Some("Wrong Title".to_string()), // Should be "Sample Slide Show"
            }],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(
        !result.success,
        "Scenario should fail due to value mismatch"
    );
    assert_eq!(result.steps[0].assertions_passed, 0);
    assert_eq!(result.steps[0].assertions_failed, 1);

    println!("✅ JSONPath value mismatch correctly failed");
}

#[tokio::test]

async fn test_body_contains_assertion_pass() {
    let scenario = Scenario {
        name: "Body Contains - Pass".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Response Contains Text".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::BodyContains("slideshow".to_string())],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ Body contains assertion passed");
}

#[tokio::test]
async fn test_body_contains_assertion_fail() {
    let scenario = Scenario {
        name: "Body Contains - Fail".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Missing Text".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::BodyContains("MISSING_TEXT_XYZ".to_string())],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(!result.success, "Scenario should fail");
    assert_eq!(result.steps[0].assertions_passed, 0);
    assert_eq!(result.steps[0].assertions_failed, 1);

    println!("✅ Body contains assertion correctly failed");
}

#[tokio::test]

async fn test_body_matches_regex_assertion() {
    let scenario = Scenario {
        name: "Body Matches Regex".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check JSON Pattern".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::BodyMatches(
                r#""slideshow"\s*:\s*\{"#.to_string(),
            )],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ Body matches regex assertion passed");
}

#[tokio::test]
async fn test_header_exists_assertion_pass() {
    let scenario = Scenario {
        name: "Header Exists - Pass".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Content-Type Header".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/headers".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::HeaderExists("content-type".to_string())],
            think_time: None,
        }],
    };

    // Retry up to 3 times to tolerate transient httpbin.org failures in CI.
    let result = {
        let mut last = None;
        for attempt in 1..=3 {
            let client = create_test_client();
            let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
            let mut context = ScenarioContext::new();
            let r = executor.execute(&scenario, &mut context).await;
            if r.success {
                last = Some(r);
                break;
            }
            eprintln!("Attempt {attempt}/3 failed — retrying");
        }
        last.expect("All 3 attempts failed")
    };

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 1);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ Header exists assertion passed");
}

#[tokio::test]
async fn test_header_exists_assertion_fail() {
    let scenario = Scenario {
        name: "Header Exists - Fail".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Check Missing Header".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/headers".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::HeaderExists("x-missing-header".to_string())],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(!result.success, "Scenario should fail");
    assert_eq!(result.steps[0].assertions_passed, 0);
    assert_eq!(result.steps[0].assertions_failed, 1);

    println!("✅ Header exists assertion correctly failed");
}

#[tokio::test]

async fn test_multiple_assertions_all_pass() {
    let scenario = Scenario {
        name: "Multiple Assertions - All Pass".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Multiple Checks".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![
                Assertion::StatusCode(200),
                Assertion::ResponseTime(Duration::from_secs(5)),
                Assertion::JsonPath {
                    path: "$.url".to_string(),
                    expected: None, // Just check it exists
                },
                Assertion::BodyContains("headers".to_string()),
                Assertion::HeaderExists("content-type".to_string()),
            ],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps[0].assertions_passed, 5);
    assert_eq!(result.steps[0].assertions_failed, 0);

    println!("✅ All 5 assertions passed");
}

#[tokio::test]

async fn test_multiple_assertions_mixed_results() {
    let scenario = Scenario {
        name: "Multiple Assertions - Mixed".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Mixed Results".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![
                Assertion::StatusCode(200),                     // PASS
                Assertion::BodyContains("headers".to_string()), // PASS
                Assertion::StatusCode(404),                     // FAIL
                Assertion::BodyContains("MISSING".to_string()), // FAIL
            ],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(
        !result.success,
        "Scenario should fail (2 failed assertions)"
    );
    assert_eq!(result.steps[0].assertions_passed, 2);
    assert_eq!(result.steps[0].assertions_failed, 2);

    println!("✅ Mixed assertions: 2 passed, 2 failed as expected");
}

#[tokio::test]

async fn test_multi_step_assertion_stops_on_failure() {
    let scenario = Scenario {
        name: "Multi-Step with Assertion Failure".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Step 1 - Pass".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/status/200".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                think_time: None,
            },
            Step {
                name: "Step 2 - Fail".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/status/200".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(404)], // Will fail
                think_time: None,
            },
            Step {
                name: "Step 3 - Never Reached".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
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
    let executor = ScenarioExecutor::new(HTTPBIN_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(!result.success, "Scenario should fail");
    assert_eq!(
        result.steps_completed, 1,
        "Should stop after step 2 failure"
    );
    assert_eq!(result.steps.len(), 2, "Should only have 2 step results");
    assert_eq!(result.failed_at_step, Some(1));

    // Step 1 should pass
    assert!(result.steps[0].success);
    assert_eq!(result.steps[0].assertions_passed, 1);

    // Step 2 should fail
    assert!(!result.steps[1].success);
    assert_eq!(result.steps[1].assertions_failed, 1);

    println!("✅ Execution correctly stopped after assertion failure in step 2");
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_realistic_e_commerce_flow_with_assertions() {
    let scenario = Scenario {
        name: "E-Commerce Flow with Assertions".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Health Check".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::ResponseTime(Duration::from_secs(2)),
                ],
                think_time: None,
            },
            Step {
                name: "Get Products".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=10".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::ResponseTime(Duration::from_secs(3)),
                    Assertion::BodyContains("id".to_string()),
                    Assertion::BodyContains("name".to_string()),
                    Assertion::HeaderExists("content-type".to_string()),
                ],
                think_time: None,
            },
            Step {
                name: "Check Status".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/status".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::JsonPath {
                        path: "$.status".to_string(),
                        expected: Some("ok".to_string()),
                    },
                    Assertion::BodyMatches(r#""status"\s*:\s*"ok""#.to_string()),
                ],
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(ECOM_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "E-commerce flow should succeed");
    assert_eq!(result.steps_completed, 3);

    // Verify assertion counts
    assert_eq!(result.steps[0].assertions_passed, 2);
    assert_eq!(result.steps[1].assertions_passed, 5);
    assert_eq!(result.steps[2].assertions_passed, 3);

    let total_assertions_passed: usize = result.steps.iter().map(|s| s.assertions_passed).sum();

    println!(
        "✅ E-commerce flow completed with {} total assertions passing",
        total_assertions_passed
    );
}
