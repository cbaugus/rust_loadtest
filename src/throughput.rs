//! Per-scenario throughput tracking and reporting.
//!
//! This module provides throughput calculation and reporting for scenarios.
//! It tracks requests per second (RPS) for each scenario type, enabling
//! performance analysis and comparison across different scenario types.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tracing::debug;

/// Throughput statistics for a scenario.
#[derive(Debug, Clone)]
pub struct ThroughputStats {
    /// Scenario name
    pub scenario_name: String,

    /// Total requests/executions
    pub total_count: u64,

    /// Duration over which requests were counted
    pub duration: Duration,

    /// Calculated throughput (requests per second)
    pub rps: f64,

    /// Average time per request (milliseconds)
    pub avg_time_ms: f64,
}

impl ThroughputStats {
    /// Format throughput statistics as a human-readable string.
    pub fn format(&self) -> String {
        format!(
            "{}: {} requests in {:.1}s = {:.2} RPS (avg {:.2}ms per request)",
            self.scenario_name,
            self.total_count,
            self.duration.as_secs_f64(),
            self.rps,
            self.avg_time_ms
        )
    }

    /// Format as a table row.
    pub fn format_table_row(&self) -> String {
        format!(
            "{:<30} {:>10} {:>10.2} {:>10.2}",
            self.scenario_name, self.total_count, self.rps, self.avg_time_ms
        )
    }
}

/// Tracks throughput for multiple scenarios.
#[derive(Clone)]
pub struct ThroughputTracker {
    /// Start time of tracking
    start_time: Instant,

    /// Request counts per scenario
    counts: Arc<Mutex<HashMap<String, u64>>>,

    /// Total time spent per scenario (for avg calculation)
    total_times: Arc<Mutex<HashMap<String, Duration>>>,
}

