# Multi-Scenario Execution

## Overview

Multi-scenario execution enables running multiple user flows concurrently with weighted traffic distribution. This simulates realistic production environments where different user behaviors occur simultaneously.

## Key Features

✅ **Weighted selection** - Scenarios selected by probability based on weights
✅ **Round-robin distribution** - Even distribution across all scenarios
✅ **Per-scenario metrics** - Track performance for each scenario independently
✅ **YAML configuration** - Define multiple scenarios in one config file
✅ **Flexible allocation** - Choose distribution strategy per use case

## Weighted Scenario Selection

### How It Works

Each scenario has a weight that determines its selection probability:

```
probability = scenario_weight / sum(all_weights)
```

### Example Configuration

```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  workers: 50
  duration: "30m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Read Operations"
    weight: 80  # 80% of traffic
    steps:
      - request:
          method: "GET"
          path: "/api/data"

  - name: "Write Operations"
    weight: 15  # 15% of traffic
    steps:
      - request:
          method: "POST"
          path: "/api/data"
          body: '{"test": true}'

  - name: "Delete Operations"
    weight: 5   # 5% of traffic
    steps:
      - request:
          method: "DELETE"
          path: "/api/data/123"
```

**Result**: Out of 100 RPS:
- ~80 RPS execute "Read Operations"
- ~15 RPS execute "Write Operations"
- ~5 RPS execute "Delete Operations"

### Weight Calculation

Weights don't need to sum to 100. The system calculates percentages automatically:

```yaml
scenarios:
  - name: "API v1"
    weight: 3
  - name: "API v2"
    weight: 1
```

**Result**: 75% API v1, 25% API v2

## Round-Robin Distribution

Round-robin provides even distribution regardless of weights.

### When to Use

- **Load balancing** - Test all scenarios equally
- **Fair distribution** - Each scenario gets same traffic
- **Testing coverage** - Ensure all flows are exercised

### Programmatic Usage

```rust
use rust_loadtest::multi_scenario::RoundRobinDistributor;

let scenarios = vec![scenario1, scenario2, scenario3];
let distributor = RoundRobinDistributor::new(scenarios);

// Each call returns next scenario in sequence
let s1 = distributor.next(); // scenario1
let s2 = distributor.next(); // scenario2
let s3 = distributor.next(); // scenario3
let s4 = distributor.next(); // scenario1 (cycles)
```

## Scenario Selection Strategies

### Weighted Random (Default)

**Best for**: Simulating realistic production traffic patterns

```rust
use rust_loadtest::multi_scenario::ScenarioSelector;

let selector = ScenarioSelector::new(scenarios);

// Each call returns weighted random scenario
let scenario = selector.select();
```

**Characteristics**:
- Follows statistical distribution over time
- Realistic traffic simulation
- Some scenarios may not execute in short tests

### Round-Robin

**Best for**: Even coverage and load balancing

```rust
use rust_loadtest::multi_scenario::RoundRobinDistributor;

let distributor = RoundRobinDistributor::new(scenarios);

// Guaranteed sequential distribution
let scenario = distributor.next();
```

**Characteristics**:
- Deterministic order
- Equal distribution across scenarios
- All scenarios guaranteed to execute

## Per-Scenario Metrics

Track performance metrics independently for each scenario.

### Metrics Tracked

- **Executions** - Total number of times scenario ran
- **Successes** - Successful completions
- **Failures** - Failed executions
- **Success Rate** - Percentage of successful executions
- **Average Time** - Mean execution duration

### Usage

```rust
use rust_loadtest::multi_scenario::ScenarioMetrics;

let mut metrics = ScenarioMetrics::new();
metrics.initialize_scenarios(&scenarios);

// Record executions
metrics.record_execution("Read Operations", true, 120); // success, 120ms
metrics.record_execution("Write Operations", false, 450); // failure, 450ms

// Query metrics
let executions = metrics.get_executions("Read Operations");
let success_rate = metrics.get_success_rate("Read Operations");
let avg_time = metrics.get_average_time_ms("Read Operations");

// Get summary for all scenarios
let summary = metrics.summary();
summary.print();
```

### Sample Output

```
=== Per-Scenario Metrics ===

Scenario: Read Operations
  Executions: 8000
  Successes:  7950 (99.4%)
  Failures:   50
  Avg Time:   120.45ms

Scenario: Write Operations
  Executions: 1500
  Successes:  1480 (98.7%)
  Failures:   20
  Avg Time:   245.32ms

Scenario: Delete Operations
  Executions: 500
  Successes:  495 (99.0%)
  Failures:   5
  Avg Time:   98.21ms
```

## Real-World Examples

### E-Commerce Load Test

```yaml
version: "1.0"
metadata:
  name: "E-Commerce Load Test"
  description: "Realistic shopping behavior patterns"

config:
  baseUrl: "https://shop.example.com"
  workers: 100
  duration: "1h"

load:
  model: "ramp"
  min: 50
  max: 500
  rampDuration: "15m"

scenarios:
  # Most users browse without buying
  - name: "Browse Only"
    weight: 60
    steps:
      - request:
          method: "GET"
          path: "/"
      - request:
          method: "GET"
          path: "/products"

  # Some users add items but don't complete purchase
  - name: "Browse and Add to Cart"
    weight: 25
    steps:
      - request:
          method: "GET"
          path: "/products"
      - request:
          method: "POST"
          path: "/cart/add"

  # Fewer users complete full purchase
  - name: "Complete Purchase"
    weight: 12
    steps:
      - request:
          method: "GET"
          path: "/products"
      - request:
          method: "POST"
          path: "/cart/add"
      - request:
          method: "POST"
          path: "/checkout"

  # Rare admin operations
  - name: "Admin Operations"
    weight: 3
    steps:
      - request:
          method: "POST"
          path: "/admin/sync"
```

