//! Connection pool configuration and monitoring.
//!
//! Tracks connection reuse accurately using local TCP port comparison
//! (Issue #119).  Each response's local SocketAddr is extracted from hyper's
//! HttpInfo extension — a new local port means a new TCP connection, same
//! port means the connection was reused from the pool.

use std::collections::HashSet;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;

use crate::metrics::{
    CONNECTION_POOL_NEW_TOTAL, CONNECTION_POOL_REQUESTS_TOTAL, CONNECTION_POOL_REUSED_TOTAL,
    CONNECTION_POOL_REUSE_RATE,
};

/// Connection pool configuration.
#[derive(Debug, Clone)]
pub struct PoolConfig {
    /// Maximum idle connections to keep per host
    pub max_idle_per_host: usize,

    /// How long idle connections stay in the pool before cleanup
    pub idle_timeout: Duration,

    /// TCP keepalive duration
    pub tcp_keepalive: Option<Duration>,

    /// Disable Nagle's algorithm for lower latency at high RPS
    pub tcp_nodelay: bool,

    /// Per-request timeout — prevents hung connections from accumulating memory
    pub request_timeout: Duration,
}

impl Default for PoolConfig {
    fn default() -> Self {
        Self {
            max_idle_per_host: 32,
            idle_timeout: Duration::from_secs(30),
            tcp_keepalive: Some(Duration::from_secs(60)),
            tcp_nodelay: true,
            request_timeout: Duration::from_secs(30),
        }
    }
}

impl PoolConfig {
    /// Create a new pool configuration.
    pub fn new() -> Self {
        Self::default()
    }

    /// Load pool configuration from environment variables with defaults.
    ///
    /// Reads: `POOL_MAX_IDLE_PER_HOST`, `POOL_IDLE_TIMEOUT_SECS`, `TCP_NODELAY`
    pub fn from_env() -> Self {
        let max_idle_per_host: usize = std::env::var("POOL_MAX_IDLE_PER_HOST")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(32);

        let idle_timeout_secs: u64 = std::env::var("POOL_IDLE_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        let tcp_nodelay: bool = std::env::var("TCP_NODELAY")
            .unwrap_or_else(|_| "true".to_string())
            .to_lowercase()
            == "true";

        let request_timeout_secs: u64 = std::env::var("REQUEST_TIMEOUT_SECS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(30);

        Self {
            max_idle_per_host,
            idle_timeout: Duration::from_secs(idle_timeout_secs),
            tcp_keepalive: Some(Duration::from_secs(60)),
            tcp_nodelay,
            request_timeout: Duration::from_secs(request_timeout_secs),
        }
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

    /// Set TCP no-delay (disables Nagle's algorithm).
    pub fn with_tcp_nodelay(mut self, nodelay: bool) -> Self {
        self.tcp_nodelay = nodelay;
        self
    }

    /// Apply this configuration to a reqwest ClientBuilder.
    pub fn apply_to_builder(&self, builder: reqwest::ClientBuilder) -> reqwest::ClientBuilder {
        let mut builder = builder
            .pool_max_idle_per_host(self.max_idle_per_host)
            .pool_idle_timeout(self.idle_timeout)
            .tcp_nodelay(self.tcp_nodelay)
            .timeout(self.request_timeout);

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

    /// Requests that used a new TCP connection (by local port)
    pub new_connections: u64,

    /// Requests that reused a pooled TCP connection (by local port)
    pub reused_connections: u64,

    /// First request timestamp (for rate calculations)
    pub first_request: Option<Instant>,

    /// Last request timestamp
    pub last_request: Option<Instant>,
}

impl ConnectionStats {
    /// Calculate the connection reuse rate.
    pub fn reuse_rate(&self) -> f64 {
        let tracked = self.new_connections + self.reused_connections;
        if tracked == 0 {
            return 0.0;
        }
        (self.reused_connections as f64 / tracked as f64) * 100.0
    }

    /// Calculate the new connection rate.
    pub fn new_connection_rate(&self) -> f64 {
        let tracked = self.new_connections + self.reused_connections;
        if tracked == 0 {
            return 0.0;
        }
        (self.new_connections as f64 / tracked as f64) * 100.0
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
            self.reused_connections,
            self.reuse_rate(),
            self.new_connections,
            self.new_connection_rate()
        )
    }
}

/// Tracker for connection pool statistics.
///
/// Uses local TCP port tracking to deterministically identify new vs reused
/// connections. A new local port = new TCP connection. Same port = reused.
#[derive(Clone)]
pub struct PoolStatsTracker {
    stats: Arc<Mutex<ConnectionStats>>,
    seen_ports: Arc<Mutex<HashSet<u16>>>,
}

impl PoolStatsTracker {
    /// Create a new pool statistics tracker.
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(ConnectionStats::default())),
            seen_ports: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Record a request with its local socket address for connection tracking.
    ///
    /// `local_addr` is obtained from `response.extensions().get::<HttpInfo>()`.
    /// When `None` (e.g. request failed before a connection was established),
    /// only the total request counter is incremented.
    pub fn record_request(&self, local_addr: Option<SocketAddr>) {
        let now = Instant::now();
        let mut stats = self.stats.lock().unwrap();

        stats.total_requests += 1;
        CONNECTION_POOL_REQUESTS_TOTAL.inc();

        if stats.first_request.is_none() {
            stats.first_request = Some(now);
        }
        stats.last_request = Some(now);

        if let Some(addr) = local_addr {
            let port = addr.port();
            let mut ports = self.seen_ports.lock().unwrap();
            if ports.insert(port) {
                stats.new_connections += 1;
                CONNECTION_POOL_NEW_TOTAL.inc();
                debug!(local_port = port, "New TCP connection (new local port)");
            } else {
                stats.reused_connections += 1;
                CONNECTION_POOL_REUSED_TOTAL.inc();
                debug!(
                    local_port = port,
                    "Reused TCP connection (seen local port)"
                );
            }
            CONNECTION_POOL_REUSE_RATE.set(stats.reuse_rate());
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
        let mut ports = self.seen_ports.lock().unwrap();
        ports.clear();
    }
}

impl Default for PoolStatsTracker {
    fn default() -> Self {
        Self::new()
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
        assert_eq!(config.idle_timeout, Duration::from_secs(30));
        assert_eq!(config.tcp_keepalive, Some(Duration::from_secs(60)));
        assert!(config.tcp_nodelay);
        assert_eq!(config.request_timeout, Duration::from_secs(30));
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
            new_connections: 10,
            reused_connections: 90,
            first_request: Some(Instant::now()),
            last_request: Some(Instant::now()),
        };

        assert_eq!(stats.reuse_rate(), 90.0);
        assert_eq!(stats.new_connection_rate(), 10.0);
    }

    #[test]
    fn test_port_tracking_new_connections() {
        let tracker = PoolStatsTracker::new();

        let addr1: SocketAddr = "127.0.0.1:50001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:50002".parse().unwrap();
        let addr3: SocketAddr = "127.0.0.1:50003".parse().unwrap();

        tracker.record_request(Some(addr1));
        tracker.record_request(Some(addr2));
        tracker.record_request(Some(addr3));

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.new_connections, 3);
        assert_eq!(stats.reused_connections, 0);
        assert_eq!(stats.reuse_rate(), 0.0);
    }

    #[test]
    fn test_port_tracking_reused_connections() {
        let tracker = PoolStatsTracker::new();

        let addr: SocketAddr = "127.0.0.1:50001".parse().unwrap();

        tracker.record_request(Some(addr));
        tracker.record_request(Some(addr));
        tracker.record_request(Some(addr));

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 3);
        assert_eq!(stats.new_connections, 1);
        assert_eq!(stats.reused_connections, 2);
    }

