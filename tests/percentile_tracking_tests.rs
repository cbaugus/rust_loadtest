//! Integration tests for percentile latency tracking (Issue #33).
//!
//! These tests validate that percentile calculations are accurate and that
//! latencies are properly tracked across requests, scenarios, and steps.

use rust_loadtest::executor::{ScenarioExecutor, SessionStore};
use rust_loadtest::percentiles::{
    MultiLabelPercentileTracker, PercentileTracker, GLOBAL_SCENARIO_PERCENTILES,
    GLOBAL_STEP_PERCENTILES,
};
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

#[test]
fn test_percentile_tracker_basic() {
    let tracker = PercentileTracker::new();

    // Record latencies: 10ms, 20ms, 30ms, 40ms, 50ms, 60ms, 70ms, 80ms, 90ms, 100ms
    for i in 1..=10 {
        tracker.record_ms(i * 10);
    }

    let stats = tracker.stats().expect("Should have stats");

    assert_eq!(stats.count, 10);
    // HdrHistogram has internal precision rounding, so use approximate checks
    assert!(
        stats.min >= 9_900 && stats.min <= 10_100,
        "min {}μs should be around 10000μs",
        stats.min
    );
    assert!(
        stats.max >= 99_900 && stats.max <= 100_500,
        "max {}μs should be around 100000μs",
        stats.max
    );

    // P50 should be around 50ms
    assert!(
        stats.p50 >= 45_000 && stats.p50 <= 55_000,
        "P50 {}μs should be around 50000μs",
        stats.p50
    );

    // P90 should be around 90ms
    assert!(
        stats.p90 >= 85_000 && stats.p90 <= 95_000,
        "P90 {}μs should be around 90000μs",
        stats.p90
    );

    // P99 should be around 100ms (or close to max)
    assert!(
        stats.p99 >= 95_000 && stats.p99 <= 105_000,
        "P99 {}μs should be around 100000μs",
        stats.p99
    );

    println!("✅ Basic percentile tracking works correctly");
    println!("   {}", stats.format());
}

#[test]
fn test_percentile_tracker_large_dataset() {
    let tracker = PercentileTracker::new();

    // Record 1000 samples from 1ms to 1000ms
    for i in 1..=1000 {
        tracker.record_ms(i);
    }

    let stats = tracker.stats().expect("Should have stats");

    assert_eq!(stats.count, 1000);

    // For uniform distribution:
    // P50 should be around 500ms
    assert!(
        stats.p50 >= 480_000 && stats.p50 <= 520_000,
        "P50 {}μs should be around 500000μs",
        stats.p50
    );

    // P90 should be around 900ms
    assert!(
        stats.p90 >= 880_000 && stats.p90 <= 920_000,
        "P90 {}μs should be around 900000μs",
        stats.p90
    );

    // P95 should be around 950ms
    assert!(
        stats.p95 >= 930_000 && stats.p95 <= 970_000,
        "P95 {}μs should be around 950000μs",
        stats.p95
    );

    // P99 should be around 990ms
    assert!(
        stats.p99 >= 970_000 && stats.p99 <= 1_010_000,
        "P99 {}μs should be around 990000μs",
        stats.p99
    );

    println!("✅ Large dataset percentile tracking accurate");
    println!("   {}", stats.format());
}

#[test]
fn test_percentile_tracker_skewed_distribution() {
    let tracker = PercentileTracker::new();

    // Record 90 fast requests (10ms) and 10 slow requests (1000ms)
    for _ in 0..90 {
        tracker.record_ms(10);
    }
    for _ in 0..10 {
        tracker.record_ms(1000);
    }

    let stats = tracker.stats().expect("Should have stats");

    assert_eq!(stats.count, 100);

    // P50 should be 10ms (median is in the fast group)
    assert!(
        stats.p50 <= 15_000,
        "P50 {}μs should be around 10000μs",
        stats.p50
    );

    // P90 should still be 10ms (90th percentile is last fast request)
    assert!(
        stats.p90 <= 15_000,
        "P90 {}μs should be around 10000μs",
        stats.p90
    );

    // P95 should be 1000ms (now in the slow group)
    assert!(
        stats.p95 >= 900_000,
        "P95 {}μs should be around 1000000μs",
        stats.p95
    );

    // P99 should be 1000ms
    assert!(
        stats.p99 >= 900_000,
        "P99 {}μs should be around 1000000μs",
        stats.p99
    );

    println!("✅ Skewed distribution percentiles correct");
    println!("   {}", stats.format());
    println!(
        "   Shows P90 at {}ms and P95 at {}ms",
        stats.p90 as f64 / 1000.0,
        stats.p95 as f64 / 1000.0
    );
}

#[test]
fn test_multi_label_tracker() {
    let tracker = MultiLabelPercentileTracker::new();

    // Record different latencies for different endpoints
    tracker.record("/api/fast", 10);
    tracker.record("/api/fast", 20);
    tracker.record("/api/fast", 15);

    tracker.record("/api/slow", 100);
    tracker.record("/api/slow", 200);
    tracker.record("/api/slow", 150);

    let fast_stats = tracker.stats("/api/fast").expect("Should have fast stats");
    let slow_stats = tracker.stats("/api/slow").expect("Should have slow stats");

    assert_eq!(fast_stats.count, 3);
    assert_eq!(slow_stats.count, 3);

    // Fast endpoint should have low latencies
    assert!(fast_stats.max < 30_000, "Fast max should be under 30ms");

    // Slow endpoint should have high latencies
    assert!(slow_stats.min > 90_000, "Slow min should be over 90ms");

    println!("✅ Multi-label tracking separates endpoints correctly");
    println!("   Fast endpoint: {}", fast_stats.format());
    println!("   Slow endpoint: {}", slow_stats.format());
}

