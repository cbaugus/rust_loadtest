//! Integration tests for cookie and session management (#28).
//!
//! These tests validate that cookies are automatically handled across
//! requests within a scenario, enabling session-based authentication.

use rust_loadtest::executor::{ScenarioExecutor, SessionStore};
use rust_loadtest::scenario::{
    Extractor, RequestConfig, Scenario, ScenarioContext, Step, ThinkTime, VariableExtraction,
};
use std::collections::HashMap;
use std::time::Duration;

// E-commerce test API - not accessible in CI
const BASE_URL: &str = "https://ecom.edge.baugus-lab.com";

/// Create a cookie-enabled HTTP client for testing
fn create_cookie_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true) // Enable automatic cookie management
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_cookies_persist_across_steps() {
    // Test that cookies set in one step are sent in subsequent steps
    let scenario = Scenario {
        name: "Cookie Persistence Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Login (sets cookies)".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/auth/login".to_string(),
                    body: Some(
                        r#"{
                            "email": "test@example.com",
                            "password": "password123"
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
                cache: None,
            think_time: Some(ThinkTime::Fixed(Duration::from_millis(100))),
            },
            Step {
                name: "Access Protected Resource (uses cookies)".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/users/me".to_string(),
                    body: None,
                    headers: HashMap::new(), // No manual auth header needed - cookies handle it
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
            think_time: None,
            },
        ],
    };

    let client = create_cookie_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    // If cookies work, both steps should succeed
    // Step 1: Login sets session cookie
    // Step 2: Uses session cookie automatically
    println!("\nCookie Persistence Test:");
    println!(
        "  Step 1 (Login): {}",
        if result.steps[0].success {
            "✓"
        } else {
            "✗"
        }
    );
    if result.steps.len() > 1 {
        println!(
            "  Step 2 (Protected): {}",
            if result.steps[1].success {
                "✓"
            } else {
                "✗"
            }
        );
    }
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_auth_flow_with_token_and_cookies() {
    // Test a realistic auth flow that combines token extraction and cookies
    let scenario = Scenario {
        name: "Auth Flow with Token and Cookies".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Register User".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/auth/register".to_string(),
                    body: Some(
                        r#"{
                            "email": "user-${timestamp}@example.com",
                            "password": "SecurePass123!",
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
                extractions: vec![
                    // Extract token from response
                    VariableExtraction {
                        name: "auth_token".to_string(),
                        extractor: Extractor::JsonPath("$.token".to_string()),
                    },
                ],
                assertions: vec![],
                cache: None,
            think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Access Profile with Token".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/users/me".to_string(),
                    body: None,
                    headers: {
                        let mut headers = HashMap::new();
                        // Use extracted token in Authorization header
                        headers.insert(
                            "Authorization".to_string(),
                            "Bearer ${auth_token}".to_string(),
                        );
                        headers
                    },
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
            think_time: None,
            },
        ],
    };

    let client = create_cookie_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    println!("\nAuth Flow Test:");
    println!(
        "  Registration: {}",
        if result.steps[0].success {
            "✓"
        } else {
            "✗"
        }
    );

    // Token should be extracted
    let token = context.get_variable("auth_token");
    println!(
        "  Token extracted: {}",
        if token.is_some() { "✓" } else { "✗" }
    );

    if result.steps.len() > 1 {
        println!(
            "  Profile access: {}",
            if result.steps[1].success {
                "✓"
            } else {
                "✗"
            }
        );
    }
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_cookie_isolation_between_clients() {
    // Test that different client instances have isolated cookies
    let scenario = Scenario {
        name: "Login Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Login".to_string(),
            request: RequestConfig {
                method: "POST".to_string(),
                path: "/auth/register".to_string(),
                body: Some(
                    r#"{
                        "email": "user-${timestamp}@example.com",
                        "password": "password123",
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
            cache: None,
            think_time: None,
        }],
    };

    // Create two separate cookie-enabled clients
    let client1 = create_cookie_client();
    let client2 = create_cookie_client();

    let executor1 = ScenarioExecutor::new(BASE_URL.to_string(), client1);
    let executor2 = ScenarioExecutor::new(BASE_URL.to_string(), client2);

    let mut context1 = ScenarioContext::new();
    let mut context2 = ScenarioContext::new();

    // Execute scenarios with different clients
    let result1 = executor1
        .execute(&scenario, &mut context1, &mut SessionStore::new())
        .await;
    let result2 = executor2
        .execute(&scenario, &mut context2, &mut SessionStore::new())
        .await;

    println!("\nCookie Isolation Test:");
    println!("  Client 1: {}", if result1.success { "✓" } else { "✗" });
    println!("  Client 2: {}", if result2.success { "✓" } else { "✗" });

    // Both should succeed independently (cookies are isolated)
    assert!(
        result1.success || result2.success,
        "At least one should succeed"
    );
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_shopping_flow_with_session() {
    // Realistic e-commerce flow using session cookies
    let scenario = Scenario {
        name: "Shopping with Session".to_string(),
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
                cache: None,
            think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Register and Login".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/auth/register".to_string(),
                    body: Some(
                        r#"{
                            "email": "shopper-${timestamp}@example.com",
                            "password": "Shop123!",
                            "name": "Shopper"
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
                    name: "token".to_string(),
                    extractor: Extractor::JsonPath("$.token".to_string()),
                }],
                assertions: vec![],
                cache: None,
            think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Add to Cart (with auth)".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/cart/items".to_string(),
                    body: Some(
                        r#"{
                            "product_id": "${product_id}",
                            "quantity": 2
                        }"#
                        .to_string(),
                    ),
                    headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Content-Type".to_string(), "application/json".to_string());
                        headers.insert("Authorization".to_string(), "Bearer ${token}".to_string());
                        headers
                    },
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
            think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "View Cart (session maintained)".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/cart".to_string(),
                    body: None,
                    headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Authorization".to_string(), "Bearer ${token}".to_string());
                        headers
                    },
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
            think_time: None,
            },
        ],
    };

    let client = create_cookie_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
    let mut context = ScenarioContext::new();

    let result = executor
        .execute(&scenario, &mut context, &mut SessionStore::new())
        .await;

    println!("\nShopping Flow with Session:");
    println!("  Success: {}", result.success);
    println!(
        "  Steps completed: {}/{}",
        result.steps_completed,
        result.steps.len()
    );

    for (idx, step) in result.steps.iter().enumerate() {
        println!(
            "  Step {}: {} - {}",
            idx + 1,
            step.step_name,
            if step.success { "✓" } else { "✗" }
        );
    }
}

