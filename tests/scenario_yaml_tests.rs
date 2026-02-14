//! Integration tests for scenario YAML definitions (Issue #42).
//!
//! These tests validate enhanced scenario features in YAML including:
//! - Data file support (CSV, JSON)
//! - Random think time
//! - Scenario-level configuration overrides
//! - Multiple scenarios with weighting
//! - Complex multi-step scenarios

use rust_loadtest::scenario::ThinkTime;
use rust_loadtest::yaml_config::YamlConfig;
use std::time::Duration;

#[test]
fn test_basic_scenario() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Basic Scenario"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].name, "Basic Scenario");
    assert_eq!(scenarios[0].weight, 1.0); // Default weight
    assert_eq!(scenarios[0].steps.len(), 1);

    println!("✅ Basic scenario parsing works");
}

#[test]
fn test_multiple_scenarios_with_weight() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Heavy Traffic Scenario"
    weight: 70
    steps:
      - request:
          method: "GET"
          path: "/api/v1/popular"

  - name: "Light Traffic Scenario"
    weight: 30
    steps:
      - request:
          method: "GET"
          path: "/api/v1/details"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].name, "Heavy Traffic Scenario");
    assert_eq!(scenarios[0].weight, 70.0);
    assert_eq!(scenarios[1].name, "Light Traffic Scenario");
    assert_eq!(scenarios[1].weight, 30.0);

    println!("✅ Multiple scenarios with weighting work");
}

#[test]
fn test_scenario_with_fixed_think_time() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Scenario with Think Time"
    steps:
      - name: "Step 1"
        request:
          method: "GET"
          path: "/page1"
        thinkTime: "3s"

      - name: "Step 2"
        request:
          method: "GET"
          path: "/page2"
        thinkTime: "5s"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios[0].steps.len(), 2);

    // Check Step 1 think time
    let step1_think_time = scenarios[0].steps[0].think_time.as_ref().unwrap();
    match step1_think_time {
        ThinkTime::Fixed(duration) => {
            assert_eq!(*duration, Duration::from_secs(3));
        }
        _ => panic!("Expected Fixed think time"),
    }

    // Check Step 2 think time
    let step2_think_time = scenarios[0].steps[1].think_time.as_ref().unwrap();
    match step2_think_time {
        ThinkTime::Fixed(duration) => {
            assert_eq!(*duration, Duration::from_secs(5));
        }
        _ => panic!("Expected Fixed think time"),
    }

    println!("✅ Fixed think time works");
}

#[test]
fn test_scenario_with_random_think_time() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Scenario with Random Think Time"
    steps:
      - name: "Browse"
        request:
          method: "GET"
          path: "/browse"
        thinkTime:
          min: "2s"
          max: "5s"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    let think_time = scenarios[0].steps[0].think_time.as_ref().unwrap();
    match think_time {
        ThinkTime::Random { min, max } => {
            assert_eq!(*min, Duration::from_secs(2));
            assert_eq!(*max, Duration::from_secs(5));
        }
        _ => panic!("Expected Random think time"),
    }

    println!("✅ Random think time works");
}

#[test]
fn test_multi_step_scenario() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "10m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "E-commerce Flow"
    weight: 1.0
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"

      - name: "Search"
        request:
          method: "GET"
          path: "/search?q=laptop"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.products[0].id"
        thinkTime: "3s"

      - name: "Product Details"
        request:
          method: "GET"
          path: "/products/${productId}"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "5s"

      - name: "Add to Cart"
        request:
          method: "POST"
          path: "/cart"
          body: '{"productId": "${productId}", "quantity": 1}'
        assertions:
          - type: "statusCode"
            expected: 201
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios[0].steps.len(), 4);
    assert_eq!(scenarios[0].steps[0].name, "Homepage");
    assert_eq!(scenarios[0].steps[1].name, "Search");
    assert_eq!(scenarios[0].steps[2].name, "Product Details");
    assert_eq!(scenarios[0].steps[3].name, "Add to Cart");

    // Validate extraction in step 2
    assert_eq!(scenarios[0].steps[1].extractions.len(), 1);

    // Validate assertions
    assert_eq!(scenarios[0].steps[0].assertions.len(), 1);
    assert_eq!(scenarios[0].steps[2].assertions.len(), 1);
    assert_eq!(scenarios[0].steps[3].assertions.len(), 1);

    println!("✅ Multi-step scenario with extractions and assertions works");
}

