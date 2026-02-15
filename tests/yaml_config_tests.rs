//! Integration tests for YAML configuration (Issue #37).
//!
//! These tests validate YAML config file parsing, validation, and conversion.

use rust_loadtest::yaml_config::{YamlConfig, YamlConfigError};
use std::fs;
use tempfile::NamedTempFile;

#[test]
fn test_simple_yaml_config() {
    let yaml = r#"
version: "1.0"
metadata:
  name: "Simple Test"
  description: "Basic API test"
config:
  baseUrl: "https://api.example.com"
  workers: 10
  duration: "5m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Health Check"
    steps:
      - request:
          method: "GET"
          path: "/health"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    assert_eq!(config.version, "1.0");
    assert_eq!(config.metadata.name, Some("Simple Test".to_string()));
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 10);
    assert_eq!(config.scenarios.len(), 1);

    println!("✅ Simple YAML config parses correctly");
}

#[test]
fn test_yaml_config_from_file() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let temp_file = NamedTempFile::new().unwrap();
    fs::write(temp_file.path(), yaml).unwrap();

    let config = YamlConfig::from_file(temp_file.path()).unwrap();
    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://test.com");

    println!("✅ YAML config loads from file");
}

#[test]
fn test_yaml_duration_formats() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "30s"
  timeout: 15
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
        thinkTime: "2s"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    let duration = config.config.duration.to_std_duration().unwrap();
    assert_eq!(duration.as_secs(), 30);

    let timeout = config.config.timeout.to_std_duration().unwrap();
    assert_eq!(timeout.as_secs(), 15);

    let scenarios = config.to_scenarios().unwrap();
    let think_time = scenarios[0].steps[0].think_time.as_ref().unwrap();
    match think_time {
        rust_loadtest::scenario::ThinkTime::Fixed(d) => assert_eq!(d.as_secs(), 2),
        _ => panic!("Expected fixed think time"),
    }

    println!("✅ Duration formats (seconds and strings) work");
}

#[test]
fn test_yaml_load_models() {
    // Test RPS model
    let yaml_rps = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "rps"
  target: 50
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let config = YamlConfig::from_str(yaml_rps).unwrap();
    let load_model = config.load.to_load_model().unwrap();
    match load_model {
        rust_loadtest::load_models::LoadModel::Rps { target_rps } => {
            assert_eq!(target_rps, 50.0);
        }
        _ => panic!("Expected RPS load model"),
    }

    // Test Ramp model
    let yaml_ramp = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "ramp"
  min: 10
  max: 100
  rampDuration: "30s"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let config = YamlConfig::from_str(yaml_ramp).unwrap();
    let load_model = config.load.to_load_model().unwrap();
    match load_model {
        rust_loadtest::load_models::LoadModel::RampRps { min_rps, max_rps, ramp_duration } => {
            assert_eq!(min_rps, 10.0);
            assert_eq!(max_rps, 100.0);
            assert_eq!(ramp_duration.as_secs(), 30);
        }
        _ => panic!("Expected Ramp load model"),
    }

    println!("✅ All load model types parse correctly");
}

#[test]
fn test_yaml_scenarios_with_assertions() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "API Test"
    weight: 1.5
    steps:
      - name: "Create Resource"
        request:
          method: "POST"
          path: "/api/resource"
          body: '{"name": "test"}'
        assertions:
          - type: "statusCode"
            expected: 201
          - type: "jsonPath"
            path: "$.id"
          - type: "responseTime"
            max: "500ms"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 1);
    assert_eq!(scenarios[0].name, "API Test");
    assert_eq!(scenarios[0].weight, 1.5);
    assert_eq!(scenarios[0].steps.len(), 1);
    assert_eq!(scenarios[0].steps[0].assertions.len(), 3);

    println!("✅ Scenarios with assertions convert correctly");
}

#[test]
fn test_yaml_scenarios_with_extractors() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://shop.example.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Shopping Flow"
    steps:
      - name: "Search Products"
        request:
          method: "GET"
          path: "/api/search?q=laptop"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.products[0].id"
          - type: "header"
            name: "sessionToken"
            header: "X-Session-Token"
        thinkTime: "2s"

      - name: "View Product"
        request:
          method: "GET"
          path: "/products/${productId}"
        thinkTime: "3s"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios[0].steps.len(), 2);
    assert_eq!(scenarios[0].steps[0].extractions.len(), 2);

    // Check extractor types
    match &scenarios[0].steps[0].extractions[0] {
        rust_loadtest::scenario::Extractor::JsonPath { var_name, json_path } => {
            assert_eq!(var_name, "productId");
            assert_eq!(json_path, "$.products[0].id");
        }
        _ => panic!("Expected JsonPath extractor"),
    }

    println!("✅ Scenarios with extractors convert correctly");
}

#[test]
fn test_yaml_query_params() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Search"
    steps:
      - request:
          method: "GET"
          path: "/search"
          queryParams:
            q: "laptop"
            limit: "20"
            sort: "price"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    let path = &scenarios[0].steps[0].request.path;
    assert!(path.contains("?"));
    assert!(path.contains("q=laptop"));
    assert!(path.contains("limit=20"));
    assert!(path.contains("sort=price"));

    println!("✅ Query parameters are appended to path");
}

#[test]
fn test_yaml_custom_headers() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "1m"
  customHeaders: "Authorization: Bearer token123"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/api/data"
          headers:
            X-Custom-Header: "value"
            Content-Type: "application/json"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    let headers = &scenarios[0].steps[0].request.headers;
    assert_eq!(headers.get("X-Custom-Header"), Some(&"value".to_string()));
    assert_eq!(headers.get("Content-Type"), Some(&"application/json".to_string()));

    println!("✅ Custom headers work correctly");
}