### API Versioning Test

```yaml
scenarios:
  # Gradual migration from v1 to v2
  - name: "API v1 (Legacy)"
    weight: 70
    steps:
      - request:
          method: "GET"
          path: "/v1/users"

  - name: "API v2 (New)"
    weight: 30
    steps:
      - request:
          method: "GET"
          path: "/v2/users"
```

### Read/Write Workload

```yaml
scenarios:
  - name: "Read Heavy"
    weight: 90
    steps:
      - request:
          method: "GET"
          path: "/api/data"

  - name: "Write Operations"
    weight: 10
    steps:
      - request:
          method: "POST"
          path: "/api/data"
```

## Worker Allocation

### Concurrent Model

Workers continuously pick scenarios based on selection strategy:

```yaml
load:
  model: "concurrent"
config:
  workers: 50  # Each worker picks scenarios independently
```

With weighted selection (80/15/5 split):
- ~40 workers execute Read Operations
- ~7 workers execute Write Operations
- ~3 workers execute Delete Operations

### RPS Model

Target RPS is distributed across scenarios by weight:

```yaml
load:
  model: "rps"
  target: 100  # Total 100 RPS across all scenarios
```

With weighted selection (80/15/5 split):
- ~80 RPS for Read Operations
- ~15 RPS for Write Operations
- ~5 RPS for Delete Operations

## Best Practices

### 1. Base Weights on Real Traffic

Analyze production traffic to set realistic weights:

```bash
# Example: Analyze access logs
$ cat access.log | awk '{print $7}' | sort | uniq -c | sort -rn

  80000 GET /api/data
  15000 POST /api/data
   5000 DELETE /api/data
```

**Configuration**:
```yaml
scenarios:
  - name: "Read"
    weight: 80  # Based on actual traffic
  - name: "Write"
    weight: 15
  - name: "Delete"
    weight: 5
```

### 2. Start with Equal Weights for Testing

Use equal weights initially to test all scenarios:

```yaml
scenarios:
  - name: "Scenario 1"
    weight: 1
  - name: "Scenario 2"
    weight: 1
  - name: "Scenario 3"
    weight: 1
```

Then adjust based on production patterns.

### 3. Use Round-Robin for Balanced Testing

For comprehensive testing of all scenarios:

```rust
let distributor = RoundRobinDistributor::new(scenarios);
// Guarantees equal distribution
```

### 4. Monitor Per-Scenario Metrics

Track metrics separately to identify problematic flows:

```
Scenario: User Login
  Success: 99.9%  ✅
  Avg Time: 120ms

Scenario: Payment Processing
  Success: 95.2%  ⚠️ Investigate failures
  Avg Time: 850ms
```

### 5. Consider Scenario Complexity

Weight scenarios by both traffic and importance:

```yaml
scenarios:
  # Critical path - high weight
  - name: "User Registration"
    weight: 50

  # Important but less frequent
  - name: "Password Reset"
    weight: 10

  # Edge case testing
  - name: "Account Deletion"
    weight: 1
```

## Troubleshooting

### Uneven Distribution

**Problem**: Weighted distribution doesn't match expectations in short tests.

**Solution**: Run longer tests for statistical convergence:
```yaml
config:
  duration: "30m"  # Longer duration = better distribution
```

### Scenario Not Executing

**Problem**: Low-weight scenario never executes.

**Solution**: Increase weight or use round-robin:
```yaml
scenarios:
  - name: "Rare Scenario"
    weight: 5  # Increase from 1 to 5
```

### Metrics Inconsistent

**Problem**: Per-scenario metrics seem incorrect.

**Solution**: Ensure metrics are initialized before recording:
```rust
metrics.initialize_scenarios(&scenarios);
```

## Integration Example

Complete integration with weighted selection and metrics:

```rust
use rust_loadtest::multi_scenario::{ScenarioSelector, ScenarioMetrics};
use rust_loadtest::yaml_config::YamlConfig;

// Load scenarios from YAML
let config = YamlConfig::from_file("loadtest.yaml")?;
let scenarios = config.to_scenarios()?;

// Setup selector and metrics
let selector = ScenarioSelector::new(scenarios.clone());
let mut metrics = ScenarioMetrics::new();
metrics.initialize_scenarios(&scenarios);

// Execute scenarios
for _ in 0..10000 {
    let scenario = selector.select();

    // Execute scenario (simplified)
    let success = execute_scenario(scenario);
    let duration_ms = 100; // From execution

    // Record metrics
    metrics.record_execution(&scenario.name, success, duration_ms);
}

// Print summary
let summary = metrics.summary();
summary.print();
```

## CLI Usage

### Run with Multiple Scenarios

```bash
rust-loadtest --config multi-scenario.yaml
```

### View Per-Scenario Metrics

```bash
rust-loadtest --config multi-scenario.yaml --metrics per-scenario
```

### Test Specific Scenario

```bash
rust-loadtest --config multi-scenario.yaml --scenario "Read Operations"
```

## Related Documentation

- [Scenario YAML Definitions](/docs/SCENARIO_YAML.md)
- [Load Models](/docs/LOAD_MODELS.md)
- [Metrics and Reporting](/docs/METRICS.md)
- [YAML Configuration](/docs/YAML_CONFIG.md)
