//! Integration tests for connection pool statistics (Issue #36, #119).
//!
//! These tests validate connection pool configuration and accurate
//! port-based connection tracking.

use rust_loadtest::connection_pool::{
    ConnectionStats, PoolConfig, PoolStatsTracker, GLOBAL_POOL_STATS,
};
use std::net::SocketAddr;
use std::time::Duration;

#[test]
fn test_pool_config_default() {
    let config = PoolConfig::default();

    assert_eq!(config.max_idle_per_host, 32);
    assert_eq!(config.idle_timeout, Duration::from_secs(30));
    assert_eq!(config.tcp_keepalive, Some(Duration::from_secs(60)));
    assert!(config.tcp_nodelay);
    assert_eq!(config.request_timeout, Duration::from_secs(30));
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
}

#[test]
fn test_pool_config_disable_keepalive() {
    let config = PoolConfig::new().with_tcp_keepalive(None);
    assert_eq!(config.tcp_keepalive, None);
}

#[test]
fn test_connection_stats_empty() {
    let stats = ConnectionStats::default();

    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.new_connections, 0);
    assert_eq!(stats.reused_connections, 0);
    assert_eq!(stats.reuse_rate(), 0.0);
    assert_eq!(stats.new_connection_rate(), 0.0);
    assert!(stats.duration().is_none());
}

#[test]
fn test_connection_stats_calculations() {
    let stats = ConnectionStats {
        total_requests: 100,
        new_connections: 20,
        reused_connections: 80,
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
}

#[test]
fn test_port_tracking_all_new_connections() {
    let tracker = PoolStatsTracker::new();

    // Each request from a different port = new connection
    for port in 50001..50011 {
        let addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        tracker.record_request(Some(addr));
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.new_connections, 10);
    assert_eq!(stats.reused_connections, 0);
    assert_eq!(stats.reuse_rate(), 0.0);
}

#[test]
fn test_port_tracking_all_reused_connections() {
    let tracker = PoolStatsTracker::new();

    // Same port every time = reused connection (after first)
    let addr: SocketAddr = "127.0.0.1:50001".parse().unwrap();
    for _ in 0..10 {
        tracker.record_request(Some(addr));
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 10);
    assert_eq!(stats.new_connections, 1); // First use of port
    assert_eq!(stats.reused_connections, 9);
}

#[test]
fn test_port_tracking_mixed_pattern() {
    let tracker = PoolStatsTracker::new();

    let addr1: SocketAddr = "127.0.0.1:50001".parse().unwrap();
    let addr2: SocketAddr = "127.0.0.1:50002".parse().unwrap();
    let addr3: SocketAddr = "127.0.0.1:50003".parse().unwrap();

    tracker.record_request(Some(addr1)); // New
    tracker.record_request(Some(addr1)); // Reused
    tracker.record_request(Some(addr2)); // New
    tracker.record_request(Some(addr1)); // Reused
    tracker.record_request(Some(addr2)); // Reused
    tracker.record_request(Some(addr3)); // New
    tracker.record_request(Some(addr3)); // Reused
    tracker.record_request(Some(addr1)); // Reused

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 8);
    assert_eq!(stats.new_connections, 3);
    assert_eq!(stats.reused_connections, 5);
    assert_eq!(stats.reuse_rate(), 62.5);
    assert_eq!(stats.new_connection_rate(), 37.5);
}

#[test]
fn test_port_tracking_none_addr() {
    let tracker = PoolStatsTracker::new();

    // No local_addr (failed requests) — only total counted
    tracker.record_request(None);
    tracker.record_request(None);
    tracker.record_request(None);

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 3);
    assert_eq!(stats.new_connections, 0);
    assert_eq!(stats.reused_connections, 0);
}

#[test]
fn test_pool_stats_tracker_reset() {
    let tracker = PoolStatsTracker::new();

    let addr: SocketAddr = "127.0.0.1:50001".parse().unwrap();
    tracker.record_request(Some(addr));
    tracker.record_request(Some(addr));
    tracker.record_request(None);

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 3);

    tracker.reset();

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 0);
    assert_eq!(stats.new_connections, 0);
    assert_eq!(stats.reused_connections, 0);

    // After reset, same port should be new again
    tracker.record_request(Some(addr));
    let stats = tracker.stats();
    assert_eq!(stats.new_connections, 1);
    assert_eq!(stats.reused_connections, 0);
}

