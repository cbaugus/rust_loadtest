//! Integration tests for connection pool statistics (Issue #36).
//!
//! These tests validate connection pool configuration and statistics tracking.

use rust_loadtest::connection_pool::{ConnectionStats, PoolConfig, PoolStatsTracker, GLOBAL_POOL_STATS};
use std::time::Duration;

#[test]
fn test_pool_config_default() {
    let config = PoolConfig::default();

    assert_eq!(config.max_idle_per_host, 32);
    assert_eq!(config.idle_timeout, Duration::from_secs(90));
    assert_eq!(config.tcp_keepalive, Some(Duration::from_secs(60)));

    println!("✅ Pool configuration defaults work");
}

#[test]
fn test_pool_config_builder_pattern() {
    let config = PoolConfig::new()
        .with_max_idle_per_host(64)
        .with_idle_timeout(Duration::from_secs(120))
        .with_tcp_keepalive(Some(Duration::from_secs(30)));

    assert_eq!(config.max_idle_per_host, 64);
    assert_eq!(config.idle_timeout, Duration::from_secs(120));
    assert_eq!(config.tcp_keepalive, Some(Duration::from_secs(30)));

    println!("✅ Pool configuration builder pattern works");
}

#[test]
fn test_pool_config_disable_keepalive() {
    let config = PoolConfig::new()
        .with_tcp_keepalive(None);

    assert_eq!(config.tcp_keepalive, None);

    println!("✅ TCP keepalive can be disabled");
}

#[test]
fn test_connection_stats_empty() {
    let stats = ConnectionStats::default();

    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.likely_new_connections, 0);
    assert_eq!(stats.likely_reused_connections, 0);
    assert_eq!(stats.reuse_rate(), 0.0);
    assert_eq!(stats.new_connection_rate(), 0.0);
    assert!(stats.duration().is_none());

    println!("✅ Empty connection stats handled correctly");
}

#[test]
fn test_connection_stats_calculations() {
    let stats = ConnectionStats {
        total_requests: 100,
        likely_new_connections: 20,
        likely_reused_connections: 80,
        first_request: Some(std::time::Instant::now()),
        last_request: Some(std::time::Instant::now()),
    };

    assert_eq!(stats.reuse_rate(), 80.0);
    assert_eq!(stats.new_connection_rate(), 20.0);

    let formatted = stats.format();
    assert!(formatted.contains("Total: 100"));
    assert!(formatted.contains("Reused: 80"));
    assert!(formatted.contains("80.0%"));
    assert!(formatted.contains("New: 20"));
    assert!(formatted.contains("20.0%"));

    println!("✅ Connection stats calculations work");
    println!("   {}", formatted);
}

#[test]
fn test_pool_stats_tracker_fast_requests() {
    let tracker = PoolStatsTracker::new(100);

    // Simulate 10 fast requests (reused connections)
    for _ in 0..10 {
        tracker.record_request(30); // 30ms - very fast
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.likely_reused_connections, 10);
    assert_eq!(stats.likely_new_connections, 0);
    assert_eq!(stats.reuse_rate(), 100.0);

    println!("✅ Fast requests classified as reused connections");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_tracker_slow_requests() {
    let tracker = PoolStatsTracker::new(100);

    // Simulate 10 slow requests (new connections with TLS handshake)
    for _ in 0..10 {
        tracker.record_request(150); // 150ms - includes TLS handshake
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.likely_reused_connections, 0);
    assert_eq!(stats.likely_new_connections, 10);
    assert_eq!(stats.new_connection_rate(), 100.0);

    println!("✅ Slow requests classified as new connections");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_tracker_mixed_patterns() {
    let tracker = PoolStatsTracker::new(100);

    // Simulate realistic mixed pattern
    tracker.record_request(150); // New connection (slow)
    tracker.record_request(25);  // Reused (fast)
    tracker.record_request(30);  // Reused (fast)
    tracker.record_request(120); // New connection (slow)
    tracker.record_request(20);  // Reused (fast)
    tracker.record_request(35);  // Reused (fast)
    tracker.record_request(110); // New connection (slow)
    tracker.record_request(28);  // Reused (fast)

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 8);
    assert_eq!(stats.likely_reused_connections, 5);
    assert_eq!(stats.likely_new_connections, 3);
    assert_eq!(stats.reuse_rate(), 62.5);
    assert_eq!(stats.new_connection_rate(), 37.5);

    println!("✅ Mixed request patterns tracked correctly");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_tracker_custom_threshold() {
    let tracker = PoolStatsTracker::new(200); // Higher threshold

    tracker.record_request(150); // Under threshold - reused
    tracker.record_request(180); // Under threshold - reused
    tracker.record_request(210); // Over threshold - new
    tracker.record_request(250); // Over threshold - new

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 4);
    assert_eq!(stats.likely_reused_connections, 2);
    assert_eq!(stats.likely_new_connections, 2);

    println!("✅ Custom threshold works correctly");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_tracker_reset() {
    let tracker = PoolStatsTracker::new(100);

    // Record some requests
    tracker.record_request(50);
    tracker.record_request(150);
    tracker.record_request(30);

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 3);

    // Reset
    tracker.reset();

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.likely_reused_connections, 0);
    assert_eq!(stats.likely_new_connections, 0);

    println!("✅ Pool stats tracker reset works");
}

#[test]
fn test_pool_stats_timing_accuracy() {
    let tracker = PoolStatsTracker::new(100);

    tracker.record_request(50);

    // Wait a known duration
    std::thread::sleep(Duration::from_millis(100));

    tracker.record_request(50);

    let stats = tracker.stats();
    let duration = stats.duration().unwrap();

    // Duration should be at least 100ms but less than 200ms
    assert!(duration >= Duration::from_millis(100));
    assert!(duration < Duration::from_millis(200));

    println!("✅ Timing accuracy validated");
    println!("   Duration: {:?}", duration);
}

