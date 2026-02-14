//! Integration tests for config validation (Issue #38).
//!
//! These tests validate the enhanced validation system with detailed error messages.

use rust_loadtest::config_validation::{
    ConfigSchema, DurationValidator, HttpMethodValidator, LoadModelValidator, RangeValidator,
    UrlValidator, ValidationContext,
};
use rust_loadtest::yaml_config::YamlConfig;

#[test]
fn test_url_validator_valid_urls() {
    assert!(UrlValidator::validate("https://example.com").is_ok());
    assert!(UrlValidator::validate("http://localhost").is_ok());
    assert!(UrlValidator::validate("https://api.example.com/v1").is_ok());
    assert!(UrlValidator::validate("http://192.168.1.1:8080").is_ok());

    println!("✅ Valid URLs pass validation");
}

#[test]
fn test_url_validator_invalid_urls() {
    assert!(UrlValidator::validate("").is_err());
    assert!(UrlValidator::validate("example.com").is_err());
    assert!(UrlValidator::validate("ftp://example.com").is_err());
    assert!(UrlValidator::validate("https://example .com").is_err());

    println!("✅ Invalid URLs are rejected");
}

#[test]
fn test_duration_validator_valid_formats() {
    assert!(DurationValidator::validate("1s").is_ok());
    assert!(DurationValidator::validate("30s").is_ok());
    assert!(DurationValidator::validate("5m").is_ok());
    assert!(DurationValidator::validate("2h").is_ok());
    assert!(DurationValidator::validate("1d").is_ok());

    println!("✅ Valid duration formats pass validation");
}

#[test]
fn test_duration_validator_invalid_formats() {
    assert!(DurationValidator::validate("invalid").is_err());
    assert!(DurationValidator::validate("30").is_err()); // missing unit
    assert!(DurationValidator::validate("abc").is_err());

    println!("✅ Invalid duration formats are rejected");
}

#[test]
fn test_duration_validator_positive() {
    assert!(DurationValidator::validate_positive("1s").is_ok());
    assert!(DurationValidator::validate_positive("5m").is_ok());
    assert!(DurationValidator::validate_positive("0s").is_err());

    println!("✅ Zero duration is rejected when positive required");
}

#[test]
fn test_range_validator_u64() {
    assert!(RangeValidator::validate_u64(50, 1, 100, "test").is_ok());
    assert!(RangeValidator::validate_u64(1, 1, 100, "test").is_ok());
    assert!(RangeValidator::validate_u64(100, 1, 100, "test").is_ok());
    assert!(RangeValidator::validate_u64(0, 1, 100, "test").is_err());
    assert!(RangeValidator::validate_u64(101, 1, 100, "test").is_err());

    println!("✅ Range validation for u64 works");
}

#[test]
fn test_range_validator_f64() {
    assert!(RangeValidator::validate_f64(50.0, 1.0, 100.0, "test").is_ok());
    assert!(RangeValidator::validate_f64(0.5, 1.0, 100.0, "test").is_err());
    assert!(RangeValidator::validate_f64(100.5, 1.0, 100.0, "test").is_err());

    println!("✅ Range validation for f64 works");
}

#[test]
fn test_range_validator_positive() {
    assert!(RangeValidator::validate_positive_u64(1, "test").is_ok());
    assert!(RangeValidator::validate_positive_u64(100, "test").is_ok());
    assert!(RangeValidator::validate_positive_u64(0, "test").is_err());

    assert!(RangeValidator::validate_positive_f64(0.1, "test").is_ok());
    assert!(RangeValidator::validate_positive_f64(100.0, "test").is_ok());
    assert!(RangeValidator::validate_positive_f64(0.0, "test").is_err());
    assert!(RangeValidator::validate_positive_f64(-1.0, "test").is_err());

    println!("✅ Positive value validation works");
}

