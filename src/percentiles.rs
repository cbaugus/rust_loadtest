//! Percentile latency tracking using HDR Histogram.
//!
//! This module provides accurate percentile calculation for request latencies
//! using HdrHistogram, which is the industry standard for latency measurement.
//!
//! # Features
//! - P50 (median), P90, P95, P99, P99.9 percentile tracking
//! - Per-endpoint percentile tracking
//! - Per-scenario percentile tracking
//! - Thread-safe concurrent updates
//! - Memory-efficient histogram storage

use hdrhistogram::Histogram;
use lru::LruCache;
use std::collections::HashMap;
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};
use tracing::{debug, warn};

/// Percentile statistics for a set of latency measurements.
#[derive(Debug, Clone)]
pub struct PercentileStats {
    /// Number of samples
    pub count: u64,

    /// Minimum value (microseconds)
    pub min: u64,

    /// Maximum value (microseconds)
    pub max: u64,

    /// Mean/average value (microseconds)
    pub mean: f64,

    /// 50th percentile - median (microseconds)
    pub p50: u64,

    /// 90th percentile (microseconds)
    pub p90: u64,

    /// 95th percentile (microseconds)
    pub p95: u64,

    /// 99th percentile (microseconds)
    pub p99: u64,

    /// 99.9th percentile (microseconds)
    pub p99_9: u64,
}

impl PercentileStats {
    /// Format statistics as a human-readable string.
    pub fn format(&self) -> String {
        format!(
            "count={}, min={:.2}ms, max={:.2}ms, mean={:.2}ms, p50={:.2}ms, p90={:.2}ms, p95={:.2}ms, p99={:.2}ms, p99.9={:.2}ms",
            self.count,
            self.min as f64 / 1000.0,
            self.max as f64 / 1000.0,
            self.mean / 1000.0,
            self.p50 as f64 / 1000.0,
            self.p90 as f64 / 1000.0,
            self.p95 as f64 / 1000.0,
            self.p99 as f64 / 1000.0,
            self.p99_9 as f64 / 1000.0,
        )
    }

    /// Format statistics as a compact table row.
    pub fn format_table_row(&self, label: &str) -> String {
        format!(
            "{:<30} {:>8} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2} {:>8.2}",
            label,
            self.count,
            self.p50 as f64 / 1000.0,
            self.p90 as f64 / 1000.0,
            self.p95 as f64 / 1000.0,
            self.p99 as f64 / 1000.0,
            self.p99_9 as f64 / 1000.0,
            self.mean / 1000.0,
            self.max as f64 / 1000.0,
        )
    }
}

/// Thread-safe percentile tracker.
///
/// Uses HdrHistogram internally for efficient percentile calculation.
/// All latencies are stored in microseconds.
pub struct PercentileTracker {
    /// HDR Histogram for efficient percentile calculation
    /// Tracks latencies from 1 microsecond to 60 seconds with 3 significant digits
    histogram: Arc<Mutex<Histogram<u64>>>,
}

impl PercentileTracker {
    /// Create a new percentile tracker.
    ///
    /// Configures histogram to track latencies from 1μs to 60 seconds
    /// with 3 significant digits of precision.
    pub fn new() -> Self {
        // Create histogram that can track 1μs to 60s with 3 significant digits
        let histogram =
            Histogram::new_with_bounds(1, 60_000_000, 3).expect("Failed to create histogram");

        Self {
            histogram: Arc::new(Mutex::new(histogram)),
        }
    }

    /// Record a latency measurement in milliseconds.
    ///
    /// # Arguments
    /// * `latency_ms` - Latency in milliseconds
    pub fn record_ms(&self, latency_ms: u64) {
        let latency_us = latency_ms * 1000; // Convert to microseconds
        self.record_us(latency_us);
    }

    /// Record a latency measurement in microseconds.
    ///
    /// # Arguments
    /// * `latency_us` - Latency in microseconds
    pub fn record_us(&self, latency_us: u64) {
        let mut hist = self.histogram.lock().unwrap();

        // Clamp to valid range (1μs to 60s)
        let clamped = latency_us.clamp(1, 60_000_000);

        if let Err(e) = hist.record(clamped) {
            warn!(
                latency_us = latency_us,
                error = %e,
                "Failed to record latency in histogram"
            );
        }
    }