impl ThroughputTracker {
    /// Create a new throughput tracker.
    pub fn new() -> Self {
        Self {
            start_time: Instant::now(),
            counts: Arc::new(Mutex::new(HashMap::new())),
            total_times: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Record a scenario execution.
    ///
    /// # Arguments
    /// * `scenario_name` - Name of the scenario
    /// * `duration` - Duration of the execution
    pub fn record(&self, scenario_name: &str, duration: Duration) {
        let mut counts = self.counts.lock().unwrap();
        *counts.entry(scenario_name.to_string()).or_insert(0) += 1;

        let mut times = self.total_times.lock().unwrap();
        *times
            .entry(scenario_name.to_string())
            .or_insert(Duration::ZERO) += duration;

        debug!(
            scenario = scenario_name,
            duration_ms = duration.as_millis(),
            "Recorded scenario execution"
        );
    }

    /// Get throughput statistics for a specific scenario.
    pub fn stats(&self, scenario_name: &str) -> Option<ThroughputStats> {
        let counts = self.counts.lock().unwrap();
        let times = self.total_times.lock().unwrap();

        let count = counts.get(scenario_name)?;
        let total_time = times.get(scenario_name)?;

        let duration = self.start_time.elapsed();
        let rps = if duration.as_secs_f64() > 0.0 {
            *count as f64 / duration.as_secs_f64()
        } else {
            0.0
        };

        let avg_time_ms = if *count > 0 {
            total_time.as_millis() as f64 / *count as f64
        } else {
            0.0
        };

        Some(ThroughputStats {
            scenario_name: scenario_name.to_string(),
            total_count: *count,
            duration,
            rps,
            avg_time_ms,
        })
    }

    /// Get statistics for all scenarios.
    pub fn all_stats(&self) -> Vec<ThroughputStats> {
        let counts = self.counts.lock().unwrap();
        let mut stats = Vec::new();

        for scenario_name in counts.keys() {
            if let Some(stat) = self.stats(scenario_name) {
                stats.push(stat);
            }
        }

        // Sort by scenario name for consistent output
        stats.sort_by(|a, b| a.scenario_name.cmp(&b.scenario_name));
        stats
    }

    /// Get total throughput across all scenarios.
    pub fn total_throughput(&self) -> f64 {
        let counts = self.counts.lock().unwrap();
        let total: u64 = counts.values().sum();
        let duration = self.start_time.elapsed();

        if duration.as_secs_f64() > 0.0 {
            total as f64 / duration.as_secs_f64()
        } else {
            0.0
        }
    }

    /// Reset all tracking data.
    pub fn reset(&self) {
        let mut counts = self.counts.lock().unwrap();
        let mut times = self.total_times.lock().unwrap();
        counts.clear();
        times.clear();
    }

    /// Get the elapsed time since tracking started.
    pub fn elapsed(&self) -> Duration {
        self.start_time.elapsed()
    }
}

impl Default for ThroughputTracker {
    fn default() -> Self {
        Self::new()
    }
}

/// Format throughput statistics as a table.
pub fn format_throughput_table(stats: &[ThroughputStats]) -> String {
    if stats.is_empty() {
        return "No throughput data available.\n".to_string();
    }

    let mut output = String::new();
    output.push_str(&format!(
        "\n{:<30} {:>10} {:>10} {:>10}\n",
        "Scenario", "Requests", "RPS", "Avg Time"
    ));
    output.push_str(&format!(
        "{:<30} {:>10} {:>10} {:>10}\n",
        "", "", "", "(ms)"
    ));
    output.push_str(&"-".repeat(70));
    output.push('\n');

    for stat in stats {
        output.push_str(&stat.format_table_row());
        output.push('\n');
    }

    output
}

/// Global throughput tracker.
lazy_static::lazy_static! {
    pub static ref GLOBAL_THROUGHPUT_TRACKER: ThroughputTracker = ThroughputTracker::new();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_throughput_tracker() {
        let tracker = ThroughputTracker::new();

        tracker.record("scenario1", Duration::from_millis(100));
        tracker.record("scenario1", Duration::from_millis(150));
        tracker.record("scenario2", Duration::from_millis(200));

        let stats1 = tracker.stats("scenario1").unwrap();
        assert_eq!(stats1.total_count, 2);
        assert_eq!(stats1.avg_time_ms, 125.0); // (100 + 150) / 2

        let stats2 = tracker.stats("scenario2").unwrap();
        assert_eq!(stats2.total_count, 1);
        assert_eq!(stats2.avg_time_ms, 200.0);
    }

    #[test]
    fn test_all_stats() {
        let tracker = ThroughputTracker::new();

        tracker.record("alpha", Duration::from_millis(100));
        tracker.record("beta", Duration::from_millis(200));
        tracker.record("gamma", Duration::from_millis(300));

        let all_stats = tracker.all_stats();
        assert_eq!(all_stats.len(), 3);

        // Should be sorted by name
        assert_eq!(all_stats[0].scenario_name, "alpha");
        assert_eq!(all_stats[1].scenario_name, "beta");
        assert_eq!(all_stats[2].scenario_name, "gamma");
    }

    #[test]
    fn test_total_throughput() {
        let tracker = ThroughputTracker::new();

        // Record some scenarios
        for _ in 0..10 {
            tracker.record("test", Duration::from_millis(100));
        }

        // Give it a moment to calculate
        std::thread::sleep(Duration::from_millis(100));

        let total_rps = tracker.total_throughput();
        assert!(total_rps > 0.0, "Total RPS should be greater than 0");
    }

    #[test]
    fn test_stats_format() {
        let stats = ThroughputStats {
            scenario_name: "Test Scenario".to_string(),
            total_count: 100,
            duration: Duration::from_secs(10),
            rps: 10.0,
            avg_time_ms: 50.0,
        };

        let formatted = stats.format();
        assert!(formatted.contains("Test Scenario"));
        assert!(formatted.contains("100 requests"));
        assert!(formatted.contains("10.0"));
    }

    #[test]
    fn test_reset() {
        let tracker = ThroughputTracker::new();

        tracker.record("test", Duration::from_millis(100));
        assert!(tracker.stats("test").is_some());

        tracker.reset();
        assert!(tracker.stats("test").is_none());
    }

    #[test]
    fn test_format_throughput_table() {
        let stats = vec![
            ThroughputStats {
                scenario_name: "Scenario A".to_string(),
                total_count: 100,
                duration: Duration::from_secs(10),
                rps: 10.0,
                avg_time_ms: 50.0,
            },
            ThroughputStats {
                scenario_name: "Scenario B".to_string(),
                total_count: 200,
                duration: Duration::from_secs(10),
                rps: 20.0,
                avg_time_ms: 25.0,
            },
        ];

        let table = format_throughput_table(&stats);
        assert!(table.contains("Scenario"));
        assert!(table.contains("Requests"));
        assert!(table.contains("RPS"));
    }

    #[test]
    fn test_empty_stats() {
        let tracker = ThroughputTracker::new();
        assert!(tracker.stats("nonexistent").is_none());
    }
}
