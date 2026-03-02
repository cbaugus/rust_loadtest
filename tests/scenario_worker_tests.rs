//! Unit tests for scenario worker functionality.
//!
//! These tests validate that the scenario worker correctly executes scenarios
//! according to load models and respects timing constraints.

use rust_loadtest::load_models::LoadModel;
use rust_loadtest::scenario::{RequestConfig, Scenario, Step, ThinkTime};
use rust_loadtest::worker::{run_scenario_worker, ScenarioWorkerConfig};
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::Instant;

#[tokio::test]
async fn test_scenario_worker_respects_duration() {
    let scenario = Scenario {
        name: "Test Scenario".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Health Check".to_string(),
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
        }],
    };

    let config = ScenarioWorkerConfig {
        task_id: 1,
        base_url: "https://httpbin.org".to_string(),
        scenario,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Rps { target_rps: 1.0 },
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
        region: "local".to_string(),
    };

    let client = reqwest::Client::new();
    let start_time = Instant::now();

    // Run worker
    let worker_start = Instant::now();
    run_scenario_worker(client, config, start_time).await;
    let worker_duration = worker_start.elapsed();

    // Worker should stop after ~2 seconds
    assert!(
        worker_duration.as_secs() >= 2 && worker_duration.as_secs() <= 3,
        "Worker should run for approximately 2 seconds, ran for {}s",
        worker_duration.as_secs()
    );
}

#[tokio::test]
async fn test_scenario_worker_constant_load() {
    let scenario = Scenario {
        name: "Constant Load Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Quick Request".to_string(),
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
        }],
    };

    // Run at 2 scenarios per second for 3 seconds
    // Should execute approximately 6 scenarios
    let config = ScenarioWorkerConfig {
        task_id: 1,
        base_url: "https://httpbin.org".to_string(),
        scenario,
        test_duration: Duration::from_secs(3),
        load_model: LoadModel::Rps { target_rps: 2.0 },
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
        region: "local".to_string(),
    };

    let client = reqwest::Client::new();
    let start_time = Instant::now();

    run_scenario_worker(client, config, start_time).await;

    // Just verify it completes without panicking
    // Actual scenario count would need metrics tracking to verify
}

#[tokio::test]
async fn test_scenario_worker_with_think_time() {
    let scenario = Scenario {
        name: "Think Time Test".to_string(),
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
                cache: None,
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
                cache: None,
            think_time: None,
            },
        ],
    };

    let config = ScenarioWorkerConfig {
        task_id: 1,
        base_url: "https://httpbin.org".to_string(),
        scenario,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Rps { target_rps: 0.5 }, // 1 scenario every 2 seconds
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
        region: "local".to_string(),
    };

    let client = reqwest::Client::new();
    let start_time = Instant::now();

    let worker_start = Instant::now();
    run_scenario_worker(client, config, start_time).await;
    let worker_duration = worker_start.elapsed();

    // Should take at least 2 seconds (test duration)
    assert!(
        worker_duration.as_secs() >= 2,
        "Worker should run for at least 2 seconds"
    );
}