#[test]
fn test_pool_stats_timing_accuracy() {
    let tracker = PoolStatsTracker::new();

    tracker.record_request(None);
    std::thread::sleep(Duration::from_millis(100));
    tracker.record_request(None);

    let stats = tracker.stats();
    let duration = stats.duration().unwrap();

    assert!(duration >= Duration::from_millis(100));
    assert!(duration < Duration::from_millis(200));
}

#[test]
fn test_connection_stats_duration_calculation() {
    use std::time::Instant;

    let start = Instant::now();
    std::thread::sleep(Duration::from_millis(50));
    let end = Instant::now();

    let stats = ConnectionStats {
        total_requests: 10,
        new_connections: 2,
        reused_connections: 8,
        first_request: Some(start),
        last_request: Some(end),
    };

    let duration = stats.duration().unwrap();
    assert!(duration >= Duration::from_millis(50));
    assert!(duration < Duration::from_millis(100));
}

#[test]
fn test_port_tracking_high_reuse_scenario() {
    let tracker = PoolStatsTracker::new();

    // One connection reused 99 times
    let addr: SocketAddr = "127.0.0.1:50001".parse().unwrap();
    for _ in 0..100 {
        tracker.record_request(Some(addr));
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 100);
    assert_eq!(stats.new_connections, 1);
    assert_eq!(stats.reused_connections, 99);
    assert_eq!(stats.reuse_rate(), 99.0);
}

#[test]
fn test_pool_stats_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let tracker = Arc::new(PoolStatsTracker::new());
    let mut handles = vec![];

    // 5 threads, each using a unique port (simulating new connections)
    for thread_id in 0..5u16 {
        let tracker_clone = Arc::clone(&tracker);
        let handle = thread::spawn(move || {
            let addr: SocketAddr =
                format!("127.0.0.1:{}", 50000 + thread_id).parse().unwrap();
            for _ in 0..20 {
                tracker_clone.record_request(Some(addr));
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let stats = tracker.stats();
    assert_eq!(stats.total_requests, 100); // 5 threads * 20 requests
    assert_eq!(stats.new_connections, 5); // 5 unique ports
    assert_eq!(stats.reused_connections, 95); // rest are reuses
}

#[test]
fn test_global_pool_stats_singleton() {
    let stats = GLOBAL_POOL_STATS.stats();
    let _ = stats.total_requests;
}

#[test]
fn test_pool_config_apply_to_builder() {
    let config = PoolConfig::new()
        .with_max_idle_per_host(64)
        .with_idle_timeout(Duration::from_secs(120))
        .with_tcp_keepalive(Some(Duration::from_secs(30)));

    let builder = reqwest::Client::builder();
    let _builder = config.apply_to_builder(builder);
}

#[tokio::test]
async fn test_pool_with_real_client() {
    let config = PoolConfig::new()
        .with_max_idle_per_host(10)
        .with_idle_timeout(Duration::from_secs(30));

    let builder = reqwest::Client::builder();
    let builder = config.apply_to_builder(builder);
    let client = builder.build().expect("Failed to build client");

    assert!(client.get("http://example.com").build().is_ok());
}

#[test]
fn test_connection_stats_format_variations() {
    let test_cases = vec![
        (100, 0, 100), // 100% reuse
        (100, 100, 0), // 0% reuse (all new)
        (100, 50, 50), // 50/50
        (100, 75, 25), // 75% reuse
    ];

    for (total, new, reused) in test_cases {
        let stats = ConnectionStats {
            total_requests: total,
            new_connections: new,
            reused_connections: reused,
            first_request: Some(std::time::Instant::now()),
            last_request: Some(std::time::Instant::now()),
        };

        let formatted = stats.format();
        assert!(formatted.contains(&format!("Total: {}", total)));
        assert!(formatted.contains(&format!("New: {}", new)));
        assert!(formatted.contains(&format!("Reused: {}", reused)));
    }
}
