//! Multi-scenario execution with weighted distribution (Issue #43).
//!
//! This module provides functionality for running multiple scenarios concurrently
//! with weighted traffic distribution, per-scenario metrics, and round-robin
//! distribution across workers.

use crate::scenario::Scenario;
use rand::Rng;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

/// Scenario selector that chooses scenarios based on weighted distribution.
///
/// Uses weighted random selection where each scenario's weight determines
/// its selection probability.
///
/// # Example
/// ```
/// use rust_loadtest::multi_scenario::ScenarioSelector;
/// use rust_loadtest::scenario::Scenario;
///
/// let scenarios = vec![
///     Scenario { name: "Read".to_string(), weight: 80.0, steps: vec![] },
///     Scenario { name: "Write".to_string(), weight: 20.0, steps: vec![] },
/// ];
///
/// let selector = ScenarioSelector::new(scenarios);
/// let scenario = selector.select();
/// // 80% chance of "Read", 20% chance of "Write"
/// ```
#[derive(Clone)]
pub struct ScenarioSelector {
    scenarios: Arc<Vec<Scenario>>,
    cumulative_weights: Arc<Vec<f64>>,
    total_weight: f64,
}

impl ScenarioSelector {
    /// Create a new scenario selector with weighted scenarios.
    ///
    /// # Arguments
    /// * `scenarios` - List of scenarios with weights
    ///
    /// # Panics
    /// Panics if scenarios list is empty or if any weight is negative.
    pub fn new(scenarios: Vec<Scenario>) -> Self {
        if scenarios.is_empty() {
            panic!("Cannot create ScenarioSelector with empty scenarios list");
        }

        // Validate weights
        for scenario in &scenarios {
            if scenario.weight < 0.0 {
                panic!(
                    "Scenario '{}' has negative weight: {}",
                    scenario.name, scenario.weight
                );
            }
            if scenario.weight == 0.0 {
                panic!(
                    "Scenario '{}' has zero weight. Remove scenarios with zero weight.",
                    scenario.name
                );
            }
        }

        // Calculate cumulative weights for weighted random selection
        let mut cumulative = Vec::with_capacity(scenarios.len());
        let mut sum = 0.0;

        for scenario in &scenarios {
            sum += scenario.weight;
            cumulative.push(sum);
        }

        Self {
            scenarios: Arc::new(scenarios),
            cumulative_weights: Arc::new(cumulative),
            total_weight: sum,
        }
    }

    /// Select a scenario based on weighted random distribution.
    ///
    /// Uses cumulative weight distribution for O(log n) selection.
    pub fn select(&self) -> &Scenario {
        let mut rng = rand::thread_rng();
        let random = rng.gen_range(0.0..self.total_weight);

        // Binary search for the selected scenario
        let index = self
            .cumulative_weights
            .binary_search_by(|weight| {
                if *weight <= random {
                    std::cmp::Ordering::Less
                } else {
                    std::cmp::Ordering::Greater
                }
            })
            .unwrap_or_else(|i| i);

        &self.scenarios[index]
    }

    /// Get scenario by index.
    pub fn get_scenario(&self, index: usize) -> Option<&Scenario> {
        self.scenarios.get(index)
    }

    /// Get total number of scenarios.
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }

    /// Get all scenarios.
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }

    /// Get the total weight of all scenarios.
    pub fn total_weight(&self) -> f64 {
        self.total_weight
    }

    /// Calculate the selection probability for each scenario.
    pub fn probabilities(&self) -> Vec<(String, f64)> {
        self.scenarios
            .iter()
            .map(|s| {
                let probability = s.weight / self.total_weight;
                (s.name.clone(), probability)
            })
            .collect()
    }
}

