//! Integration tests for per-scenario throughput tracking (Issue #35).
//!
//! These tests validate that throughput (requests per second) is tracked
//! separately for each scenario type, enabling performance comparison.

use rust_loadtest::executor::ScenarioExecutor;
use rust_loadtest::scenario::{RequestConfig, Scenario, ScenarioContext, Step};
use rust_loadtest::throughput::{format_throughput_table, ThroughputTracker};
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
fn test_throughput_tracker_basic() {
    let tracker = ThroughputTracker::new();

    tracker.record("scenario1", Duration::from_millis(100));
    tracker.record("scenario1", Duration::from_millis(150));
    tracker.record("scenario2", Duration::from_millis(200));

    let stats1 = tracker.stats("scenario1").unwrap();
    assert_eq!(stats1.total_count, 2);
    assert_eq!(stats1.avg_time_ms, 125.0);

    let stats2 = tracker.stats("scenario2").unwrap();
    assert_eq!(stats2.total_count, 1);
    assert_eq!(stats2.avg_time_ms, 200.0);

    println!("✅ Throughput tracker basic functionality works");
}

#[test]
fn test_throughput_tracker_rps_calculation() {
    let tracker = ThroughputTracker::new();

    // Record 10 requests
    for _ in 0..10 {
        tracker.record("test", Duration::from_millis(50));
    }

    // Wait a bit to ensure time has passed
    std::thread::sleep(Duration::from_millis(100));

    let stats = tracker.stats("test").unwrap();
    assert_eq!(stats.total_count, 10);
    assert!(stats.rps > 0.0, "RPS should be greater than 0");
    assert!(stats.duration.as_millis() >= 100);

    println!("✅ RPS calculation works (RPS: {:.2})", stats.rps);
}

#[test]
fn test_throughput_tracker_multiple_scenarios() {
    let tracker = ThroughputTracker::new();

    tracker.record("fast", Duration::from_millis(10));
    tracker.record("fast", Duration::from_millis(20));
    tracker.record("medium", Duration::from_millis(100));
    tracker.record("slow", Duration::from_millis(500));

    let all_stats = tracker.all_stats();
    assert_eq!(all_stats.len(), 3);

    // Should be sorted by name
    assert_eq!(all_stats[0].scenario_name, "fast");
    assert_eq!(all_stats[1].scenario_name, "medium");
    assert_eq!(all_stats[2].scenario_name, "slow");

    println!("✅ Multiple scenarios tracked correctly");
}

#[test]
fn test_throughput_stats_formatting() {
    let tracker = ThroughputTracker::new();

    tracker.record("TestScenario", Duration::from_millis(100));

    let stats = tracker.stats("TestScenario").unwrap();
    let formatted = stats.format();

    assert!(formatted.contains("TestScenario"));
    assert!(formatted.contains("requests"));
    assert!(formatted.contains("RPS"));

    println!("✅ Throughput stats formatting works");
    println!("   {}", formatted);
}

#[test]
fn test_throughput_table_formatting() {
    let tracker = ThroughputTracker::new();

    tracker.record("Scenario A", Duration::from_millis(50));
    tracker.record("Scenario B", Duration::from_millis(100));
    tracker.record("Scenario C", Duration::from_millis(150));

    let all_stats = tracker.all_stats();
    let table = format_throughput_table(&all_stats);

    assert!(table.contains("Scenario"));
    assert!(table.contains("Requests"));
    assert!(table.contains("RPS"));
    assert!(table.contains("Scenario A"));
    assert!(table.contains("Scenario B"));

    println!("✅ Throughput table formatting works");
    println!("{}", table);
}

#[test]
fn test_total_throughput() {
    let tracker = ThroughputTracker::new();

    // Record requests across multiple scenarios
    for _ in 0..5 {
        tracker.record("scenario1", Duration::from_millis(50));
    }
    for _ in 0..3 {
        tracker.record("scenario2", Duration::from_millis(75));
    }

    std::thread::sleep(Duration::from_millis(50));

    let total_rps = tracker.total_throughput();
    assert!(total_rps > 0.0, "Total RPS should be greater than 0");

    println!("✅ Total throughput calculation works (Total RPS: {:.2})", total_rps);
}

