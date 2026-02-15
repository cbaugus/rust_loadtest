//! Integration tests for variable extraction (#27).
//!
//! These tests validate JSONPath, Regex, Header, and Cookie extraction
//! from HTTP responses against the live mock API.

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{
    Extractor, RequestConfig, Scenario, ScenarioContext, Step, ThinkTime, VariableExtraction,
};
use std::collections::HashMap;
use std::time::Duration;

const BASE_URL: &str = "https://ecom.edge.baugus-lab.com";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
async fn test_jsonpath_extraction_from_products() {
    let scenario = Scenario {
        name: "JSONPath Extraction Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get Products and Extract ID".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/products?limit=1".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![
                VariableExtraction {
                    name: "product_id".to_string(),
                    extractor: Extractor::JsonPath("$.products[0].id".to_string()),
                },
                VariableExtraction {
                    name: "product_name".to_string(),
                    extractor: Extractor::JsonPath("$.products[0].name".to_string()),
                },
            ],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Scenario should succeed");

    // Verify variables were extracted
    assert!(
        context.get_variable("product_id").is_some(),
        "Should extract product_id"
    );
    assert!(
        context.get_variable("product_name").is_some(),
        "Should extract product_name"
    );

    println!(
        "Extracted product_id: {:?}",
        context.get_variable("product_id")
    );
    println!(
        "Extracted product_name: {:?}",
        context.get_variable("product_name")
    );
}

#[tokio::test]
async fn test_extraction_and_reuse_in_next_step() {
    // This is the key test: extract a value and use it in a subsequent request
    let scenario = Scenario {
        name: "Extract and Reuse".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Get Products List".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=5".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![VariableExtraction {
                    name: "first_product_id".to_string(),
                    extractor: Extractor::JsonPath("$.products[0].id".to_string()),
                }],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(100))),
            },
            Step {
                name: "Get Product Details Using Extracted ID".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    // Use the extracted product ID in the path
                    path: "/products/${first_product_id}".to_string(),
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

    assert!(result.success, "Both steps should succeed");
    assert_eq!(result.steps_completed, 2, "Should complete both steps");

    // Verify product ID was extracted
    let product_id = context.get_variable("first_product_id");
    assert!(product_id.is_some(), "Should extract product ID");

    println!("Extracted and reused product_id: {:?}", product_id);

    // Both steps should have succeeded
    assert!(result.steps[0].success, "First step should succeed");
    assert!(
        result.steps[1].success,
        "Second step (using extracted var) should succeed"
    );
}

#[tokio::test]
async fn test_header_extraction() {
    let scenario = Scenario {
        name: "Header Extraction Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get Response with Headers".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/health".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![VariableExtraction {
                name: "content_type".to_string(),
                extractor: Extractor::Header("content-type".to_string()),
            }],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Should succeed");

    // Content-type header should be extracted
    let content_type = context.get_variable("content_type");
    assert!(content_type.is_some(), "Should extract content-type header");

    if let Some(ct) = content_type {
        println!("Extracted content-type: {}", ct);
        assert!(
            ct.contains("json") || ct.contains("text"),
            "Content-type should be a valid MIME type"
        );
    }
}

#[tokio::test]
async fn test_multiple_extractions_in_single_step() {
    let scenario = Scenario {
        name: "Multiple Extractions".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get Status with Multiple Extractions".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/status".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![
                VariableExtraction {
                    name: "status".to_string(),
                    extractor: Extractor::JsonPath("$.status".to_string()),
                },
                VariableExtraction {
                    name: "version".to_string(),
                    extractor: Extractor::JsonPath("$.version".to_string()),
                },
                VariableExtraction {
                    name: "content_type".to_string(),
                    extractor: Extractor::Header("content-type".to_string()),
                },
            ],
            assertions: vec![],
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    assert!(result.success, "Should succeed");

    // Verify all extractions worked
    assert!(
        context.get_variable("status").is_some(),
        "Should extract status"
    );
    assert!(
        context.get_variable("version").is_some(),
        "Should extract version"
    );
    assert!(
        context.get_variable("content_type").is_some(),
        "Should extract content_type"
    );

    println!("Extracted variables:");
    println!("  status: {:?}", context.get_variable("status"));
    println!("  version: {:?}", context.get_variable("version"));
    println!("  content_type: {:?}", context.get_variable("content_type"));
}

#[tokio::test]
async fn test_shopping_flow_with_extraction() {
    // Realistic e-commerce flow using variable extraction
    let scenario = Scenario {
        name: "Shopping Flow with Extraction".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Browse Products".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=3".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![VariableExtraction {
                    name: "product_id".to_string(),
                    extractor: Extractor::JsonPath("$.products[0].id".to_string()),
                }],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "View Product Details".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products/${product_id}".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![
                    VariableExtraction {
                        name: "price".to_string(),
                        extractor: Extractor::JsonPath("$.price".to_string()),
                    },
                    VariableExtraction {
                        name: "name".to_string(),
                        extractor: Extractor::JsonPath("$.name".to_string()),
                    },
                ],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(1000))),
            },
            Step {
                name: "Register User".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/auth/register".to_string(),
                    body: Some(
                        r#"{
                            "email": "test-${timestamp}@example.com",
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
                extractions: vec![VariableExtraction {
                    name: "auth_token".to_string(),
                    extractor: Extractor::JsonPath("$.token".to_string()),
                }],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // All steps should succeed
    assert!(result.success, "Shopping flow should succeed");
    assert_eq!(result.steps_completed, 3);

    // Verify all extractions
    assert!(context.get_variable("product_id").is_some());
    assert!(context.get_variable("price").is_some());
    assert!(context.get_variable("name").is_some());
    assert!(context.get_variable("auth_token").is_some());

    println!("\nShopping Flow Extracted Variables:");
    println!("  product_id: {:?}", context.get_variable("product_id"));
    println!("  price: {:?}", context.get_variable("price"));
    println!("  name: {:?}", context.get_variable("name"));
    println!("  auth_token: {:?}", context.get_variable("auth_token"));
}

#[tokio::test]
async fn test_extraction_failure_doesnt_stop_scenario() {
    // Test that failed extraction doesn't stop the scenario
    let scenario = Scenario {
        name: "Partial Extraction Failure".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Step with Mixed Extractions".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=1".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![
                    VariableExtraction {
                        name: "product_id".to_string(),
                        extractor: Extractor::JsonPath("$.products[0].id".to_string()),
                    },
                    VariableExtraction {
                        name: "nonexistent".to_string(),
                        extractor: Extractor::JsonPath("$.does.not.exist".to_string()),
                    },
                ],
                assertions: vec![],
                think_time: None,
            },
            Step {
                name: "Next Step".to_string(),
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
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Scenario should still succeed
    assert!(
        result.success,
        "Scenario should succeed even with failed extraction"
    );
    assert_eq!(result.steps_completed, 2);

    // product_id should be extracted
    assert!(context.get_variable("product_id").is_some());

    // nonexistent should NOT be in context (extraction failed)
    assert!(context.get_variable("nonexistent").is_none());
}