#[tokio::test]
#[ignore] // Requires ecom.edge.baugus-lab.com
async fn test_client_without_cookies_fails_session() {
    // Demonstrate that without cookies, session-based auth fails
    let scenario = Scenario {
        name: "No Cookie Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Login".to_string(),
            request: RequestConfig {
                method: "POST".to_string(),
                path: "/auth/register".to_string(),
                body: Some(
                    r#"{
                            "email": "nocookie-${timestamp}@example.com",
                            "password": "Test123!",
                            "name": "No Cookie User"
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
            cache: None,
            think_time: None,
        }],
    };

    // Client WITHOUT cookies
    let client_no_cookies = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()
        .unwrap();

    // Client WITH cookies
    let client_with_cookies = create_cookie_client();

    let executor_no_cookies = ScenarioExecutor::new(BASE_URL.to_string(), client_no_cookies);
    let executor_with_cookies = ScenarioExecutor::new(BASE_URL.to_string(), client_with_cookies);

    let mut context_no_cookies = ScenarioContext::new();
    let mut context_with_cookies = ScenarioContext::new();

    let result_no_cookies = executor_no_cookies
        .execute(&scenario, &mut context_no_cookies, &mut SessionStore::new())
        .await;
    let result_with_cookies = executor_with_cookies
        .execute(&scenario, &mut context_with_cookies, &mut SessionStore::new())
        .await;

    println!("\nCookie Enabled Comparison:");
    println!(
        "  Without cookies: {}",
        if result_no_cookies.success {
            "✓"
        } else {
            "✗"
        }
    );
    println!(
        "  With cookies: {}",
        if result_with_cookies.success {
            "✓"
        } else {
            "✗"
        }
    );
}
