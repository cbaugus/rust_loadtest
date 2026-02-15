//! Connection pool configuration and monitoring.
//!
//! This module provides connection pool statistics tracking and configuration.
//! Since reqwest doesn't expose internal pool metrics, we track connection
//! behavior patterns and configuration to provide insights into pool utilization.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum idle connections to keep per host
    pub max_idle_per_host: usize,

    /// How long idle connections stay in the pool before cleanup
    pub idle_timeout: Duration,

    /// TCP keepalive duration
    pub tcp_keepalive: Option<Duration>,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout: Duration::from_secs(90),
            tcp_keepalive: Some(Duration::from_secs(60)),
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set maximum idle connections per host.
    pub fn with_max_idle_per_host(mut self, max: usize) -> Self {
        self.max_idle_per_host = max;
        self
    }

    /// Set idle connection timeout.
    pub fn with_idle_timeout(mut self, timeout: Duration) -> Self {
        self.idle_timeout = timeout;
        self
    }

    /// Set TCP keepalive duration.
    pub fn with_tcp_keepalive(mut self, keepalive: Option<Duration>) -> Self {
        self.tcp_keepalive = keepalive;
        self
    }

    /// Apply this configuration to a reqwest ClientBuilder.
    pub fn apply_to_builder(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        let mut builder = builder
            .pool_max_idle_per_host(self.max_idle_per_host)
            .pool_idle_timeout(self.idle_timeout);

        if let Some(keepalive) = self.tcp_keepalive {
            builder = builder.tcp_keepalive(keepalive);
        }

        builder
    }
}

/// Connection statistics for monitoring pool behavior.
#[derive(Debug, Clone, Default)]
pub struct ConnectionStats {
    /// Total requests made
    pub total_requests: u64,

    /// Requests that likely used a new connection (slow initial handshake)
    pub likely_new_connections: u64,

    /// Requests that likely reused a connection (fast, no TLS handshake)
    pub likely_reused_connections: u64,

    /// First request timestamp (for rate calculations)
    pub first_request: Option<Instant>,

    /// Last request timestamp
    pub last_request: Option<Instant>,
}

impl ConnectionStats {
    /// Calculate the connection reuse rate.
    pub fn reuse_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.likely_reused_connections as f64 / self.total_requests as f64) * 100.0
    }

    /// Calculate the new connection rate.
    pub fn new_connection_rate(&self) -> f64 {
        if self.total_requests == 0 {
            return 0.0;
        }
        (self.likely_new_connections as f64 / self.total_requests as f64) * 100.0
    }

    /// Get the duration over which requests were tracked.
    pub fn duration(&self) -> Option<Duration> {
        match (self.first_request, self.last_request) {
            (Some(first), Some(last)) => Some(last.duration_since(first)),
            _ => None,
        }
    }

    /// Format statistics as a human-readable string.
    pub fn format(&self) -> String {
        format!(
            "Total: {}, Reused: {} ({:.1}%), New: {} ({:.1}%)",
            self.total_requests,
            self.likely_reused_connections,
            self.reuse_rate(),
            self.likely_new_connections,
            self.new_connection_rate()
        )
    }
}

/// Tracker for connection pool statistics.
///
/// This tracker monitors connection behavior patterns to provide insights
/// into connection reuse. It uses timing heuristics to infer whether a
/// connection was likely reused or newly established.
#[derive(Clone)]
pub struct PoolStatsTracker {
    stats: Arc<Mutex<ConnectionStats>>,

    /// Threshold for considering a connection "likely new" (milliseconds)
    /// Requests slower than this are likely establishing new connections
    new_connection_threshold_ms: u64,
}

impl PoolStatsTracker {
    /// Create a new pool statistics tracker.
    ///
    /// # Arguments
    /// * `new_connection_threshold_ms` - Latency threshold (ms) above which we
    ///   consider a connection likely new (includes TLS handshake time)
    pub fn new(new_connection_threshold_ms: u64) -> Self {
        Self {
            stats: Arc::new(Mutex::new(ConnectionStats::default())),
            new_connection_threshold_ms,
        }
    }

    /// Record a request with timing information.
    ///
    /// Uses latency to infer connection reuse. Requests with very low latency
    /// (<50ms typically) likely reused an existing connection. Slower requests
    /// may have established a new connection (including TLS handshake).
    pub fn record_request(&self, latency_ms: u64) {
        let now = Instant::now();
        let mut stats = self.stats.lock().unwrap();

        stats.total_requests += 1;

        // Track timing
        if stats.first_request.is_none() {
            stats.first_request = Some(now);
        }
        stats.last_request = Some(now);

        // Infer connection type based on latency
        // Fast requests (<threshold) likely reused connections
        // Slow requests likely established new connections (TLS handshake adds ~50-100ms)
        if latency_ms >= self.new_connection_threshold_ms {
            stats.likely_new_connections += 1;
            debug!(
                latency_ms = latency_ms,
                threshold = self.new_connection_threshold_ms,
                "Request latency suggests new connection"
            );
        } else {
            stats.likely_reused_connections += 1;
            debug!(
                latency_ms = latency_ms,
                threshold = self.new_connection_threshold_ms,
                "Request latency suggests reused connection"
            );
        }
    }

