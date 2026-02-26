//! Integration tests for think times and configurable delays (#29).
//!
//! These tests validate that think times:
//! - Add delays between steps
//! - Support both fixed and random delays
//! - Do NOT count towards request latency metrics

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{RequestConfig, Scenario, ScenarioContext, Step, ThinkTime};
use std::collections::HashMap;
use std::time::{Duration, Instant};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

/// Mount GET /get and GET /json on the mock server.
async fn mount_basic_routes(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/get"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/json"))
        .respond_with(ResponseTemplate::new(200))
        .mount(server)
        .await;
}

#[tokio::test]
async fn test_fixed_think_time() {
    let server = MockServer::start().await;
    mount_basic_routes(&server).await;

    let scenario = Scenario {
        name: "Fixed Think Time Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Step 1".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(500))),
            },
            Step {
                name: "Step 2".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
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
    let executor = ScenarioExecutor::new(server.uri(), client);
    let mut context = ScenarioContext::new();

    let start = Instant::now();
    let result = executor.execute(&scenario, &mut context).await;
    let total_duration = start.elapsed();

    assert!(result.success, "Scenario should succeed");
    assert_eq!(result.steps_completed, 2);

    let step1_ms = result.steps[0].response_time_ms;
    let step2_ms = result.steps[1].response_time_ms;
    let total_ms = total_duration.as_millis() as u64;

    // Total time must include the 500ms think time on top of both requests.
    assert!(
        total_ms >= 500,
        "Total duration {}ms should be at least 500ms (think time)",
        total_ms
    );

    // The think time should appear as overhead ON TOP of the two step latencies.
    // With wiremock responses are ~instant, so think time is almost all of total.
    // Using 90% of think time (450ms) as the threshold to absorb scheduling jitter.
    let think_overhead_ms = total_ms.saturating_sub(step1_ms + step2_ms);
    assert!(
        think_overhead_ms >= 450,
        "Think time overhead should be ~500ms, got {}ms \
         (total={}ms, step1={}ms, step2={}ms — think time not applied separately?)",
        think_overhead_ms,
        total_ms,
        step1_ms,
        step2_ms
    );

    println!("\nFixed Think Time Test:");
    println!("  Total duration: {}ms", total_ms);
    println!("  Step 1 latency: {}ms (excludes think time)", step1_ms);
    println!("  Step 2 latency: {}ms", step2_ms);
    println!("  Think time overhead: {}ms", think_overhead_ms);
    println!("  ✅ Think time does NOT count towards request latency");
}

#[tokio::test]
async fn test_random_think_time() {
    let server = MockServer::start().await;
    mount_basic_routes(&server).await;

    let scenario = Scenario {
        name: "Random Think Time Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Request with Random Delay".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Random {
                    min: Duration::from_millis(200),
                    max: Duration::from_millis(800),
                }),
            },
            Step {
                name: "Next Step".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
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
    let executor = ScenarioExecutor::new(server.uri(), client);

    // Run multiple times to test randomness
    let mut durations = Vec::new();

    for _ in 0..5 {
        let mut context = ScenarioContext::new();
        let start = Instant::now();
        let result = executor.execute(&scenario, &mut context).await;
        let total_duration = start.elapsed();

        assert!(result.success);
        durations.push(total_duration.as_millis());

        // Should take at least 200ms (min think time)
        assert!(
            total_duration.as_millis() >= 200,
            "Duration {}ms should be at least 200ms",
            total_duration.as_millis()
        );
    }

    println!("\nRandom Think Time Test (200-800ms):");
    println!("  Run 1: {}ms", durations[0]);
    println!("  Run 2: {}ms", durations[1]);
    println!("  Run 3: {}ms", durations[2]);
    println!("  Run 4: {}ms", durations[3]);
    println!("  Run 5: {}ms", durations[4]);

    // Check that durations vary (randomness working)
    let all_same = durations.windows(2).all(|w| w[0] == w[1]);
    assert!(!all_same, "Durations should vary due to random think time");

    println!("  ✅ Think times are random and vary between runs");
}