#[test]
fn test_multi_label_all_stats() {
    let tracker = MultiLabelPercentileTracker::new();

    tracker.record("endpoint1", 10);
    tracker.record("endpoint2", 20);
    tracker.record("endpoint3", 30);

    let all_stats = tracker.all_stats();

    assert_eq!(all_stats.len(), 3);
    assert!(all_stats.contains_key("endpoint1"));
    assert!(all_stats.contains_key("endpoint2"));
    assert!(all_stats.contains_key("endpoint3"));

    println!("✅ all_stats() returns all tracked labels");
}

#[tokio::test]
async fn test_scenario_percentile_tracking() {
    let scenario = Scenario {
        name: "Percentile Test Scenario".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
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
            },
            Step {
                name: "Status Check".to_string(),
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

    let client = create_test_client();
    let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

    // Execute scenario multiple times
    for _ in 0..5 {
        let mut context = ScenarioContext::new();
        let result = executor
            .execute(&scenario, &mut context, &mut SessionStore::new())
            .await;

        assert!(result.success);

        // Manually record for testing (in production, worker.rs does this)
        GLOBAL_SCENARIO_PERCENTILES.record(&scenario.name, result.total_time_ms);

        for step in &result.steps {
            let label = format!("{}:{}", scenario.name, step.step_name);
            GLOBAL_STEP_PERCENTILES.record(&label, step.response_time_ms);
        }
    }

    // Verify we have stats
    let scenario_stats = GLOBAL_SCENARIO_PERCENTILES
        .stats(&scenario.name)
        .expect("Should have scenario stats");

    assert_eq!(scenario_stats.count, 5, "Should have 5 scenario executions");

    let health_label = format!("{}:Health Check", scenario.name);
    let health_stats = GLOBAL_STEP_PERCENTILES
        .stats(&health_label)
        .expect("Should have health step stats");

    assert_eq!(health_stats.count, 5, "Should have 5 health check steps");

    println!("✅ Scenario percentile tracking works");
    println!("   Scenario: {}", scenario_stats.format());
    println!("   Health step: {}", health_stats.format());
}

#[test]
fn test_percentile_tracker_reset() {
    let tracker = PercentileTracker::new();

    tracker.record_ms(100);
    tracker.record_ms(200);
    assert!(tracker.stats().is_some());

    tracker.reset();
    assert!(
        tracker.stats().is_none(),
        "Stats should be None after reset"
    );

    println!("✅ Tracker reset works correctly");
}

#[test]
fn test_percentile_stats_format() {
    let tracker = PercentileTracker::new();

    // Record some values
    for i in 1..=100 {
        tracker.record_ms(i);
    }

    let stats = tracker.stats().expect("Should have stats");
    let formatted = stats.format();

    // Should contain all the key metrics
    assert!(formatted.contains("count="));
    assert!(formatted.contains("min="));
    assert!(formatted.contains("max="));
    assert!(formatted.contains("mean="));
    assert!(formatted.contains("p50="));
    assert!(formatted.contains("p90="));
    assert!(formatted.contains("p95="));
    assert!(formatted.contains("p99="));
    assert!(formatted.contains("p99.9="));

    println!("✅ Stats formatting includes all percentiles");
    println!("   {}", formatted);
}

#[tokio::test]
async fn test_realistic_latency_distribution() {
    // Simulate realistic API latencies: mostly fast with occasional slow requests
    let tracker = PercentileTracker::new();

    // 80% of requests are fast (10-50ms)
    for _ in 0..80 {
        let latency = 10 + (rand::random::<u64>() % 40);
        tracker.record_ms(latency);
    }

    // 15% are medium (50-200ms)
    for _ in 0..15 {
        let latency = 50 + (rand::random::<u64>() % 150);
        tracker.record_ms(latency);
    }

    // 5% are slow (200-1000ms)
    for _ in 0..5 {
        let latency = 200 + (rand::random::<u64>() % 800);
        tracker.record_ms(latency);
    }

    let stats = tracker.stats().expect("Should have stats");

    assert_eq!(stats.count, 100);

    // P50 should be in the fast range
    assert!(
        stats.p50 < 100_000,
        "P50 {}μs should be under 100ms",
        stats.p50
    );

    // P90 should be in the medium range or below
    assert!(
        stats.p90 < 300_000,
        "P90 {}μs should be under 300ms",
        stats.p90
    );

    // P99 should catch the slow requests
    assert!(
        stats.p99 >= 200_000,
        "P99 {}μs should be at least 200ms",
        stats.p99
    );

    println!("✅ Realistic latency distribution captured correctly");
    println!("   {}", stats.format());
    println!(
        "   P50 at {:.2}ms, P90 at {:.2}ms, P99 at {:.2}ms",
        stats.p50 as f64 / 1000.0,
        stats.p90 as f64 / 1000.0,
        stats.p99 as f64 / 1000.0
    );
}