/// Round-robin scenario distributor.
///
/// Distributes scenarios evenly across workers in a round-robin fashion.
/// Each worker gets the next scenario in sequence, cycling through all scenarios.
///
/// # Example
/// ```
/// use rust_loadtest::multi_scenario::RoundRobinDistributor;
/// use rust_loadtest::scenario::Scenario;
///
/// let scenarios = vec![
///     Scenario { name: "S1".to_string(), weight: 1.0, steps: vec![] },
///     Scenario { name: "S2".to_string(), weight: 1.0, steps: vec![] },
/// ];
///
/// let distributor = RoundRobinDistributor::new(scenarios);
/// let s1 = distributor.next(); // Returns S1
/// let s2 = distributor.next(); // Returns S2
/// let s3 = distributor.next(); // Returns S1 (cycles back)
/// ```
pub struct RoundRobinDistributor {
    scenarios: Arc<Vec<Scenario>>,
    counter: AtomicU64,
}

impl RoundRobinDistributor {
    /// Create a new round-robin distributor.
    pub fn new(scenarios: Vec<Scenario>) -> Self {
        if scenarios.is_empty() {
            panic!("Cannot create RoundRobinDistributor with empty scenarios list");
        }

        Self {
            scenarios: Arc::new(scenarios),
            counter: AtomicU64::new(0),
        }
    }

    /// Get the next scenario in round-robin order.
    pub fn next(&self) -> &Scenario {
        let index = self.counter.fetch_add(1, Ordering::Relaxed) as usize;
        &self.scenarios[index % self.scenarios.len()]
    }

    /// Get scenario by index.
    pub fn get_scenario(&self, index: usize) -> Option<&Scenario> {
        self.scenarios.get(index)
    }

    /// Get total number of scenarios.
    pub fn scenario_count(&self) -> usize {
        self.scenarios.len()
    }

    /// Get all scenarios.
    pub fn scenarios(&self) -> &[Scenario] {
        &self.scenarios
    }
}

/// Per-scenario metrics tracker.
///
/// Tracks execution counts, success/failure rates, and timing metrics
/// for each scenario independently.
#[derive(Default)]
pub struct ScenarioMetrics {
    /// Total executions per scenario
    executions: HashMap<String, AtomicU64>,

    /// Successful executions per scenario
    successes: HashMap<String, AtomicU64>,

    /// Failed executions per scenario
    failures: HashMap<String, AtomicU64>,

    /// Total execution time in milliseconds per scenario
    total_time_ms: HashMap<String, AtomicU64>,
}

impl ScenarioMetrics {
    /// Create a new scenario metrics tracker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Initialize metrics for a list of scenarios.
    pub fn initialize_scenarios(&mut self, scenarios: &[Scenario]) {
        for scenario in scenarios {
            self.executions
                .insert(scenario.name.clone(), AtomicU64::new(0));
            self.successes
                .insert(scenario.name.clone(), AtomicU64::new(0));
            self.failures
                .insert(scenario.name.clone(), AtomicU64::new(0));
            self.total_time_ms
                .insert(scenario.name.clone(), AtomicU64::new(0));
        }
    }

    /// Record a scenario execution.
    pub fn record_execution(&self, scenario_name: &str, success: bool, duration_ms: u64) {
        if let Some(counter) = self.executions.get(scenario_name) {
            counter.fetch_add(1, Ordering::Relaxed);
        }

        if success {
            if let Some(counter) = self.successes.get(scenario_name) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        } else {
            if let Some(counter) = self.failures.get(scenario_name) {
                counter.fetch_add(1, Ordering::Relaxed);
            }
        }

        if let Some(counter) = self.total_time_ms.get(scenario_name) {
            counter.fetch_add(duration_ms, Ordering::Relaxed);
        }
    }