#[test]
fn test_throughput_reset() {
    let tracker = ThroughputTracker::new();

    tracker.record("test", Duration::from_millis(100));
    assert!(tracker.stats("test").is_some());

    tracker.reset();
    assert!(tracker.stats("test").is_none());

    println!("✅ Throughput tracker reset works");
}

#[tokio::test]
async fn test_scenario_throughput_tracking() {
    let tracker = ThroughputTracker::new();

    let scenario = Scenario {
        name: "Throughput Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Fast Request".to_string(),
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

    // Execute scenario 5 times
    for _ in 0..5 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
        let mut context = ScenarioContext::new();

        let result = executor.execute(&scenario, &mut context).await;
        assert!(result.success);

        // Record throughput
        tracker.record(
            &scenario.name,
            Duration::from_millis(result.total_time_ms)
        );
    }

    let stats = tracker.stats(&scenario.name).unwrap();
    assert_eq!(stats.total_count, 5);
    assert!(stats.rps > 0.0);

    println!("✅ Scenario throughput tracking works");
    println!("   {}", stats.format());
}

#[tokio::test]
async fn test_multiple_scenarios_different_throughput() {
    let tracker = ThroughputTracker::new();

    let fast_scenario = Scenario {
        name: "Fast Scenario".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Health Check".to_string(),
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

    let slow_scenario = Scenario {
        name: "Slow Scenario".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "First Request".to_string(),
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
                name: "Second Request".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/status".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                think_time: None,
            },
        ],
    };

    // Execute fast scenario 3 times
    for _ in 0..3 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
        let mut context = ScenarioContext::new();

        let result = executor.execute(&fast_scenario, &mut context).await;
        tracker.record(&fast_scenario.name, Duration::from_millis(result.total_time_ms));
    }

    // Execute slow scenario 2 times
    for _ in 0..2 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);
        let mut context = ScenarioContext::new();

        let result = executor.execute(&slow_scenario, &mut context).await;
        tracker.record(&slow_scenario.name, Duration::from_millis(result.total_time_ms));
    }

    let fast_stats = tracker.stats(&fast_scenario.name).unwrap();
    let slow_stats = tracker.stats(&slow_scenario.name).unwrap();

    assert_eq!(fast_stats.total_count, 3);
    assert_eq!(slow_stats.total_count, 2);

    // Fast scenario should have lower average time
    assert!(
        fast_stats.avg_time_ms < slow_stats.avg_time_ms,
        "Fast scenario ({:.2}ms) should be faster than slow scenario ({:.2}ms)",
        fast_stats.avg_time_ms,
        slow_stats.avg_time_ms
    );

    println!("✅ Multiple scenarios tracked with different throughput");
    println!("   Fast: {}", fast_stats.format());
    println!("   Slow: {}", slow_stats.format());
}

#[test]
fn test_throughput_tracker_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let tracker = Arc::new(ThroughputTracker::new());
    let mut handles = vec![];

    // Spawn 5 threads, each recording 10 requests
    for thread_id in 0..5 {
        let tracker_clone = Arc::clone(&tracker);
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                tracker_clone.record(
                    &format!("scenario{}", thread_id % 2),
                    Duration::from_millis(50)
                );
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    // Should have recorded 50 total requests across 2 scenarios
    let all_stats = tracker.all_stats();
    let total_count: u64 = all_stats.iter().map(|s| s.total_count).sum();
    assert_eq!(total_count, 50);

    println!("✅ Concurrent access to throughput tracker works");
}

#[test]
fn test_empty_throughput_tracker() {
    let tracker = ThroughputTracker::new();

    assert!(tracker.stats("nonexistent").is_none());
    assert_eq!(tracker.all_stats().len(), 0);

    let table = format_throughput_table(&tracker.all_stats());
    assert!(table.contains("No throughput data"));

    println!("✅ Empty throughput tracker handled correctly");
}