#[test]
fn test_http_method_validator() {
    // Valid methods
    assert!(HttpMethodValidator::validate("GET").is_ok());
    assert!(HttpMethodValidator::validate("POST").is_ok());
    assert!(HttpMethodValidator::validate("PUT").is_ok());
    assert!(HttpMethodValidator::validate("PATCH").is_ok());
    assert!(HttpMethodValidator::validate("DELETE").is_ok());
    assert!(HttpMethodValidator::validate("HEAD").is_ok());
    assert!(HttpMethodValidator::validate("OPTIONS").is_ok());

    // Case insensitive
    assert!(HttpMethodValidator::validate("get").is_ok());
    assert!(HttpMethodValidator::validate("Post").is_ok());

    // Invalid methods
    assert!(HttpMethodValidator::validate("INVALID").is_err());
    assert!(HttpMethodValidator::validate("CONNECT").is_err());

    println!("✅ HTTP method validation works");
}

#[test]
fn test_load_model_validator_rps() {
    assert!(LoadModelValidator::validate_rps(1.0).is_ok());
    assert!(LoadModelValidator::validate_rps(100.0).is_ok());
    assert!(LoadModelValidator::validate_rps(0.1).is_ok());

    assert!(LoadModelValidator::validate_rps(0.0).is_err());
    assert!(LoadModelValidator::validate_rps(-10.0).is_err());

    println!("✅ RPS load model validation works");
}

#[test]
fn test_load_model_validator_ramp() {
    assert!(LoadModelValidator::validate_ramp(10.0, 100.0).is_ok());
    assert!(LoadModelValidator::validate_ramp(0.1, 100.0).is_ok());

    assert!(LoadModelValidator::validate_ramp(100.0, 10.0).is_err());
    assert!(LoadModelValidator::validate_ramp(50.0, 50.0).is_err());
    assert!(LoadModelValidator::validate_ramp(0.0, 100.0).is_err());

    println!("✅ Ramp load model validation works");
}

#[test]
fn test_load_model_validator_daily_traffic() {
    assert!(LoadModelValidator::validate_daily_traffic(10.0, 50.0, 100.0).is_ok());
    assert!(LoadModelValidator::validate_daily_traffic(1.0, 10.0, 100.0).is_ok());

    assert!(LoadModelValidator::validate_daily_traffic(100.0, 50.0, 10.0).is_err());
    assert!(LoadModelValidator::validate_daily_traffic(10.0, 10.0, 100.0).is_err());
    assert!(LoadModelValidator::validate_daily_traffic(10.0, 50.0, 50.0).is_err());

    println!("✅ Daily traffic load model validation works");
}

#[test]
fn test_validation_context() {
    let mut ctx = ValidationContext::new();

    ctx.enter("config");
    assert_eq!(ctx.current_path(), "config");

    ctx.enter("baseUrl");
    assert_eq!(ctx.current_path(), "config.baseUrl");

    ctx.field_error("Invalid URL".to_string());
    assert!(ctx.has_errors());
    assert_eq!(ctx.errors().len(), 1);

    ctx.exit();
    ctx.exit();

    println!("✅ Validation context tracks field paths");
}

#[test]
fn test_validation_context_multiple_errors() {
    let mut ctx = ValidationContext::new();

    ctx.enter("config");
    ctx.enter("baseUrl");
    ctx.field_error("Invalid URL".to_string());
    ctx.exit();

    ctx.enter("workers");
    ctx.field_error("Invalid worker count".to_string());
    ctx.exit();
    ctx.exit();

    assert_eq!(ctx.errors().len(), 2);

    let result = ctx.into_result();
    assert!(result.is_err());

    println!("✅ Validation context collects multiple errors");
}

#[test]
fn test_yaml_validation_invalid_version() {
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

    let err = result.unwrap_err().to_string();
    assert!(err.contains("version"));
    assert!(err.contains("2.0"));

    println!("✅ Invalid version caught by enhanced validation");
}

#[test]
fn test_yaml_validation_invalid_url() {
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

    let err = result.unwrap_err().to_string();
    assert!(err.contains("baseUrl") || err.contains("URL"));

    println!("✅ Invalid base URL caught by enhanced validation");
}

