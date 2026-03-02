//! Integration tests for variable extraction (#27).
//!
//! These tests validate JSONPath, Regex, Header, and Cookie extraction
//! from HTTP responses against httpbin.org.

use rust_loadtest::executor::{ScenarioExecutor, SessionStore};
use rust_loadtest::scenario::{
    Extractor, RequestConfig, Scenario, ScenarioContext, Step, ThinkTime, VariableExtraction,
};
use std::collections::HashMap;
use std::time::Duration;

const BASE_URL: &str = "https://httpbin.org";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
async fn test_jsonpath_extraction_from_products() {
    // httpbin /json returns {"slideshow": {"author": "...", "title": "...", ...}}
    let scenario = Scenario {
        name: "JSONPath Extraction Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get JSON and Extract Fields".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![
                VariableExtraction {
                    name: "author".to_string(),
                    extractor: Extractor::JsonPath("$.slideshow.author".to_string()),
                },
                VariableExtraction {
                    name: "title".to_string(),
                    extractor: Extractor::JsonPath("$.slideshow.title".to_string()),
                },
            ],
            assertions: vec![],
            cache: None,
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    assert!(result.success, "Scenario should succeed");

    // Verify variables were extracted
    assert!(
        context.get_variable("author").is_some(),
        "Should extract author"
    );
    assert!(
        context.get_variable("title").is_some(),
        "Should extract title"
    );

    println!("Extracted author: {:?}", context.get_variable("author"));
    println!("Extracted title: {:?}", context.get_variable("title"));
}

#[tokio::test]
async fn test_extraction_and_reuse_in_next_step() {
    // Extract the origin IP from /get and reuse it as a query param in the next step
    let scenario = Scenario {
        name: "Extract and Reuse".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Get Origin IP".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![VariableExtraction {
                    name: "origin_ip".to_string(),
                    extractor: Extractor::JsonPath("$.origin".to_string()),
                }],
                assertions: vec![],
                cache: None,
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(100))),
            },
            Step {
                name: "Use Extracted Value".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get?origin=${origin_ip}".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    assert!(result.success, "Both steps should succeed");
    assert_eq!(result.steps_completed, 2, "Should complete both steps");

    // Verify origin IP was extracted
    let origin_ip = context.get_variable("origin_ip");
    assert!(origin_ip.is_some(), "Should extract origin IP");

    println!("Extracted and reused origin_ip: {:?}", origin_ip);

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
                path: "/get".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![VariableExtraction {
                name: "content_type".to_string(),
                extractor: Extractor::Header("content-type".to_string()),
            }],
            assertions: vec![],
            cache: None,
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

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
    // httpbin /json returns {"slideshow": {"author": "...", "date": "...", "title": "...", ...}}
    let scenario = Scenario {
        name: "Multiple Extractions".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Get JSON with Multiple Extractions".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/json".to_string(),
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![
                VariableExtraction {
                    name: "author".to_string(),
                    extractor: Extractor::JsonPath("$.slideshow.author".to_string()),
                },
                VariableExtraction {
                    name: "title".to_string(),
                    extractor: Extractor::JsonPath("$.slideshow.title".to_string()),
                },
                VariableExtraction {
                    name: "content_type".to_string(),
                    extractor: Extractor::Header("content-type".to_string()),
                },
            ],
            assertions: vec![],
            cache: None,
            think_time: None,
        }],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    assert!(result.success, "Should succeed");

    // Verify all extractions worked
    assert!(
        context.get_variable("author").is_some(),
        "Should extract author"
    );
    assert!(
        context.get_variable("title").is_some(),
        "Should extract title"
    );
    assert!(
        context.get_variable("content_type").is_some(),
        "Should extract content_type"
    );

    println!("Extracted variables:");
    println!("  author: {:?}", context.get_variable("author"));
    println!("  title: {:?}", context.get_variable("title"));
    println!("  content_type: {:?}", context.get_variable("content_type"));
}

#[tokio::test]
async fn test_shopping_flow_with_extraction() {
    // Realistic multi-step flow using variable extraction with httpbin
    let scenario = Scenario {
        name: "Multi-Step Flow with Extraction".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Get JSON Data".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![VariableExtraction {
                    name: "author".to_string(),
                    extractor: Extractor::JsonPath("$.slideshow.author".to_string()),
                }],
                assertions: vec![],
                cache: None,
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Post Data with Extracted Value".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/post".to_string(),
                    body: Some(
                        r#"{
                            "author": "${author}",
                            "timestamp": "${timestamp}"
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
                    name: "post_url".to_string(),
                    extractor: Extractor::JsonPath("$.url".to_string()),
                }],
                assertions: vec![],
                cache: None,
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Final GET".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![VariableExtraction {
                    name: "final_origin".to_string(),
                    extractor: Extractor::JsonPath("$.origin".to_string()),
                }],
                assertions: vec![],
                cache: None,
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    // All steps should succeed
    assert!(result.success, "Multi-step flow should succeed");
    assert_eq!(result.steps_completed, 3);

    // Verify extractions
    assert!(context.get_variable("author").is_some());
    assert!(context.get_variable("post_url").is_some());
    assert!(context.get_variable("final_origin").is_some());

    println!("\nMulti-Step Flow Extracted Variables:");
    println!("  author: {:?}", context.get_variable("author"));
    println!("  post_url: {:?}", context.get_variable("post_url"));
    println!("  final_origin: {:?}", context.get_variable("final_origin"));
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
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![
                    VariableExtraction {
                        name: "author".to_string(),
                        extractor: Extractor::JsonPath("$.slideshow.author".to_string()),
                    },
                    VariableExtraction {
                        name: "nonexistent".to_string(),
                        extractor: Extractor::JsonPath("$.does.not.exist".to_string()),
                    },
                ],
                assertions: vec![],
                cache: None,
                think_time: None,
            },
            Step {
                name: "Next Step".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
                think_time: None,
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    // Scenario should still succeed
    assert!(
        result.success,
        "Scenario should succeed even with failed extraction"
    );
    assert_eq!(result.steps_completed, 2);

    // author should be extracted
    assert!(context.get_variable("author").is_some());

    // nonexistent should NOT be in context (extraction failed)
    assert!(context.get_variable("nonexistent").is_none());
}