#[test]
fn test_validation_unsupported_version() {
    let yaml = r#"
version: "2.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::Validation(msg) => {
            assert!(msg.contains("Unsupported config version"));
            println!("✅ Unsupported version rejected: {}", msg);
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_validation_invalid_url() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "not-a-url"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::Validation(msg) => {
            assert!(msg.contains("Invalid base URL"));
            println!("✅ Invalid URL rejected: {}", msg);
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_validation_zero_workers() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  workers: 0
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::Validation(msg) => {
            assert!(msg.contains("workers must be greater than 0"));
            println!("✅ Zero workers rejected: {}", msg);
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_validation_no_scenarios() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios: []
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::Validation(msg) => {
            assert!(msg.contains("At least one scenario"));
            println!("✅ Empty scenarios rejected: {}", msg);
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_validation_empty_scenario_steps() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Empty Scenario"
    steps: []
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::Validation(msg) => {
            assert!(msg.contains("must have at least one step"));
            println!("✅ Empty scenario steps rejected: {}", msg);
        }
        _ => panic!("Expected validation error"),
    }
}

#[test]
fn test_validation_invalid_duration_format() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "invalid"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_ok()); // Parse succeeds

    let config = result.unwrap();
    let duration_result = config.config.duration.to_std_duration();
    assert!(duration_result.is_err());

    println!("✅ Invalid duration format detected during conversion");
}

#[test]
fn test_multiple_scenarios_different_weights() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Heavy Traffic"
    weight: 70
    steps:
      - request:
          method: "GET"
          path: "/api/heavy"

  - name: "Light Traffic"
    weight: 30
    steps:
      - request:
          method: "GET"
          path: "/api/light"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();
    let scenarios = config.to_scenarios().unwrap();

    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].weight, 70.0);
    assert_eq!(scenarios[1].weight, 30.0);

    println!("✅ Multiple scenarios with different weights work");
}

#[test]
fn test_complex_ecommerce_scenario() {
    let yaml = r#"
version: "1.0"
metadata:
  name: "E-commerce Load Test"
  description: "Full shopping flow"
  author: "test@example.com"
  tags: ["production", "critical"]
config:
  baseUrl: "https://shop.example.com"
  workers: 50
  duration: "10m"
  timeout: "30s"
  skipTlsVerify: false
load:
  model: "ramp"
  min: 10
  max: 100
  rampDuration: "2m"
scenarios:
  - name: "Browse and Purchase"
    weight: 70
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "responseTime"
            max: "500ms"
        thinkTime: "2s"

      - name: "Search"
        request:
          method: "GET"
          path: "/search"
          queryParams:
            q: "laptop"
            limit: "20"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.products[0].id"
          - type: "jsonPath"
            name: "productPrice"
            jsonPath: "$.products[0].price"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "jsonPath"
            path: "$.products"
        thinkTime: "3s"

      - name: "View Product"
        request:
          method: "GET"
          path: "/products/${productId}"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "bodyContains"
            text: "Add to Cart"
        thinkTime: "5s"

      - name: "Add to Cart"
        request:
          method: "POST"
          path: "/api/cart"
          headers:
            Content-Type: "application/json"
          body: '{"productId": "${productId}", "quantity": 1}'
        extract:
          - type: "jsonPath"
            name: "cartId"
            jsonPath: "$.cartId"
        assertions:
          - type: "statusCode"
            expected: 201
          - type: "jsonPath"
            path: "$.cartId"
        thinkTime: "1s"

  - name: "Quick Browse"
    weight: 30
    steps:
      - request:
          method: "GET"
          path: "/"
      - request:
          method: "GET"
          path: "/products/featured"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    // Validate metadata
    assert_eq!(config.metadata.name, Some("E-commerce Load Test".to_string()));
    assert_eq!(config.metadata.tags.len(), 2);

    // Validate config
    assert_eq!(config.config.workers, 50);
    assert!(!config.config.skip_tls_verify);

    // Validate load model
    let load_model = config.load.to_load_model().unwrap();
    match load_model {
        rust_loadtest::load_models::LoadModel::RampRps { min_rps, max_rps, .. } => {
            assert_eq!(min_rps, 10.0);
            assert_eq!(max_rps, 100.0);
        }
        _ => panic!("Expected RampRps model"),
    }

    // Validate scenarios
    let scenarios = config.to_scenarios().unwrap();
    assert_eq!(scenarios.len(), 2);
    assert_eq!(scenarios[0].steps.len(), 4);
    assert_eq!(scenarios[1].steps.len(), 2);

    println!("✅ Complex e-commerce scenario parses completely");
}

#[test]
fn test_default_values() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let config = YamlConfig::from_str(yaml).unwrap();

    // Default workers should be 10
    assert_eq!(config.config.workers, 10);

    // Default timeout should be 30 seconds
    let timeout = config.config.timeout.to_std_duration().unwrap();
    assert_eq!(timeout.as_secs(), 30);

    // Default scenario weight should be 1.0
    assert_eq!(config.scenarios[0].weight, 1.0);

    println!("✅ Default values are applied correctly");
}

#[test]
fn test_parse_error_helpful_message() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request
          method: "GET"  # Missing colon after 'request'
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    match result.unwrap_err() {
        YamlConfigError::YamlParse(e) => {
            let error_msg = e.to_string();
            assert!(!error_msg.is_empty());
            println!("✅ Parse error provides message: {}", error_msg);
        }
        _ => panic!("Expected YAML parse error"),
    }
}