#[test]
fn test_scenario_with_data_file_csv() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Data-Driven Test"
    dataFile:
      path: "./testdata/users.csv"
      format: "csv"
      strategy: "sequential"
    steps:
      - request:
          method: "POST"
          path: "/login"
          body: '{"username": "${username}", "password": "${password}"}'
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    // Validate data file configuration
    assert!(config.scenarios[0].data_file.is_some());

    let data_file = config.scenarios[0].data_file.as_ref().unwrap();
    assert_eq!(data_file.path, "./testdata/users.csv");
    assert_eq!(data_file.format, "csv");
    assert_eq!(data_file.strategy, "sequential");

    println!("✅ Data file configuration (CSV) works");
}

#[test]
fn test_scenario_with_data_file_json() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "JSON Data-Driven Test"
    dataFile:
      path: "./testdata/products.json"
      format: "json"
      strategy: "random"
    steps:
      - request:
          method: "GET"
          path: "/products/${productId}"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    let data_file = config.scenarios[0].data_file.as_ref().unwrap();
    assert_eq!(data_file.path, "./testdata/products.json");
    assert_eq!(data_file.format, "json");
    assert_eq!(data_file.strategy, "random");

    println!("✅ Data file configuration (JSON) works");
}

#[test]
fn test_scenario_with_config_overrides() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  timeout: "30s"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Slow API Scenario"
    config:
      timeout: "120s"
      retryCount: 3
      retryDelay: "5s"
    steps:
      - request:
          method: "GET"
          path: "/slow-endpoint"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    // Validate scenario config overrides
    let scenario_config = &config.scenarios[0].config;
    assert!(scenario_config.timeout.is_some());
    assert_eq!(scenario_config.retry_count, Some(3));
    assert!(scenario_config.retry_delay.is_some());

    println!("✅ Scenario-level config overrides work");
}

#[test]
fn test_scenario_with_extractors() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test Extractors"
    steps:
      - name: "Get User"
        request:
          method: "GET"
          path: "/user/123"
        extract:
          - type: "jsonPath"
            name: "userId"
            jsonPath: "$.id"
          - type: "jsonPath"
            name: "userName"
            jsonPath: "$.name"
          - type: "header"
            name: "authToken"
            header: "X-Auth-Token"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios[0].steps[0].extractions.len(), 3);

    println!("✅ Multiple extractors per step work");
}

#[test]
fn test_scenario_with_multiple_assertions() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test Assertions"
    steps:
      - name: "API Call"
        request:
          method: "POST"
          path: "/api/data"
          body: '{"test": true}'
        assertions:
          - type: "statusCode"
            expected: 201
          - type: "responseTime"
            max: "500ms"
          - type: "bodyContains"
            text: "success"
          - type: "jsonPath"
            path: "$.status"
            expected: "ok"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios[0].steps[0].assertions.len(), 4);

    println!("✅ Multiple assertions per step work");
}

#[test]
fn test_scenario_with_headers_and_query_params() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test Headers and Query Params"
    steps:
      - request:
          method: "GET"
          path: "/api/search"
          queryParams:
            q: "laptop"
            limit: "10"
            sort: "price"
          headers:
            Authorization: "Bearer ${token}"
            X-Custom-Header: "test-value"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    // Validate request path includes query params
    assert!(scenarios[0].steps[0].request.path.contains("?"));
    assert!(scenarios[0].steps[0].request.path.contains("q=laptop"));
    assert!(scenarios[0].steps[0].request.path.contains("limit=10"));

    // Validate headers
    assert_eq!(scenarios[0].steps[0].request.headers.len(), 2);

    println!("✅ Headers and query parameters work");
}