#[test]
fn test_connection_stats_duration_calculation() {
    use std::time::Instant;

    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    let end = Instant::now();

    let stats = ConnectionStats {
        total_requests: 10,
        likely_new_connections: 2,
        likely_reused_connections: 8,
        first_request: Some(start),
        last_request: Some(end),
    };

    let duration = stats.duration().unwrap();
    assert!(duration >= Duration::from_millis(50));
    assert!(duration < Duration::from_millis(100));

    println!("✅ Duration calculation works");
    println!("   Duration: {:.3}s", duration.as_secs_f64());
}

#[test]
fn test_pool_stats_high_reuse_scenario() {
    let tracker = PoolStatsTracker::new(100);

    // Simulate high connection reuse (ideal scenario)
    // First request is slow (new connection)
    tracker.record_request(150);

    // Following 99 requests are fast (reused)
    for _ in 0..99 {
        tracker.record_request(30);
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.likely_reused_connections, 99);
    assert_eq!(stats.likely_new_connections, 1);
    assert_eq!(stats.reuse_rate(), 99.0);

    println!("✅ High reuse scenario validated");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let tracker = Arc::new(PoolStatsTracker::new(100));
    let mut handles = vec![];

    // Spawn 5 threads, each recording 20 requests
    for thread_id in 0..5 {
        let tracker_clone = Arc::clone(&tracker);
        let handle = thread::spawn(move || {
            for i in 0..20 {
                // Alternate between fast and slow requests
                if (thread_id + i) % 3 == 0 {
                    tracker_clone.record_request(150); // Slow (new)
                } else {
                    tracker_clone.record_request(30); // Fast (reused)
                }
            }
        });
        handles.push(handle);
    }

    // Wait for all threads
    for handle in handles {
        handle.join().unwrap();
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 100); // 5 threads * 20 requests

    println!("✅ Concurrent access handled correctly");
    println!("   {}", stats.format());
}

#[test]
fn test_pool_stats_boundary_values() {
    let tracker = PoolStatsTracker::new(100);

    // Test exact threshold
    tracker.record_request(99);  // Just below threshold - reused
    tracker.record_request(100); // Exactly at threshold - new
    tracker.record_request(101); // Just above threshold - new

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.likely_reused_connections, 1);
    assert_eq!(stats.likely_new_connections, 2);

    println!("✅ Boundary values handled correctly");
}

#[test]
fn test_pool_stats_zero_latency() {
    let tracker = PoolStatsTracker::new(100);

    // Edge case: zero latency (shouldn't happen in practice)
    tracker.record_request(0);

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.likely_reused_connections, 1); // Zero is below threshold

    println!("✅ Zero latency handled correctly");
}

#[test]
fn test_pool_stats_extreme_latency() {
    let tracker = PoolStatsTracker::new(100);

    // Edge case: very high latency (network issues)
    tracker.record_request(5000); // 5 seconds - definitely new connection or error

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 1);
    assert_eq!(stats.likely_new_connections, 1);

    println!("✅ Extreme latency handled correctly");
}

#[test]
fn test_global_pool_stats_singleton() {
    // Note: GLOBAL_POOL_STATS is shared across tests, so we just verify it exists
    // and can be called without testing specific values

    let stats = GLOBAL_POOL_STATS.stats();

    // Should be able to get stats (may have data from other tests)
    assert!(stats.total_requests >= 0);

    println!("✅ Global pool stats singleton accessible");
}

#[test]
fn test_pool_config_apply_to_builder() {
    let config = PoolConfig::new()
        .with_max_idle_per_host(64)
        .with_idle_timeout(Duration::from_secs(120))
        .with_tcp_keepalive(Some(Duration::from_secs(30)));

    // Create a reqwest client builder
    let builder = reqwest::Client::builder();

    // Apply pool config (this should not panic)
    let _builder = config.apply_to_builder(builder);

    println!("✅ Pool config can be applied to reqwest ClientBuilder");
}

#[tokio::test]
async fn test_pool_with_real_client() {
    let config = PoolConfig::new()
        .with_max_idle_per_host(10)
        .with_idle_timeout(Duration::from_secs(30));

    let builder = reqwest::Client::builder();
    let builder = config.apply_to_builder(builder);

    let client = builder.build().expect("Failed to build client");

    // Just verify we can create a client with pool config
    // We won't make actual requests in unit tests
    assert!(client.get("http://example.com").build().is_ok());

    println!("✅ Real HTTP client with pool config works");
}

#[test]
fn test_connection_stats_format_variations() {
    // Test different percentage scenarios
    let test_cases = vec![
        (100, 0, 100), // 100% reuse
        (100, 100, 0), // 0% reuse (all new)
        (100, 50, 50), // 50/50
        (100, 75, 25), // 75% reuse
    ];

    for (total, new, reused) in test_cases {
        let stats = ConnectionStats {
            total_requests: total,
            likely_new_connections: new,
            likely_reused_connections: reused,
            first_request: Some(std::time::Instant::now()),
            last_request: Some(std::time::Instant::now()),
        };

        let formatted = stats.format();
        assert!(formatted.contains(&format!("Total: {}", total)));
        assert!(formatted.contains(&format!("New: {}", new)));
        assert!(formatted.contains(&format!("Reused: {}", reused)));
    }

    println!("✅ Connection stats formatting works for all scenarios");
}