    /// Get current connection statistics.
    pub fn stats(&self) -> ConnectionStats {
        self.stats.lock().unwrap().clone()
    }

    /// Reset all statistics.
    pub fn reset(&self) {
        let mut stats = self.stats.lock().unwrap();
        *stats = ConnectionStats::default();
    }
}

impl Default for PoolStatsTracker {
    fn default() -> Self {
        // Default threshold of 100ms to distinguish new vs reused connections
        // TLS handshake typically adds 50-150ms depending on network conditions
        Self::new(100)
    }
}

// Global pool statistics tracker.
lazy_static::lazy_static! {
    pub static ref GLOBAL_POOL_STATS: PoolStatsTracker = PoolStatsTracker::default();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_config_defaults() {
        let config = PoolConfig::default();
        assert_eq!(config.max_idle_per_host, 32);
        assert_eq!(config.idle_timeout, Duration::from_secs(90));
        assert_eq!(config.tcp_keepalive, Some(Duration::from_secs(60)));
    }

    #[test]
    fn test_pool_config_builder() {
        let config = PoolConfig::new()
            .with_max_idle_per_host(64)
            .with_idle_timeout(Duration::from_secs(120))
            .with_tcp_keepalive(None);

        assert_eq!(config.max_idle_per_host, 64);
        assert_eq!(config.idle_timeout, Duration::from_secs(120));
        assert_eq!(config.tcp_keepalive, None);
    }

    #[test]
    fn test_connection_stats_empty() {
        let stats = ConnectionStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.reuse_rate(), 0.0);
        assert_eq!(stats.new_connection_rate(), 0.0);
        assert!(stats.duration().is_none());
    }

    #[test]
    fn test_connection_stats_rates() {
        let stats = ConnectionStats {
            total_requests: 100,
            likely_new_connections: 20,
            likely_reused_connections: 80,
            first_request: Some(Instant::now()),
            last_request: Some(Instant::now()),
        };

        assert_eq!(stats.reuse_rate(), 80.0);
        assert_eq!(stats.new_connection_rate(), 20.0);
    }

    #[test]
    fn test_pool_stats_tracker_fast_requests() {
        let tracker = PoolStatsTracker::new(100);

        // Simulate 10 fast requests (likely reused connections)
        for _ in 0..10 {
            tracker.record_request(20); // 20ms - fast
        }

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 10);
        assert_eq!(stats.likely_reused_connections, 10);
        assert_eq!(stats.likely_new_connections, 0);
        assert_eq!(stats.reuse_rate(), 100.0);
    }

    #[test]
    fn test_pool_stats_tracker_slow_requests() {
        let tracker = PoolStatsTracker::new(100);

        // Simulate 10 slow requests (likely new connections)
        for _ in 0..10 {
            tracker.record_request(150); // 150ms - slow (includes TLS handshake)
        }

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 10);
        assert_eq!(stats.likely_reused_connections, 0);
        assert_eq!(stats.likely_new_connections, 10);
        assert_eq!(stats.new_connection_rate(), 100.0);
    }

    #[test]
    fn test_pool_stats_tracker_mixed() {
        let tracker = PoolStatsTracker::new(100);

        // Simulate mixed requests
        tracker.record_request(150); // New connection (slow)
        tracker.record_request(30); // Reused (fast)
        tracker.record_request(25); // Reused (fast)
        tracker.record_request(120); // New connection (slow)
        tracker.record_request(40); // Reused (fast)

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 5);
        assert_eq!(stats.likely_reused_connections, 3);
        assert_eq!(stats.likely_new_connections, 2);
        assert_eq!(stats.reuse_rate(), 60.0);
        assert_eq!(stats.new_connection_rate(), 40.0);
    }

    #[test]
    fn test_pool_stats_tracker_reset() {
        let tracker = PoolStatsTracker::new(100);

        tracker.record_request(50);
        tracker.record_request(150);

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 2);

        tracker.reset();

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.likely_reused_connections, 0);
        assert_eq!(stats.likely_new_connections, 0);
    }

    #[test]
    fn test_connection_stats_format() {
        let stats = ConnectionStats {
            total_requests: 100,
            likely_new_connections: 25,
            likely_reused_connections: 75,
            first_request: Some(Instant::now()),
            last_request: Some(Instant::now()),
        };

        let formatted = stats.format();
        assert!(formatted.contains("Total: 100"));
        assert!(formatted.contains("Reused: 75"));
        assert!(formatted.contains("75.0%"));
        assert!(formatted.contains("New: 25"));
        assert!(formatted.contains("25.0%"));
    }

    #[test]
    fn test_pool_stats_timing() {
        let tracker = PoolStatsTracker::new(100);

        tracker.record_request(50);
        std::thread::sleep(Duration::from_millis(100));
        tracker.record_request(50);

        let stats = tracker.stats();
        let duration = stats.duration().unwrap();

        assert!(duration >= Duration::from_millis(100));
        assert!(duration < Duration::from_millis(200));
    }

    #[test]
    fn test_custom_threshold() {
        let tracker = PoolStatsTracker::new(200); // Higher threshold

        tracker.record_request(150); // Under threshold - reused
        tracker.record_request(250); // Over threshold - new

        let stats = tracker.stats();
        assert_eq!(stats.likely_reused_connections, 1);
        assert_eq!(stats.likely_new_connections, 1);
    }
}