#[test]
fn test_yaml_validation_zero_workers() {
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

    let err = result.unwrap_err().to_string();
    assert!(err.contains("workers") || err.contains("greater than 0"));

    println!("✅ Zero workers caught by enhanced validation");
}

#[test]
fn test_yaml_validation_invalid_http_method() {
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
          method: "INVALID"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("method") || err.contains("INVALID"));

    println!("✅ Invalid HTTP method caught by enhanced validation");
}

#[test]
fn test_yaml_validation_empty_path() {
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
          path: ""
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("path") || err.contains("empty"));

    println!("✅ Empty request path caught by enhanced validation");
}

#[test]
fn test_yaml_validation_invalid_rps() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "rps"
  target: 0
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("load") || err.contains("target") || err.contains("0"));

    println!("✅ Zero RPS caught by enhanced validation");
}

#[test]
fn test_yaml_validation_invalid_ramp() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "ramp"
  min: 100
  max: 10
  rampDuration: "30s"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("load") || err.contains("min") || err.contains("max"));

    println!("✅ Invalid ramp configuration caught");
}

#[test]
fn test_yaml_validation_empty_scenario_name() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: ""
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("name") || err.contains("empty"));

    println!("✅ Empty scenario name caught");
}

#[test]
fn test_yaml_validation_negative_weight() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    weight: 0
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    assert!(err.contains("weight") || err.contains("0"));

    println!("✅ Zero/negative weight caught");
}

#[test]
fn test_yaml_validation_too_many_workers() {
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  workers: 20000
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

    let err = result.unwrap_err().to_string();
    assert!(err.contains("workers") || err.contains("10000"));

    println!("✅ Excessive worker count caught");
}

#[test]
fn test_json_schema_generation() {
    let schema = ConfigSchema::to_json_schema();

    assert!(schema.is_object());
    assert!(schema.get("$schema").is_some());
    assert!(schema.get("title").is_some());
    assert!(schema.get("properties").is_some());

    let properties = schema.get("properties").unwrap();
    assert!(properties.get("version").is_some());
    assert!(properties.get("config").is_some());
    assert!(properties.get("load").is_some());
    assert!(properties.get("scenarios").is_some());

    println!("✅ JSON Schema generation works");
}

#[test]
fn test_json_schema_export() {
    let schema_str = ConfigSchema::export_json_schema();

    assert!(!schema_str.is_empty());
    assert!(schema_str.contains("\"$schema\""));
    assert!(schema_str.contains("\"version\""));
    assert!(schema_str.contains("\"config\""));
    assert!(schema_str.contains("\"baseUrl\""));
    assert!(schema_str.contains("\"workers\""));

    println!("✅ JSON Schema export produces valid JSON");
    println!("   Schema length: {} bytes", schema_str.len());
}

#[test]
fn test_yaml_validation_multiple_errors() {
    let yaml = r#"
version: "2.0"
config:
  baseUrl: "invalid-url"
  workers: 0
  duration: "1m"
load:
  model: "rps"
  target: -10
scenarios:
  - name: ""
    weight: 0
    steps:
      - request:
          method: "INVALID"
          path: ""
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err().to_string();
    // Should contain multiple error mentions
    assert!(err.len() > 100); // Multiple errors make for a long message

    println!("✅ Multiple validation errors are collected");
    println!("   Error message length: {} chars", err.len());
}

#[test]
fn test_yaml_validation_valid_complex_config() {
    let yaml = r#"
version: "1.0"
metadata:
  name: "Valid Complex Test"
  author: "test@example.com"
config:
  baseUrl: "https://api.example.com"
  workers: 50
  duration: "10m"
  timeout: "30s"
load:
  model: "ramp"
  min: 10
  max: 100
  rampDuration: "5m"
scenarios:
  - name: "Heavy Traffic"
    weight: 70
    steps:
      - name: "GET Request"
        request:
          method: "GET"
          path: "/api/test"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"

  - name: "Light Traffic"
    weight: 30
    steps:
      - request:
          method: "POST"
          path: "/api/data"
          body: '{"test": true}'
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_ok());

    println!("✅ Valid complex config passes all validations");
}