    /// Get execution count for a scenario.
    pub fn get_executions(&self, scenario_name: &str) -> u64 {
        self.executions
            .get(scenario_name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get success count for a scenario.
    pub fn get_successes(&self, scenario_name: &str) -> u64 {
        self.successes
            .get(scenario_name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get failure count for a scenario.
    pub fn get_failures(&self, scenario_name: &str) -> u64 {
        self.failures
            .get(scenario_name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get total execution time for a scenario.
    pub fn get_total_time_ms(&self, scenario_name: &str) -> u64 {
        self.total_time_ms
            .get(scenario_name)
            .map(|c| c.load(Ordering::Relaxed))
            .unwrap_or(0)
    }

    /// Get average execution time for a scenario.
    pub fn get_average_time_ms(&self, scenario_name: &str) -> f64 {
        let total = self.get_total_time_ms(scenario_name);
        let executions = self.get_executions(scenario_name);

        if executions == 0 {
            0.0
        } else {
            total as f64 / executions as f64
        }
    }

    /// Get success rate for a scenario (0.0 to 1.0).
    pub fn get_success_rate(&self, scenario_name: &str) -> f64 {
        let successes = self.get_successes(scenario_name);
        let executions = self.get_executions(scenario_name);

        if executions == 0 {
            0.0
        } else {
            successes as f64 / executions as f64
        }
    }

    /// Get all scenario names.
    pub fn scenario_names(&self) -> Vec<String> {
        self.executions.keys().cloned().collect()
    }

    /// Get summary for all scenarios.
    pub fn summary(&self) -> ScenarioMetricsSummary {
        let mut summaries = Vec::new();

        for name in self.scenario_names() {
            summaries.push(ScenarioSummary {
                name: name.clone(),
                executions: self.get_executions(&name),
                successes: self.get_successes(&name),
                failures: self.get_failures(&name),
                success_rate: self.get_success_rate(&name),
                average_time_ms: self.get_average_time_ms(&name),
            });
        }

        ScenarioMetricsSummary {
            scenarios: summaries,
        }
    }
}

/// Summary of metrics for a single scenario.
#[derive(Debug, Clone)]
pub struct ScenarioSummary {
    pub name: String,
    pub executions: u64,
    pub successes: u64,
    pub failures: u64,
    pub success_rate: f64,
    pub average_time_ms: f64,
}

/// Summary of metrics for all scenarios.
#[derive(Debug, Clone)]
pub struct ScenarioMetricsSummary {
    pub scenarios: Vec<ScenarioSummary>,
}

impl ScenarioMetricsSummary {
    /// Print a formatted summary to stdout.
    pub fn print(&self) {
        println!("\n=== Per-Scenario Metrics ===\n");

        for summary in &self.scenarios {
            println!("Scenario: {}", summary.name);
            println!("  Executions: {}", summary.executions);
            println!(
                "  Successes:  {} ({:.1}%)",
                summary.successes,
                summary.success_rate * 100.0
            );
            println!("  Failures:   {}", summary.failures);
            println!("  Avg Time:   {:.2}ms", summary.average_time_ms);
            println!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_scenarios() -> Vec<Scenario> {
        vec![
            Scenario {
                name: "Read".to_string(),
                weight: 80.0,
                steps: vec![],
            },
            Scenario {
                name: "Write".to_string(),
                weight: 15.0,
                steps: vec![],
            },
            Scenario {
                name: "Delete".to_string(),
                weight: 5.0,
                steps: vec![],
            },
        ]
    }

    #[test]
    fn test_scenario_selector_creation() {
        let scenarios = create_test_scenarios();
        let selector = ScenarioSelector::new(scenarios);

        assert_eq!(selector.scenario_count(), 3);
        assert_eq!(selector.total_weight(), 100.0);

        println!("✅ ScenarioSelector creation works");
    }

    #[test]
    fn test_scenario_selector_probabilities() {
        let scenarios = create_test_scenarios();
        let selector = ScenarioSelector::new(scenarios);

        let probs = selector.probabilities();
        assert_eq!(probs.len(), 3);

        // Check probabilities
        assert!((probs[0].1 - 0.80).abs() < 0.001); // 80%
        assert!((probs[1].1 - 0.15).abs() < 0.001); // 15%
        assert!((probs[2].1 - 0.05).abs() < 0.001); // 5%

        println!("✅ ScenarioSelector probabilities are correct");
    }

    #[test]
    fn test_scenario_selector_distribution() {
        let scenarios = create_test_scenarios();
        let selector = ScenarioSelector::new(scenarios);

        // Select many times and check distribution
        let mut counts = HashMap::new();
        let iterations = 10000;

        for _ in 0..iterations {
            let scenario = selector.select();
            *counts.entry(scenario.name.clone()).or_insert(0) += 1;
        }

        // Check that distribution is roughly correct (within 5%)
        let read_pct = *counts.get("Read").unwrap() as f64 / iterations as f64;
        let write_pct = *counts.get("Write").unwrap() as f64 / iterations as f64;
        let delete_pct = *counts.get("Delete").unwrap() as f64 / iterations as f64;

        assert!((read_pct - 0.80).abs() < 0.05);
        assert!((write_pct - 0.15).abs() < 0.05);
        assert!((delete_pct - 0.05).abs() < 0.05);

        println!("✅ ScenarioSelector weighted distribution works");
        println!(
            "   Read: {:.1}%, Write: {:.1}%, Delete: {:.1}%",
            read_pct * 100.0,
            write_pct * 100.0,
            delete_pct * 100.0
        );
    }

    #[test]
    #[should_panic(expected = "empty scenarios list")]
    fn test_scenario_selector_empty_panics() {
        ScenarioSelector::new(vec![]);
    }

    #[test]
    #[should_panic(expected = "negative weight")]
    fn test_scenario_selector_negative_weight_panics() {
        let scenarios = vec![Scenario {
            name: "Test".to_string(),
            weight: -1.0,
            steps: vec![],
        }];
        ScenarioSelector::new(scenarios);
    }

    #[test]
    fn test_round_robin_distributor() {
        let scenarios = create_test_scenarios();
        let distributor = RoundRobinDistributor::new(scenarios);

        assert_eq!(distributor.scenario_count(), 3);

        // Get scenarios in round-robin order
        let s1 = distributor.next();
        let s2 = distributor.next();
        let s3 = distributor.next();
        let s4 = distributor.next(); // Should cycle back to first

        assert_eq!(s1.name, "Read");
        assert_eq!(s2.name, "Write");
        assert_eq!(s3.name, "Delete");
        assert_eq!(s4.name, "Read"); // Cycled back

        println!("✅ RoundRobinDistributor works");
    }

    #[test]
    fn test_scenario_metrics() {
        let scenarios = create_test_scenarios();
        let mut metrics = ScenarioMetrics::new();
        metrics.initialize_scenarios(&scenarios);

        // Record some executions
        metrics.record_execution("Read", true, 100);
        metrics.record_execution("Read", true, 200);
        metrics.record_execution("Read", false, 150);
        metrics.record_execution("Write", true, 300);

        // Check metrics
        assert_eq!(metrics.get_executions("Read"), 3);
        assert_eq!(metrics.get_successes("Read"), 2);
        assert_eq!(metrics.get_failures("Read"), 1);
        assert_eq!(metrics.get_total_time_ms("Read"), 450);
        assert_eq!(metrics.get_average_time_ms("Read"), 150.0);
        assert!((metrics.get_success_rate("Read") - 0.666).abs() < 0.01);

        assert_eq!(metrics.get_executions("Write"), 1);
        assert_eq!(metrics.get_successes("Write"), 1);

        println!("✅ ScenarioMetrics tracking works");
    }

    #[test]
    fn test_scenario_metrics_summary() {
        let scenarios = create_test_scenarios();
        let mut metrics = ScenarioMetrics::new();
        metrics.initialize_scenarios(&scenarios);

        metrics.record_execution("Read", true, 100);
        metrics.record_execution("Write", true, 200);
        metrics.record_execution("Delete", false, 150);

        let summary = metrics.summary();
        assert_eq!(summary.scenarios.len(), 3);

        println!("✅ ScenarioMetrics summary generation works");
    }
}
