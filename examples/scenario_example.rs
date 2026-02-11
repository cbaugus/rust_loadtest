//! Example of using the multi-step scenario execution engine.
//!
//! This example demonstrates how to define and execute a multi-step scenario
//! that simulates a user browsing products, adding items to cart, and checking out.
//!
//! Run with: cargo run --example scenario_example

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{
    Assertion, Extractor, RequestConfig, Scenario, ScenarioContext, Step, VariableExtraction,
};
use std::collections::HashMap;
use std::time::Duration;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing for logs
    tracing_subscriber::fmt::init();

    // Create HTTP client
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(30))
        .build()?;

    // Define a shopping scenario
    let scenario = create_shopping_scenario();

    // Create scenario executor
    let base_url = "https://ecom.edge.baugus-lab.com".to_string();
    let executor = ScenarioExecutor::new(base_url, client);

    // Execute the scenario
    let mut context = ScenarioContext::new();
    let result = executor.execute(&scenario, &mut context).await;

    // Print results
    println!("\n=== Scenario Execution Results ===");
    println!("Scenario: {}", result.scenario_name);
    println!("Success: {}", result.success);
    println!("Total Time: {}ms", result.total_time_ms);
    println!("Steps Completed: {}/{}", result.steps_completed, result.steps.len());

    if let Some(failed_step) = result.failed_at_step {
        println!("Failed at step: {}", failed_step);
    }

    println!("\n=== Step Results ===");
    for (idx, step_result) in result.steps.iter().enumerate() {
        println!(
            "Step {}: {} - {} ({}ms) - Status: {:?}",
            idx + 1,
            step_result.step_name,
            if step_result.success { "✓" } else { "✗" },
            step_result.response_time_ms,
            step_result.status_code
        );
        if let Some(error) = &step_result.error {
            println!("  Error: {}", error);
        }
    }

    Ok(())
}

/// Create a shopping scenario with multiple steps.
fn create_shopping_scenario() -> Scenario {
    Scenario {
        name: "E-commerce Shopping Flow".to_string(),
        weight: 1.0,
        steps: vec![
            // Step 1: Health check
            Step {
                name: "Health Check".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/health".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                think_time: Some(Duration::from_millis(500)),
            },

            // Step 2: Browse products
            Step {
                name: "Browse Products".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products?limit=10".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![
                    // Extract first product ID from response
                    VariableExtraction {
                        name: "product_id".to_string(),
                        extractor: Extractor::JsonPath("$.products[0].id".to_string()),
                    },
                ],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::BodyContains("products".to_string()),
                ],
                think_time: Some(Duration::from_secs(2)),
            },

            // Step 3: View product details (using extracted product_id)
            Step {
                name: "View Product Details".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/products/${product_id}".to_string(), // Variable substitution
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::ResponseTime(Duration::from_millis(500)),
                ],
                think_time: Some(Duration::from_secs(3)),
            },

            // Step 4: Register user
            Step {
                name: "Register User".to_string(),
                request: RequestConfig {
                    method: "POST".to_string(),
                    path: "/auth/register".to_string(),
                    body: Some(
                        r#"{
                            "email": "loadtest-user-${timestamp}@example.com",
                            "password": "TestPass123!",
                            "name": "Load Test User"
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
                    // Extract auth token from response
                    VariableExtraction {
                        name: "auth_token".to_string(),
                        extractor: Extractor::JsonPath("$.token".to_string()),
                    },
                ],
                assertions: vec![Assertion::StatusCode(201)],
                think_time: Some(Duration::from_secs(1)),
            },

            // Step 5: Add item to cart (using auth token)
            Step {
                name: "Add to Cart".to_string(),
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
                        headers.insert("Authorization".to_string(), "Bearer ${auth_token}".to_string());
                        headers
                    },
                },
                extractions: vec![
                    VariableExtraction {
                        name: "cart_id".to_string(),
                        extractor: Extractor::JsonPath("$.cart.id".to_string()),
                    },
                ],
                assertions: vec![Assertion::StatusCode(201)],
                think_time: Some(Duration::from_secs(2)),
            },

            // Step 6: View cart
            Step {
                name: "View Cart".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/cart".to_string(),
                    body: None,
                    headers: {
                        let mut headers = HashMap::new();
                        headers.insert("Authorization".to_string(), "Bearer ${auth_token}".to_string());
                        headers
                    },
                },
                extractions: vec![],
                assertions: vec![
                    Assertion::StatusCode(200),
                    Assertion::BodyContains("items".to_string()),
                ],
                think_time: Some(Duration::from_secs(5)),
            },
        ],
    }
}