#[tokio::test]
async fn test_multiple_think_times() {
    let server = MockServer::start().await;
    mount_basic_routes(&server).await;

    let scenario = Scenario {
        name: "Multiple Think Times".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Step 1".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(100))),
            },
            Step {
                name: "Step 2".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(200))),
            },
            Step {
                name: "Step 3".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Fixed(Duration::from_millis(300))),
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(server.uri(), client);
    let mut context = ScenarioContext::new();

    let start = Instant::now();
    let result = executor.execute(&scenario, &mut context).await;
    let total_duration = start.elapsed();

    assert!(result.success);
    assert_eq!(result.steps_completed, 3);

    // Should take at least 600ms (100 + 200 + 300)
    assert!(
        total_duration.as_millis() >= 600,
        "Total duration {}ms should be at least 600ms (cumulative think time)",
        total_duration.as_millis()
    );

    println!("\nMultiple Think Times Test:");
    println!(
        "  Total duration: {}ms (includes 600ms think time)",
        total_duration.as_millis()
    );
    println!(
        "  Step 1: {}ms + 100ms think",
        result.steps[0].response_time_ms
    );
    println!(
        "  Step 2: {}ms + 200ms think",
        result.steps[1].response_time_ms
    );
    println!(
        "  Step 3: {}ms + 300ms think",
        result.steps[2].response_time_ms
    );
    println!("  ✅ Multiple think times accumulate correctly");
}

#[tokio::test]
async fn test_no_think_time() {
    let server = MockServer::start().await;
    mount_basic_routes(&server).await;

    let scenario = Scenario {
        name: "No Think Time".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Fast Step 1".to_string(),
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
                name: "Fast Step 2".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
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
    let executor = ScenarioExecutor::new(server.uri(), client);
    let mut context = ScenarioContext::new();

    let start = Instant::now();
    let result = executor.execute(&scenario, &mut context).await;
    let total_duration = start.elapsed();

    assert!(result.success);

    // With wiremock responses the round-trips are local — well under 1 second.
    assert!(
        total_duration.as_millis() < 1000,
        "Without think time, should complete quickly ({}ms)",
        total_duration.as_millis()
    );

    println!("\nNo Think Time Test:");
    println!("  Total duration: {}ms", total_duration.as_millis());
    println!("  ✅ No delays when think_time is None");
}

#[tokio::test]
async fn test_realistic_user_behavior() {
    let server = MockServer::start().await;
    mount_basic_routes(&server).await;

    // Simulate realistic e-commerce browsing with varied think times
    let scenario = Scenario {
        name: "Realistic User Behavior".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Land on homepage".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Random {
                    min: Duration::from_secs(1),
                    max: Duration::from_secs(3),
                }), // Read homepage content
            },
            Step {
                name: "Browse products".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Random {
                    min: Duration::from_secs(2),
                    max: Duration::from_secs(5),
                }), // Browse product list
            },
            Step {
                name: "View product details".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: Some(ThinkTime::Random {
                    min: Duration::from_secs(3),
                    max: Duration::from_secs(10),
                }), // Read product description, reviews
            },
        ],
    };

    let client = create_test_client();
    let executor = ScenarioExecutor::new(server.uri(), client);
    let mut context = ScenarioContext::new();

    let start = Instant::now();
    let result = executor.execute(&scenario, &mut context).await;
    let total_duration = start.elapsed();

    assert!(result.success);

    // Should take at least 6 seconds (1+2+3 minimum think times)
    assert!(
        total_duration.as_secs() >= 6,
        "Realistic flow should take at least 6s, took {}s",
        total_duration.as_secs()
    );

    println!("\nRealistic User Behavior Test:");
    println!("  Total duration: {:.1}s", total_duration.as_secs_f64());
    println!(
        "  Step 1 (homepage): {}ms + 1-3s think",
        result.steps[0].response_time_ms
    );
    println!(
        "  Step 2 (browse): {}ms + 2-5s think",
        result.steps[1].response_time_ms
    );
    println!(
        "  Step 3 (details): {}ms + 3-10s think",
        result.steps[2].response_time_ms
    );
    println!("  ✅ Realistic user delays applied");
}
