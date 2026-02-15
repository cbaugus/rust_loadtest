//! Integration tests for multi-scenario execution (Issue #43).
//!
//! These tests validate:
//! - Weighted scenario selection
//! - Round-robin distribution
//! - Per-scenario metrics tracking
//! - Multi-scenario YAML loading

use rust_loadtest::multi_scenario::{
    RoundRobinDistributor, ScenarioMetrics, ScenarioSelector,
};
use rust_loadtest::scenario::Scenario;
use rust_loadtest::yaml_config::YamlConfig;
use std::collections::HashMap;

fn create_test_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "Read Operations".to_string(),
            weight: 80.0,
            steps: vec![],
        },
        Scenario {
            name: "Write Operations".to_string(),
            weight: 15.0,
            steps: vec![],
        },
        Scenario {
            name: "Delete Operations".to_string(),
            weight: 5.0,
            steps: vec![],
        },
    ]
}

#[test]
fn test_scenario_selector_basic() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios);

    assert_eq!(selector.scenario_count(), 3);
    assert_eq!(selector.total_weight(), 100.0);

    println!("✅ ScenarioSelector basic functionality works");
}

#[test]
fn test_scenario_selector_single_selection() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios);

    let selected = selector.select();
    assert!(
        selected.name == "Read Operations"
            || selected.name == "Write Operations"
            || selected.name == "Delete Operations"
    );

    println!("✅ ScenarioSelector can select a scenario");
}

#[test]
fn test_scenario_selector_weighted_distribution() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios);

    let iterations = 10000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let scenario = selector.select();
        *counts.entry(scenario.name.clone()).or_insert(0) += 1;
    }

    let read_count = counts.get("Read Operations").unwrap();
    let write_count = counts.get("Write Operations").unwrap();
    let delete_count = counts.get("Delete Operations").unwrap();

    // Calculate percentages
    let read_pct = *read_count as f64 / iterations as f64;
    let write_pct = *write_count as f64 / iterations as f64;
    let delete_pct = *delete_count as f64 / iterations as f64;

    // Check within 5% margin
    assert!(
        (read_pct - 0.80).abs() < 0.05,
        "Read: expected ~80%, got {:.1}%",
        read_pct * 100.0
    );
    assert!(
        (write_pct - 0.15).abs() < 0.05,
        "Write: expected ~15%, got {:.1}%",
        write_pct * 100.0
    );
    assert!(
        (delete_pct - 0.05).abs() < 0.05,
        "Delete: expected ~5%, got {:.1}%",
        delete_pct * 100.0
    );

    println!("✅ Weighted distribution is correct:");
    println!("   Read: {:.1}% (expected 80%)", read_pct * 100.0);
    println!("   Write: {:.1}% (expected 15%)", write_pct * 100.0);
    println!("   Delete: {:.1}% (expected 5%)", delete_pct * 100.0);
}

#[test]
fn test_scenario_selector_probabilities() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios);

    let probs = selector.probabilities();

    assert_eq!(probs.len(), 3);
    assert_eq!(probs[0].0, "Read Operations");
    assert!((probs[0].1 - 0.80).abs() < 0.001);
    assert_eq!(probs[1].0, "Write Operations");
    assert!((probs[1].1 - 0.15).abs() < 0.001);
    assert_eq!(probs[2].0, "Delete Operations");
    assert!((probs[2].1 - 0.05).abs() < 0.001);

    println!("✅ Probability calculation works");
}

#[test]
fn test_scenario_selector_equal_weights() {
    let scenarios = vec![
        Scenario {
            name: "S1".to_string(),
            weight: 1.0,
            steps: vec![],
        },
        Scenario {
            name: "S2".to_string(),
            weight: 1.0,
            steps: vec![],
        },
        Scenario {
            name: "S3".to_string(),
            weight: 1.0,
            steps: vec![],
        },
    ];

    let selector = ScenarioSelector::new(scenarios);

    let iterations = 9000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let scenario = selector.select();
        *counts.entry(scenario.name.clone()).or_insert(0) += 1;
    }

    // Each should be ~33% (within 5%)
    for (name, count) in &counts {
        let pct = *count as f64 / iterations as f64;
        assert!(
            (pct - 0.333).abs() < 0.05,
            "{}: expected ~33%, got {:.1}%",
            name,
            pct * 100.0
        );
    }

    println!("✅ Equal weight distribution works");
}