    #[test]
    fn test_port_tracking_mixed() {
        let tracker = PoolStatsTracker::new();

        let addr1: SocketAddr = "127.0.0.1:50001".parse().unwrap();
        let addr2: SocketAddr = "127.0.0.1:50002".parse().unwrap();

        tracker.record_request(Some(addr1)); // New
        tracker.record_request(Some(addr1)); // Reused
        tracker.record_request(Some(addr2)); // New
        tracker.record_request(Some(addr1)); // Reused
        tracker.record_request(Some(addr2)); // Reused

        let stats = tracker.stats();
        assert_eq!(stats.new_connections, 2);
        assert_eq!(stats.reused_connections, 3);
        assert_eq!(stats.reuse_rate(), 60.0);
    }

    #[test]
    fn test_port_tracking_none_addr() {
        let tracker = PoolStatsTracker::new();

        tracker.record_request(None);
        tracker.record_request(None);

        let stats = tracker.stats();
        assert_eq!(stats.total_requests, 2);
        assert_eq!(stats.new_connections, 0);
        assert_eq!(stats.reused_connections, 0);
    }

    #[test]
    fn test_reset_clears_ports() {
        let tracker = PoolStatsTracker::new();

        let addr: SocketAddr = "127.0.0.1:50001".parse().unwrap();
        tracker.record_request(Some(addr));
        tracker.record_request(Some(addr));

        assert_eq!(tracker.stats().new_connections, 1);
        assert_eq!(tracker.stats().reused_connections, 1);

        tracker.reset();

        tracker.record_request(Some(addr));
        assert_eq!(tracker.stats().new_connections, 1);
        assert_eq!(tracker.stats().reused_connections, 0);
    }

    #[test]
    fn test_connection_stats_format() {
        let stats = ConnectionStats {
            total_requests: 100,
            new_connections: 20,
            reused_connections: 80,
            first_request: Some(Instant::now()),
            last_request: Some(Instant::now()),
        };

        let formatted = stats.format();
        assert!(formatted.contains("Total: 100"));
        assert!(formatted.contains("Reused: 80"));
        assert!(formatted.contains("80.0%"));
        assert!(formatted.contains("New: 20"));
        assert!(formatted.contains("20.0%"));
    }

    #[test]
    fn test_pool_stats_timing() {
        let tracker = PoolStatsTracker::new();

        tracker.record_request(None);
        std::thread::sleep(Duration::from_millis(100));
        tracker.record_request(None);

        let stats = tracker.stats();
        let duration = stats.duration().unwrap();
        assert!(duration >= Duration::from_millis(100));
        assert!(duration < Duration::from_millis(200));
    }
}
