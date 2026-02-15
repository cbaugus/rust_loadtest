//! Integration tests for error categorization (Issue #34).
//!
//! These tests validate that errors are properly categorized into
//! client errors, server errors, network errors, timeouts, and TLS errors.

use rust_loadtest::errors::{categorize_status_code, CategorizedError, ErrorCategory};
use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{Assertion, RequestConfig, Scenario, ScenarioContext, Step};
use std::collections::HashMap;
use std::time::Duration;

const BASE_URL: &str = "https://ecom.edge.baugus-lab.com";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[test]
fn test_categorize_2xx_success() {
    assert_eq!(ErrorCategory::from_status_code(200), None);
    assert_eq!(ErrorCategory::from_status_code(201), None);
    assert_eq!(ErrorCategory::from_status_code(204), None);
    println!("✅ 2xx codes not categorized as errors");
}

#[test]
fn test_categorize_3xx_redirection() {
    assert_eq!(ErrorCategory::from_status_code(301), None);
    assert_eq!(ErrorCategory::from_status_code(302), None);
    assert_eq!(ErrorCategory::from_status_code(304), None);
    println!("✅ 3xx codes not categorized as errors");
}

#[test]
fn test_categorize_4xx_client_errors() {
    assert_eq!(
        ErrorCategory::from_status_code(400),
        Some(ErrorCategory::ClientError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(401),
        Some(ErrorCategory::ClientError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(403),
        Some(ErrorCategory::ClientError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(404),
        Some(ErrorCategory::ClientError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(429),
        Some(ErrorCategory::ClientError)
    );

    println!("✅ 4xx codes categorized as client errors");
}

#[test]
fn test_categorize_5xx_server_errors() {
    assert_eq!(
        ErrorCategory::from_status_code(500),
        Some(ErrorCategory::ServerError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(502),
        Some(ErrorCategory::ServerError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(503),
        Some(ErrorCategory::ServerError)
    );
    assert_eq!(
        ErrorCategory::from_status_code(504),
        Some(ErrorCategory::ServerError)
    );

    println!("✅ 5xx codes categorized as server errors");
}

#[test]
fn test_error_category_labels() {
    assert_eq!(ErrorCategory::ClientError.label(), "client_error");
    assert_eq!(ErrorCategory::ServerError.label(), "server_error");
    assert_eq!(ErrorCategory::NetworkError.label(), "network_error");
    assert_eq!(ErrorCategory::TimeoutError.label(), "timeout_error");
    assert_eq!(ErrorCategory::TlsError.label(), "tls_error");
    assert_eq!(ErrorCategory::OtherError.label(), "other_error");

    println!("✅ Error category labels correct");
}

#[test]
fn test_error_category_descriptions() {
    assert!(ErrorCategory::ClientError.description().contains("4xx"));
    assert!(ErrorCategory::ServerError.description().contains("5xx"));
    assert!(ErrorCategory::NetworkError
        .description()
        .contains("Network"));
    assert!(ErrorCategory::TimeoutError
        .description()
        .contains("Timeout"));
    assert!(ErrorCategory::TlsError.description().contains("TLS"));

    println!("✅ Error category descriptions correct");
}

#[test]
fn test_categorized_error_from_status() {
    let err = CategorizedError::from_status(
        404,
        "Not Found".to_string(),
        Some("/api/missing".to_string()),
    )
    .unwrap();

    assert_eq!(err.category, ErrorCategory::ClientError);
    assert_eq!(err.status_code, Some(404));
    assert_eq!(err.message, "Not Found");
    assert_eq!(err.endpoint, Some("/api/missing".to_string()));

    println!("✅ CategorizedError from status works");
}

#[test]
fn test_categorized_error_display() {
    let err = CategorizedError::new(
        ErrorCategory::ServerError,
        "Service temporarily unavailable".to_string(),
    );

    let display = format!("{}", err);
    assert!(display.contains("server_error"));
    assert!(display.contains("Service temporarily unavailable"));

    println!("✅ CategorizedError display formatting works");
}

#[test]
fn test_all_error_categories() {
    let categories = ErrorCategory::all();

    assert_eq!(categories.len(), 6);
    assert!(categories.contains(&ErrorCategory::ClientError));
    assert!(categories.contains(&ErrorCategory::ServerError));
    assert!(categories.contains(&ErrorCategory::NetworkError));
    assert!(categories.contains(&ErrorCategory::TimeoutError));
    assert!(categories.contains(&ErrorCategory::TlsError));
    assert!(categories.contains(&ErrorCategory::OtherError));

    println!("✅ All error categories enumerated");
}

#[test]
fn test_status_code_names() {
    assert_eq!(categorize_status_code(200), "OK");
    assert_eq!(categorize_status_code(404), "Not Found");
    assert_eq!(categorize_status_code(500), "Internal Server Error");
    assert_eq!(categorize_status_code(503), "Service Unavailable");
    assert_eq!(categorize_status_code(429), "Too Many Requests");

    println!("✅ Status code name mapping works");
}

#[tokio::test]
async fn test_404_error_categorization() {
    let scenario = Scenario {
        name: "404 Error Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Request non-existent endpoint".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/this-endpoint-does-not-exist-12345".to_string(),
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

    // Request should "succeed" (no network error) but return 404
    assert_eq!(result.steps[0].status_code, Some(404));

    // Error should be categorized as ClientError
    if let Some(category) = ErrorCategory::from_status_code(404) {
        assert_eq!(category, ErrorCategory::ClientError);
    }

    println!("✅ 404 error properly categorized as client error");
}

#[tokio::test]
async fn test_timeout_error_categorization() {
    let scenario = Scenario {
        name: "Timeout Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Request with very short timeout".to_string(),
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

    // Create client with extremely short timeout to force timeout
    let client = reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_micros(1)) // 1 microsecond - guaranteed to timeout
        .build()
        .expect("Failed to create client");

    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Should fail due to timeout
    assert!(!result.success);
    assert!(result.steps[0].error.is_some());

    println!("✅ Timeout error detected (may be categorized as timeout or network)");
}

#[tokio::test]
async fn test_network_error_categorization() {
    let scenario = Scenario {
        name: "Network Error Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Request to invalid host".to_string(),
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
    // Use invalid base URL to trigger network error
    let executor = ScenarioExecutor::new(
        "https://invalid-host-that-does-not-exist-12345.com".to_string(),
        client,
    );
    let mut context = ScenarioContext::new();

    let result = executor.execute(&scenario, &mut context).await;

    // Should fail due to DNS/network error
    assert!(!result.success);
    assert!(result.steps[0].error.is_some());
    assert_eq!(result.steps[0].status_code, None);

    println!("✅ Network error detected for invalid host");
}

#[tokio::test]
async fn test_mixed_error_types_in_scenario() {
    let scenario = Scenario {
        name: "Mixed Errors Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Success".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                think_time: None,
            },
            Step {
                name: "404 Client Error".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/nonexistent".to_string(),
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

    // First step succeeds
    assert!(result.steps[0].success);
    assert_eq!(result.steps[0].status_code, Some(200));

    // Second step completes but returns 404
    if result.steps.len() > 1 {
        assert_eq!(result.steps[1].status_code, Some(404));

        let category = ErrorCategory::from_status_code(404).unwrap();
        assert_eq!(category, ErrorCategory::ClientError);
    }

    println!("✅ Mixed success and error types handled correctly");
}

#[test]
fn test_error_category_equality() {
    assert_eq!(ErrorCategory::ClientError, ErrorCategory::ClientError);
    assert_ne!(ErrorCategory::ClientError, ErrorCategory::ServerError);
    assert_ne!(ErrorCategory::NetworkError, ErrorCategory::TimeoutError);

    println!("✅ Error category equality works");
}

#[test]
fn test_error_category_hash() {
    use std::collections::HashMap;

    let mut map = HashMap::new();
    map.insert(ErrorCategory::ClientError, 10);
    map.insert(ErrorCategory::ServerError, 20);

    assert_eq!(map.get(&ErrorCategory::ClientError), Some(&10));
    assert_eq!(map.get(&ErrorCategory::ServerError), Some(&20));

    println!("✅ Error category can be used as HashMap key");
}

#[test]
fn test_categorized_error_with_endpoint() {
    let err = CategorizedError::from_status(
        503,
        "Service Unavailable".to_string(),
        Some("/api/critical".to_string()),
    )
    .unwrap();

    assert_eq!(err.category, ErrorCategory::ServerError);
    assert_eq!(err.endpoint, Some("/api/critical".to_string()));

    println!("✅ CategorizedError includes endpoint information");
}

#[test]
fn test_categorized_error_new() {
    let err = CategorizedError::new(
        ErrorCategory::TlsError,
        "Certificate verification failed".to_string(),
    );

    assert_eq!(err.category, ErrorCategory::TlsError);
    assert_eq!(err.status_code, None);
    assert!(err.message.contains("Certificate"));

    println!("✅ CategorizedError::new works");
}