#[test]
fn test_scenario_selector_extreme_weights() {
    let scenarios = vec![
        Scenario {
            name: "Dominant".to_string(),
            weight: 99.0,
            steps: vec![],
        },
        Scenario {
            name: "Rare".to_string(),
            weight: 1.0,
            steps: vec![],
        },
    ];

    let selector = ScenarioSelector::new(scenarios);

    let iterations = 10000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let scenario = selector.select();
        *counts.entry(scenario.name.clone()).or_insert(0) += 1;
    }

    let dominant_pct = *counts.get("Dominant").unwrap() as f64 / iterations as f64;
    let rare_pct = *counts.get("Rare").unwrap() as f64 / iterations as f64;

    assert!((dominant_pct - 0.99).abs() < 0.02);
    assert!((rare_pct - 0.01).abs() < 0.02);

    println!("✅ Extreme weight distribution works (99:1)");
}

#[test]
#[should_panic(expected = "empty scenarios list")]
fn test_scenario_selector_empty_list() {
    ScenarioSelector::new(vec![]);
}

#[test]
#[should_panic(expected = "negative weight")]
fn test_scenario_selector_negative_weight() {
    let scenarios = vec![Scenario {
        name: "Invalid".to_string(),
        weight: -5.0,
        steps: vec![],
    }];
    ScenarioSelector::new(scenarios);
}

#[test]
#[should_panic(expected = "zero weight")]
fn test_scenario_selector_zero_weight() {
    let scenarios = vec![Scenario {
        name: "Invalid".to_string(),
        weight: 0.0,
        steps: vec![],
    }];
    ScenarioSelector::new(scenarios);
}

#[test]
fn test_round_robin_distributor_basic() {
    let scenarios = create_test_scenarios();
    let distributor = RoundRobinDistributor::new(scenarios);

    assert_eq!(distributor.scenario_count(), 3);

    println!("✅ RoundRobinDistributor basic functionality works");
}

#[test]
fn test_round_robin_distributor_sequence() {
    let scenarios = create_test_scenarios();
    let distributor = RoundRobinDistributor::new(scenarios);

    let s1 = distributor.next();
    let s2 = distributor.next();
    let s3 = distributor.next();
    let s4 = distributor.next();
    let s5 = distributor.next();
    let s6 = distributor.next();

    assert_eq!(s1.name, "Read Operations");
    assert_eq!(s2.name, "Write Operations");
    assert_eq!(s3.name, "Delete Operations");
    assert_eq!(s4.name, "Read Operations"); // Cycle
    assert_eq!(s5.name, "Write Operations");
    assert_eq!(s6.name, "Delete Operations");

    println!("✅ RoundRobinDistributor cycles through scenarios correctly");
}

#[test]
fn test_round_robin_distributor_even_distribution() {
    let scenarios = create_test_scenarios();
    let distributor = RoundRobinDistributor::new(scenarios);

    let iterations = 9000; // Multiple of 3
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let scenario = distributor.next();
        *counts.entry(scenario.name.clone()).or_insert(0) += 1;
    }

    // Each should get exactly 3000 iterations (33.33%)
    assert_eq!(*counts.get("Read Operations").unwrap(), 3000);
    assert_eq!(*counts.get("Write Operations").unwrap(), 3000);
    assert_eq!(*counts.get("Delete Operations").unwrap(), 3000);

    println!("✅ RoundRobinDistributor provides even distribution");
}

#[test]
#[should_panic(expected = "empty scenarios list")]
fn test_round_robin_distributor_empty_list() {
    RoundRobinDistributor::new(vec![]);
}

#[test]
fn test_scenario_metrics_initialization() {
    let scenarios = create_test_scenarios();
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    for scenario in &scenarios {
        assert_eq!(metrics.get_executions(&scenario.name), 0);
        assert_eq!(metrics.get_successes(&scenario.name), 0);
        assert_eq!(metrics.get_failures(&scenario.name), 0);
    }

    println!("✅ ScenarioMetrics initialization works");
}

#[test]
fn test_scenario_metrics_recording() {
    let scenarios = create_test_scenarios();
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    metrics.record_execution("Read Operations", true, 100);
    metrics.record_execution("Read Operations", true, 200);
    metrics.record_execution("Read Operations", false, 150);

    assert_eq!(metrics.get_executions("Read Operations"), 3);
    assert_eq!(metrics.get_successes("Read Operations"), 2);
    assert_eq!(metrics.get_failures("Read Operations"), 1);
    assert_eq!(metrics.get_total_time_ms("Read Operations"), 450);

    println!("✅ ScenarioMetrics recording works");
}

#[test]
fn test_scenario_metrics_calculations() {
    let scenarios = create_test_scenarios();
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    metrics.record_execution("Write Operations", true, 100);
    metrics.record_execution("Write Operations", true, 200);
    metrics.record_execution("Write Operations", true, 300);
    metrics.record_execution("Write Operations", false, 400);

    assert_eq!(metrics.get_average_time_ms("Write Operations"), 250.0);
    assert_eq!(metrics.get_success_rate("Write Operations"), 0.75);

    println!("✅ ScenarioMetrics calculations (average, success rate) work");
}