#[test]
fn test_weighted_scenario_distribution() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Read Operations"
    weight: 80
    steps:
      - request:
          method: "GET"
          path: "/api/read"

  - name: "Write Operations"
    weight: 15
    steps:
      - request:
          method: "POST"
          path: "/api/write"

  - name: "Delete Operations"
    weight: 5
    steps:
      - request:
          method: "DELETE"
          path: "/api/delete"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 3);

    let total_weight: f64 = scenarios.iter().map(|s| s.weight).sum();
    assert_eq!(total_weight, 100.0);

    // Verify percentages
    assert_eq!(scenarios[0].weight / total_weight, 0.80); // 80%
    assert_eq!(scenarios[1].weight / total_weight, 0.15); // 15%
    assert_eq!(scenarios[2].weight / total_weight, 0.05); // 5%

    println!("✅ Weighted scenario distribution works");
}

#[test]
fn test_scenario_with_no_think_time() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Fast Scenario"
    steps:
      - request:
          method: "GET"
          path: "/fast"
      - request:
          method: "GET"
          path: "/fast2"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert!(scenarios[0].steps[0].think_time.is_none());
    assert!(scenarios[0].steps[1].think_time.is_none());

    println!("✅ Scenarios without think time work");
}

#[test]
fn test_scenario_data_file_defaults() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test Defaults"
    dataFile:
      path: "./data.csv"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    let data_file = config.scenarios[0].data_file.as_ref().unwrap();
    assert_eq!(data_file.format, "csv"); // Default format
    assert_eq!(data_file.strategy, "sequential"); // Default strategy

    println!("✅ Data file defaults work");
}

#[test]
fn test_complex_real_world_scenario() {
    let yaml = r#"
version: "1.0"
metadata:
  name: "E-commerce Load Test"
  description: "Realistic user shopping flow"
  author: "test@example.com"
config:
  baseUrl: "https://shop.example.com"
  workers: 50
  timeout: "30s"
  duration: "30m"
load:
  model: "ramp"
  min: 10
  max: 200
  rampDuration: "10m"
scenarios:
  - name: "Browse and Purchase"
    weight: 70
    config:
      timeout: "60s"
      retryCount: 2
      retryDelay: "3s"
    dataFile:
      path: "./users.csv"
      format: "csv"
      strategy: "cycle"
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "responseTime"
            max: "1s"
        thinkTime:
          min: "1s"
          max: "3s"

      - name: "Login"
        request:
          method: "POST"
          path: "/api/auth/login"
          body: '{"email": "${email}", "password": "${password}"}'
          headers:
            Content-Type: "application/json"
        extract:
          - type: "jsonPath"
            name: "authToken"
            jsonPath: "$.token"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"

      - name: "Search Products"
        request:
          method: "GET"
          path: "/api/products/search"
          queryParams:
            q: "laptop"
            limit: "20"
          headers:
            Authorization: "Bearer ${authToken}"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.results[0].id"
        thinkTime:
          min: "2s"
          max: "5s"

      - name: "View Product"
        request:
          method: "GET"
          path: "/api/products/${productId}"
          headers:
            Authorization: "Bearer ${authToken}"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "bodyContains"
            text: "price"
        thinkTime: "4s"

      - name: "Add to Cart"
        request:
          method: "POST"
          path: "/api/cart/items"
          body: '{"productId": "${productId}", "quantity": 1}'
          headers:
            Authorization: "Bearer ${authToken}"
            Content-Type: "application/json"
        assertions:
          - type: "statusCode"
            expected: 201
        thinkTime: "2s"

  - name: "Quick Browse"
    weight: 30
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        thinkTime: "1s"

      - name: "Category Page"
        request:
          method: "GET"
          path: "/category/electronics"
        thinkTime:
          min: "2s"
          max: "4s"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    // Validate overall structure
    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].name, "Browse and Purchase");
    assert_eq!(scenarios[0].steps.len(), 5);
    assert_eq!(scenarios[1].name, "Quick Browse");
    assert_eq!(scenarios[1].steps.len(), 2);

    // Validate weighting
    assert_eq!(scenarios[0].weight, 70.0);
    assert_eq!(scenarios[1].weight, 30.0);

    // Validate data file
    assert!(config.scenarios[0].data_file.is_some());

    // Validate config overrides
    assert!(config.scenarios[0].config.timeout.is_some());
    assert_eq!(config.scenarios[0].config.retry_count, Some(2));

    println!("✅ Complex real-world scenario works");
}
