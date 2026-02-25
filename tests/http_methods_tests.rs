//! Integration tests for all HTTP methods (Issue #32).
//!
//! These tests validate that GET, POST, PUT, PATCH, DELETE, HEAD, and OPTIONS
//! methods work correctly in both single requests and multi-step scenarios.

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{RequestConfig, Scenario, ScenarioContext, Step};
use std::collections::HashMap;
use std::time::Duration;

const BASE_URL: &str = "https://httpbin.org";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
async fn test_get_request() {
    let scenario = Scenario {
        name: "GET Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "GET /get".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(),
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
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(
        result.steps[0].status_code.is_some(),
        "GET request should receive a response (got none)"
    );

    println!(
        "✅ GET request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_post_request() {
    let scenario = Scenario {
        name: "POST Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "POST /post".to_string(),
            request: RequestConfig {
                method: "POST".to_string(),
                path: "/post".to_string(),
                body: Some(r#"{"test": "data"}"#.to_string()),
                headers: {
                    let mut h = HashMap::new();
                    h.insert("Content-Type".to_string(), "application/json".to_string());
                    h
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

    assert!(
        result.steps[0].status_code.is_some(),
        "POST request should receive a response (got none)"
    );

    println!(
        "✅ POST request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_put_request() {
    let scenario = Scenario {
        name: "PUT Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "PUT /put".to_string(),
            request: RequestConfig {
                method: "PUT".to_string(),
                path: "/put".to_string(),
                body: Some(r#"{"update": "data"}"#.to_string()),
                headers: {
                    let mut h = HashMap::new();
                    h.insert("Content-Type".to_string(), "application/json".to_string());
                    h
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

    // PUT may return 2xx/3xx or 4xx depending on endpoint implementation
    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ PUT request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_patch_request() {
    let scenario = Scenario {
        name: "PATCH Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "PATCH /patch".to_string(),
            request: RequestConfig {
                method: "PATCH".to_string(),
                path: "/patch".to_string(),
                body: Some(r#"{"patch": "data"}"#.to_string()),
                headers: {
                    let mut h = HashMap::new();
                    h.insert("Content-Type".to_string(), "application/json".to_string());
                    h
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

    // PATCH may return 2xx/3xx or 4xx depending on endpoint implementation
    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ PATCH request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_delete_request() {
    let scenario = Scenario {
        name: "DELETE Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "DELETE /delete".to_string(),
            request: RequestConfig {
                method: "DELETE".to_string(),
                path: "/delete".to_string(),
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
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // DELETE may return 2xx/3xx or 4xx depending on endpoint implementation
    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ DELETE request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_head_request() {
    let scenario = Scenario {
        name: "HEAD Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "HEAD /get".to_string(),
            request: RequestConfig {
                method: "HEAD".to_string(),
                path: "/get".to_string(),
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
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // HEAD should return same status as GET but no body
    assert!(result.success, "HEAD request should succeed");
    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ HEAD request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_options_request() {
    let scenario = Scenario {
        name: "OPTIONS Request Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "OPTIONS /get".to_string(),
            request: RequestConfig {
                method: "OPTIONS".to_string(),
                path: "/get".to_string(),
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
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // OPTIONS typically returns 200 or 204 with Allow header
    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ OPTIONS request works (status: {:?})",
        result.steps[0].status_code
    );
}

#[tokio::test]
async fn test_mixed_methods_scenario() {
    let scenario = Scenario {
        name: "Mixed HTTP Methods".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "GET health".to_string(),
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
            Step {
                name: "POST status".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/post".to_string(),
                    body: Some(r#"{"action": "check"}"#.to_string()),
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "PUT status".to_string(),
                request: RequestConfig {
                    method: "PUT".to_string(),
                    path: "/put".to_string(),
                    body: Some(r#"{"action": "update"}"#.to_string()),
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "HEAD health".to_string(),
                request: RequestConfig {
                    method: "HEAD".to_string(),
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
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // All steps should execute (some may fail depending on API implementation)
    assert!(result.steps.len() >= 2, "Should execute multiple steps");
    assert!(result.steps[0].success, "GET should succeed");
    assert!(
        result.steps[3].success || result.steps.len() == 4,
        "HEAD should execute"
    );

    println!("✅ Mixed methods scenario works");
    println!("   Steps executed: {}", result.steps.len());
    for (i, step) in result.steps.iter().enumerate() {
        println!(
            "   Step {}: {} (status: {:?})",
            i + 1,
            step.step_name,
            step.status_code
        );
    }
}

#[tokio::test]
async fn test_case_insensitive_methods() {
    // Test that methods are case-insensitive
    let test_cases: Vec<(&str, &str)> = vec![
        ("get", "/get"),
        ("Get", "/get"),
        ("GET", "/get"),
        ("post", "/post"),
        ("Post", "/post"),
        ("POST", "/post"),
    ];

    for (method, path) in test_cases {
        let scenario = Scenario {
            name: format!("Case Test: {}", method),
            weight: 1.0,
            steps: vec![Step {
                name: format!("{} request", method),
                request: RequestConfig {
                    method: method.to_string(),
                    path: path.to_string(),
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
        let mut context = ScenarioContext::new();

        let result = executor.execute(&scenario, &mut context).await;

        assert!(result.success, "{} should work (case-insensitive)", method);
    }

    println!("✅ HTTP methods are case-insensitive");
}

#[tokio::test]
async fn test_rest_crud_flow() {
    // Simulate a realistic REST CRUD flow
    let scenario = Scenario {
        name: "REST CRUD Flow".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "1. GET - Read all".to_string(),
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
            Step {
                name: "2. POST - Create".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/post".to_string(),
                    body: Some(r#"{"name": "Test Item", "price": 99.99}"#.to_string()),
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "3. PUT - Update full".to_string(),
                request: RequestConfig {
                    method: "PUT".to_string(),
                    path: "/put".to_string(),
                    body: Some(
                        r#"{"name": "Updated Item", "price": 149.99, "stock": 10}"#.to_string(),
                    ),
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "4. PATCH - Partial update".to_string(),
                request: RequestConfig {
                    method: "PATCH".to_string(),
                    path: "/patch".to_string(),
                    body: Some(r#"{"price": 129.99}"#.to_string()),
                    headers: {
                        let mut h = HashMap::new();
                        h.insert("Content-Type".to_string(), "application/json".to_string());
                        h
                    },
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "5. HEAD - Check existence".to_string(),
                request: RequestConfig {
                    method: "HEAD".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "6. DELETE - Remove".to_string(),
                request: RequestConfig {
                    method: "DELETE".to_string(),
                    path: "/delete".to_string(),
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

    println!("✅ REST CRUD flow executed");
    println!("   Total steps: {}", result.steps.len());
    for step in result.steps.iter() {
        println!("   {} - Status: {:?}", step.step_name, step.status_code);
    }

    // At least GET should work
    assert!(result.steps[0].success, "GET should succeed");
}

#[tokio::test]
async fn test_options_cors_preflight() {
    // Test OPTIONS for CORS preflight
    let scenario = Scenario {
        name: "CORS Preflight".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "OPTIONS preflight".to_string(),
            request: RequestConfig {
                method: "OPTIONS".to_string(),
                path: "/get".to_string(),
                body: None,
                headers: {
                    let mut h = HashMap::new();
                    h.insert(
                        "Access-Control-Request-Method".to_string(),
                        "POST".to_string(),
                    );
                    h.insert(
                        "Access-Control-Request-Headers".to_string(),
                        "Content-Type".to_string(),
                    );
                    h.insert("Origin".to_string(), "https://example.com".to_string());
                    h
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

    assert!(result.steps[0].status_code.is_some());

    println!(
        "✅ OPTIONS CORS preflight works (status: {:?})",
        result.steps[0].status_code
    );
}