#[test]
fn test_scenario_metrics_summary() {
    let scenarios = create_test_scenarios();
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    metrics.record_execution("Read Operations", true, 100);
    metrics.record_execution("Write Operations", true, 200);
    metrics.record_execution("Delete Operations", false, 150);

    let summary = metrics.summary();
    assert_eq!(summary.scenarios.len(), 3);

    // Find each scenario in summary
    let read_summary = summary
        .scenarios
        .iter()
        .find(|s| s.name == "Read Operations")
        .unwrap();
    assert_eq!(read_summary.executions, 1);
    assert_eq!(read_summary.successes, 1);
    assert_eq!(read_summary.average_time_ms, 100.0);

    println!("✅ ScenarioMetrics summary generation works");
}

#[test]
fn test_scenario_metrics_zero_executions() {
    let scenarios = create_test_scenarios();
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    // Don't record any executions
    assert_eq!(metrics.get_average_time_ms("Read Operations"), 0.0);
    assert_eq!(metrics.get_success_rate("Read Operations"), 0.0);

    println!("✅ ScenarioMetrics handles zero executions correctly");
}

#[test]
fn test_yaml_multiple_scenarios_loading() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Read API"
    weight: 70
    steps:
      - request:
          method: "GET"
          path: "/api/read"

  - name: "Write API"
    weight: 20
    steps:
      - request:
          method: "POST"
          path: "/api/write"

  - name: "Delete API"
    weight: 10
    steps:
      - request:
          method: "DELETE"
          path: "/api/delete"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 3);
    assert_eq!(scenarios[0].name, "Read API");
    assert_eq!(scenarios[0].weight, 70.0);
    assert_eq!(scenarios[1].name, "Write API");
    assert_eq!(scenarios[1].weight, 20.0);
    assert_eq!(scenarios[2].name, "Delete API");
    assert_eq!(scenarios[2].weight, 10.0);

    println!("✅ YAML loading of multiple weighted scenarios works");
}

#[test]
fn test_yaml_scenarios_with_selector() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Heavy"
    weight: 80
    steps:
      - request:
          method: "GET"
          path: "/heavy"

  - name: "Light"
    weight: 20
    steps:
      - request:
          method: "GET"
          path: "/light"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    let selector = ScenarioSelector::new(scenarios);

    let iterations = 10000;
    let mut counts: HashMap<String, usize> = HashMap::new();

    for _ in 0..iterations {
        let scenario = selector.select();
        *counts.entry(scenario.name.clone()).or_insert(0) += 1;
    }

    let heavy_pct = *counts.get("Heavy").unwrap() as f64 / iterations as f64;
    let light_pct = *counts.get("Light").unwrap() as f64 / iterations as f64;

    assert!((heavy_pct - 0.80).abs() < 0.05);
    assert!((light_pct - 0.20).abs() < 0.05);

    println!("✅ YAML-loaded scenarios work with ScenarioSelector");
}

#[test]
fn test_integration_selector_with_metrics() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios.clone());
    let mut metrics = ScenarioMetrics::new();
    metrics.initialize_scenarios(&scenarios);

    // Simulate 100 scenario executions
    for _ in 0..100 {
        let scenario = selector.select();
        let success = rand::random::<bool>();
        let duration_ms = rand::random::<u64>() % 1000;
        metrics.record_execution(&scenario.name, success, duration_ms);
    }

    let summary = metrics.summary();
    let total_executions: u64 = summary.scenarios.iter().map(|s| s.executions).sum();
    assert_eq!(total_executions, 100);

    println!("✅ Integration: Selector + Metrics works");
}

#[test]
fn test_scenario_selector_get_methods() {
    let scenarios = create_test_scenarios();
    let selector = ScenarioSelector::new(scenarios);

    assert!(selector.get_scenario(0).is_some());
    assert!(selector.get_scenario(1).is_some());
    assert!(selector.get_scenario(2).is_some());
    assert!(selector.get_scenario(3).is_none());

    let all_scenarios = selector.scenarios();
    assert_eq!(all_scenarios.len(), 3);

    println!("✅ ScenarioSelector get methods work");
}

#[test]
fn test_round_robin_get_methods() {
    let scenarios = create_test_scenarios();
    let distributor = RoundRobinDistributor::new(scenarios);

    assert!(distributor.get_scenario(0).is_some());
    assert!(distributor.get_scenario(2).is_some());
    assert!(distributor.get_scenario(3).is_none());

    let all_scenarios = distributor.scenarios();
    assert_eq!(all_scenarios.len(), 3);

    println!("✅ RoundRobinDistributor get methods work");
}