    /// Get current percentile statistics.
    ///
    /// Returns None if no samples have been recorded.
    pub fn stats(&self) -> Option<PercentileStats> {
        let hist = self.histogram.lock().unwrap();

        if hist.is_empty() {
            return None;
        }

        Some(PercentileStats {
            count: hist.len(),
            min: hist.min(),
            max: hist.max(),
            mean: hist.mean(),
            p50: hist.value_at_quantile(0.50),
            p90: hist.value_at_quantile(0.90),
            p95: hist.value_at_quantile(0.95),
            p99: hist.value_at_quantile(0.99),
            p99_9: hist.value_at_quantile(0.999),
        })
    }

    /// Reset all recorded samples.
    pub fn reset(&self) {
        let mut hist = self.histogram.lock().unwrap();
        hist.clear();
    }
}

impl Default for PercentileTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Multi-label percentile tracker with LRU eviction (Issue #68).
///
/// Tracks percentiles separately for different labels (e.g., endpoints, scenarios).
/// Thread-safe for concurrent updates. Uses LRU eviction to limit memory usage.
pub struct MultiLabelPercentileTracker {
    trackers: Arc<Mutex<LruCache<String, PercentileTracker>>>,
    max_labels: usize,
    warned_at_80_percent: Arc<Mutex<bool>>,
}

impl MultiLabelPercentileTracker {
    /// Create a new multi-label tracker with a maximum number of labels.
    ///
    /// # Arguments
    /// * `max_labels` - Maximum number of unique labels to track (default: 100)
    ///
    /// When the limit is reached, least recently used labels are evicted.
    pub fn new_with_limit(max_labels: usize) -> Self {
        let capacity = NonZeroUsize::new(max_labels).unwrap_or(NonZeroUsize::new(100).unwrap());
        Self {
            trackers: Arc::new(Mutex::new(LruCache::new(capacity))),
            max_labels,
            warned_at_80_percent: Arc::new(Mutex::new(false)),
        }
    }

    /// Create a new multi-label tracker with default limit of 100 labels.
    pub fn new() -> Self {
        Self::new_with_limit(100)
    }

    /// Record a latency for a specific label.
    ///
    /// # Arguments
    /// * `label` - Label to track (e.g., endpoint path, scenario name)
    /// * `latency_ms` - Latency in milliseconds
    ///
    /// If the label doesn't exist and we're at capacity, the least recently
    /// used label will be evicted to make room.
    pub fn record(&self, label: &str, latency_ms: u64) {
        let mut trackers = self.trackers.lock().unwrap();

        // Check if we're approaching the limit (80%)
        let current_size = trackers.len();
        let threshold_80 = (self.max_labels as f64 * 0.8) as usize;

        if current_size >= threshold_80 && !trackers.contains(&label.to_string()) {
            let mut warned = self.warned_at_80_percent.lock().unwrap();
            if !*warned {
                warn!(
                    current_labels = current_size,
                    max_labels = self.max_labels,
                    threshold_percent = 80,
                    "⚠️  Histogram label limit approaching: {}/{} labels ({}%). \
                     Consider increasing MAX_HISTOGRAM_LABELS or using fewer unique scenario/step names. \
                     Least recently used labels will be evicted when limit is reached.",
                    current_size, self.max_labels, (current_size as f64 / self.max_labels as f64 * 100.0) as u32
                );
                *warned = true;
            }
        }

        // Get or create tracker for this label
        // LRU will automatically evict oldest entry if at capacity
        if !trackers.contains(&label.to_string()) {
            if trackers.len() >= self.max_labels {
                debug!(
                    label = label,
                    max_labels = self.max_labels,
                    "Histogram label limit reached, evicting least recently used label"
                );
                crate::metrics::HISTOGRAM_LABELS_EVICTED_TOTAL.inc();
            }
            trackers.put(label.to_string(), PercentileTracker::new());
        }

        // Record the latency
        if let Some(tracker) = trackers.get_mut(&label.to_string()) {
            tracker.record_ms(latency_ms);
        }
    }

    /// Get statistics for a specific label.
    ///
    /// Returns None if label doesn't exist or has no samples.
    pub fn stats(&self, label: &str) -> Option<PercentileStats> {
        let trackers = self.trackers.lock().unwrap();
        // peek() doesn't update LRU order
        trackers.peek(label).and_then(|t| t.stats())
    }

    /// Get statistics for all labels.
    ///
    /// Returns a map of label -> statistics.
    pub fn all_stats(&self) -> HashMap<String, PercentileStats> {
        let trackers = self.trackers.lock().unwrap();
        let mut results = HashMap::new();

        for (label, tracker) in trackers.iter() {
            if let Some(stats) = tracker.stats() {
                results.insert(label.clone(), stats);
            }
        }

        results
    }

    /// Get all labels currently being tracked.
    pub fn labels(&self) -> Vec<String> {
        let trackers = self.trackers.lock().unwrap();
        trackers.iter().map(|(k, _)| k.clone()).collect()
    }
    /// Get the current number of tracked labels.
    pub fn len(&self) -> usize {
        let trackers = self.trackers.lock().unwrap();
        trackers.len()
    }

    /// Check if there are no tracked labels.
    pub fn is_empty(&self) -> bool {
        let trackers = self.trackers.lock().unwrap();
        trackers.is_empty()
    }

    /// Get the maximum number of labels that can be tracked.
    pub fn capacity(&self) -> usize {
        self.max_labels
    }

    /// Reset all trackers.
    pub fn reset_all(&self) {
        let mut trackers = self.trackers.lock().unwrap();
        trackers.clear();
        // Reset the warning flag
        let mut warned = self.warned_at_80_percent.lock().unwrap();
        *warned = false;
    }

    /// Rotate histograms by clearing all data (Issue #67).
    ///
    /// This resets all histogram data to free memory while keeping
    /// the label structure intact. Called periodically for long-running tests.
    pub fn rotate(&self) {
        let trackers = self.trackers.lock().unwrap();

        // Clear data in each histogram
        for (_label, tracker) in trackers.iter() {
            tracker.reset();
        }

        // Reset the warning flag since we're starting fresh
        let mut warned = self.warned_at_80_percent.lock().unwrap();
        *warned = false;
    }
}

impl Default for MultiLabelPercentileTracker {
    fn default() -> Self {
        Self::new()
    }
}

// Global percentile trackers for the application.
//
// These are lazily initialized and thread-safe.
lazy_static::lazy_static! {
    /// Global tracker for single request latencies
    pub static ref GLOBAL_REQUEST_PERCENTILES: PercentileTracker = PercentileTracker::new();

    /// Global tracker for scenario latencies (by scenario name)
    pub static ref GLOBAL_SCENARIO_PERCENTILES: MultiLabelPercentileTracker = MultiLabelPercentileTracker::new();

    /// Global tracker for step latencies (by scenario:step)
    pub static ref GLOBAL_STEP_PERCENTILES: MultiLabelPercentileTracker = MultiLabelPercentileTracker::new();
}

/// Rotate all global histogram trackers (Issue #67).
///
/// Clears histogram data to free memory while keeping labels intact.
/// Should be called periodically for long-running tests to bound memory usage.
pub fn rotate_all_histograms() {
    GLOBAL_REQUEST_PERCENTILES.reset();
    GLOBAL_SCENARIO_PERCENTILES.rotate();
    GLOBAL_STEP_PERCENTILES.rotate();
}

/// Format percentile statistics as a table.
///
/// # Arguments
/// * `title` - Table title
/// * `stats_map` - Map of label -> statistics
///
/// # Returns
/// Formatted table string
pub fn format_percentile_table(
    title: &str,
    stats_map: &HashMap<String, PercentileStats>,
) -> String {
    if stats_map.is_empty() {
        return format!("## {}\n\nNo data available.\n", title);
    }

    let mut output = String::new();
    output.push_str(&format!("\n## {}\n\n", title));
    output.push_str(&format!(
        "{:<30} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}\n",
        "Label", "Count", "P50", "P90", "P95", "P99", "P99.9", "Mean", "Max"
    ));
    output.push_str(&format!(
        "{:<30} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8} {:>8}\n",
        "", "", "(ms)", "(ms)", "(ms)", "(ms)", "(ms)", "(ms)", "(ms)"
    ));
    output.push_str(&"-".repeat(120));
    output.push('\n');

    // Sort labels for consistent output
    let mut labels: Vec<_> = stats_map.keys().collect();
    labels.sort();

    for label in labels {
        let stats = &stats_map[label];
        output.push_str(&stats.format_table_row(label));
        output.push('\n');
    }

    output
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_percentile_tracker_basic() {
        let tracker = PercentileTracker::new();

        // Record some values: 10ms, 20ms, 30ms, 40ms, 50ms
        for i in 1..=5 {
            tracker.record_ms(i * 10);
        }

        let stats = tracker.stats().expect("Should have stats");
        assert_eq!(stats.count, 5);
        assert_eq!(stats.min, 10_000); // 10ms in microseconds

        // HDR histogram has precision limits - use tolerance for max value
        // Expected 50_000 but histogram may round to ~50_015 due to bucketing
        let expected_max = 50_000;
        let tolerance = 100; // 0.2% tolerance for histogram precision
        assert!(
            stats.max >= expected_max && stats.max <= expected_max + tolerance,
            "max should be ~{} but was {}",
            expected_max,
            stats.max
        );
    }

    #[test]
    fn test_percentile_tracker_empty() {
        let tracker = PercentileTracker::new();
        assert!(tracker.stats().is_none());
    }

    #[test]
    fn test_percentile_tracker_single_value() {
        let tracker = PercentileTracker::new();
        tracker.record_ms(100);

        let stats = tracker.stats().unwrap();
        assert_eq!(stats.count, 1);

        // HDR histogram has precision limits due to bucketing
        // Expected 100_000 but may round to ~100_031 (0.03% error)
        let expected = 100_000;
        let tolerance = 100; // 0.1% tolerance
        assert!(
            stats.p50 >= expected && stats.p50 <= expected + tolerance,
            "p50 should be ~{} but was {}",
            expected,
            stats.p50
        );
        assert!(
            stats.p99 >= expected && stats.p99 <= expected + tolerance,
            "p99 should be ~{} but was {}",
            expected,
            stats.p99
        );
    }

    #[test]
    fn test_percentile_tracker_reset() {
        let tracker = PercentileTracker::new();
        tracker.record_ms(100);
        assert!(tracker.stats().is_some());

        tracker.reset();
        assert!(tracker.stats().is_none());
    }

    #[test]
    fn test_multi_label_tracker() {
        let tracker = MultiLabelPercentileTracker::new();

        // Record for different endpoints
        tracker.record("/api/users", 10);
        tracker.record("/api/users", 20);
        tracker.record("/api/products", 30);

        let user_stats = tracker.stats("/api/users").unwrap();
        assert_eq!(user_stats.count, 2);

        let product_stats = tracker.stats("/api/products").unwrap();
        assert_eq!(product_stats.count, 1);

        assert!(tracker.stats("/api/missing").is_none());
    }

    #[test]
    fn test_multi_label_all_stats() {
        let tracker = MultiLabelPercentileTracker::new();

        tracker.record("endpoint1", 10);
        tracker.record("endpoint2", 20);

        let all = tracker.all_stats();
        assert_eq!(all.len(), 2);
        assert!(all.contains_key("endpoint1"));
        assert!(all.contains_key("endpoint2"));
    }

    #[test]
    fn test_multi_label_labels() {
        let tracker = MultiLabelPercentileTracker::new();

        tracker.record("a", 10);
        tracker.record("b", 20);
        tracker.record("c", 30);

        let mut labels = tracker.labels();
        labels.sort();
        assert_eq!(labels, vec!["a", "b", "c"]);
    }

    #[test]
    fn test_percentile_stats_format() {
        let stats = PercentileStats {
            count: 100,
            min: 1_000,     // 1ms
            max: 100_000,   // 100ms
            mean: 50_000.0, // 50ms
            p50: 50_000,    // 50ms
            p90: 90_000,    // 90ms
            p95: 95_000,    // 95ms
            p99: 99_000,    // 99ms
            p99_9: 99_900,  // 99.9ms
        };

        let formatted = stats.format();
        assert!(formatted.contains("count=100"));
        assert!(formatted.contains("p50=50.00ms"));
        assert!(formatted.contains("p99=99.00ms"));
    }

    #[test]
    fn test_format_percentile_table() {
        let mut stats_map = HashMap::new();
        stats_map.insert(
            "endpoint1".to_string(),
            PercentileStats {
                count: 100,
                min: 10_000,
                max: 100_000,
                mean: 50_000.0,
                p50: 50_000,
                p90: 90_000,
                p95: 95_000,
                p99: 99_000,
                p99_9: 99_900,
            },
        );

        let table = format_percentile_table("Test Table", &stats_map);
        assert!(table.contains("Test Table"));
        assert!(table.contains("endpoint1"));
        assert!(table.contains("P50"));
    }

    #[test]
    fn test_format_percentile_table_empty() {
        let stats_map = HashMap::new();
        let table = format_percentile_table("Empty Table", &stats_map);
        assert!(table.contains("No data available"));
    }
}
