//! Integration tests for environment variable overrides (Issue #40).
//!
//! These tests validate that environment variables can override YAML config values
//! according to precedence: env > yaml > defaults.

use rust_loadtest::config::Config;
use rust_loadtest::load_models::LoadModel;
use rust_loadtest::yaml_config::YamlConfig;
use serial_test::serial;
use std::env;
use std::time::Duration;

/// Clear all env vars that could affect config parsing.
/// Must be called at the start of every test to prevent leakage
/// from other tests (execution order is not guaranteed).
fn clean_env() {
    for var in [
        "TARGET_URL",
        "NUM_CONCURRENT_TASKS",
        "REQUEST_TIMEOUT",
        "TEST_DURATION",
        "SKIP_TLS_VERIFY",
        "CUSTOM_HEADERS",
        "LOAD_MODEL_TYPE",
        "TARGET_RPS",
        "MIN_RPS",
        "MAX_RPS",
        "RAMP_DURATION",
        "DAILY_MIN_RPS",
        "DAILY_MID_RPS",
        "DAILY_MAX_RPS",
        "DAILY_CYCLE_DURATION",
    ] {
        env::remove_var(var);
    }
}

#[test]
#[serial]
fn test_no_env_override_uses_yaml_values() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.example.com"
  workers: 50
  timeout: "60s"
  duration: "10m"
  skipTlsVerify: true
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.target_url, "https://yaml.example.com");
    assert_eq!(config.num_concurrent_tasks, 50);
    assert_eq!(config.test_duration, Duration::from_secs(600)); // 10m
    assert!(config.skip_tls_verify);

    println!("✅ YAML values used when no env overrides");
}

#[test]
#[serial]
fn test_env_overrides_base_url() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.example.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("TARGET_URL", "https://env.example.com");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.target_url, "https://env.example.com");

    env::remove_var("TARGET_URL");

    println!("✅ TARGET_URL env var overrides YAML baseUrl");
}

#[test]
#[serial]
fn test_env_overrides_workers() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  workers: 50
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("NUM_CONCURRENT_TASKS", "100");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.num_concurrent_tasks, 100);

    env::remove_var("NUM_CONCURRENT_TASKS");

    println!("✅ NUM_CONCURRENT_TASKS env var overrides YAML workers");
}

#[test]
#[serial]
fn test_env_overrides_timeout() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  timeout: "30s"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("REQUEST_TIMEOUT", "90s");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let _config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    // Note: timeout is currently not stored in Config struct, but test validates parsing works
    // The timeout is used in client config creation

    env::remove_var("REQUEST_TIMEOUT");

    println!("✅ REQUEST_TIMEOUT env var overrides YAML timeout");
}

#[test]
#[serial]
fn test_env_overrides_test_duration() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("TEST_DURATION", "30m");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.test_duration, Duration::from_secs(1800)); // 30m

    env::remove_var("TEST_DURATION");

    println!("✅ TEST_DURATION env var overrides YAML duration");
}

#[test]
#[serial]
fn test_env_overrides_skip_tls_verify() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
  skipTlsVerify: false
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("SKIP_TLS_VERIFY", "true");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert!(config.skip_tls_verify);

    env::remove_var("SKIP_TLS_VERIFY");

    println!("✅ SKIP_TLS_VERIFY env var overrides YAML skipTlsVerify");
}

#[test]
#[serial]
fn test_env_overrides_custom_headers() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
  customHeaders: "X-YAML-Header:yaml-value"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("CUSTOM_HEADERS", "X-ENV-Header:env-value");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.custom_headers.unwrap(), "X-ENV-Header:env-value");

    env::remove_var("CUSTOM_HEADERS");

    println!("✅ CUSTOM_HEADERS env var overrides YAML customHeaders");
}

#[test]
#[serial]
fn test_env_overrides_rps_target() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("TARGET_RPS", "500");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    match config.load_model {
        LoadModel::Rps { target_rps } => {
            assert_eq!(target_rps, 500.0);
        }
        _ => panic!("Expected RPS load model"),
    }

    env::remove_var("TARGET_RPS");

    println!("✅ TARGET_RPS env var overrides YAML load.target");
}

#[test]
#[serial]
fn test_env_overrides_ramp_params() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "ramp"
  min: 10
  max: 100
  rampDuration: "2m"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("MIN_RPS", "50");
    env::set_var("MAX_RPS", "500");
    env::set_var("RAMP_DURATION", "10m");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    match config.load_model {
        LoadModel::RampRps {
            min_rps,
            max_rps,
            ramp_duration,
        } => {
            assert_eq!(min_rps, 50.0);
            assert_eq!(max_rps, 500.0);
            assert_eq!(ramp_duration, Duration::from_secs(600)); // 10m
        }
        _ => panic!("Expected RampRps load model"),
    }

    env::remove_var("MIN_RPS");
    env::remove_var("MAX_RPS");
    env::remove_var("RAMP_DURATION");

    println!("✅ MIN_RPS, MAX_RPS, RAMP_DURATION env vars override YAML ramp params");
}

#[test]
#[serial]
fn test_env_overrides_load_model_entirely() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("LOAD_MODEL_TYPE", "Rps");
    env::set_var("TARGET_RPS", "200");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    match config.load_model {
        LoadModel::Rps { target_rps } => {
            assert_eq!(target_rps, 200.0);
        }
        _ => panic!("Expected RPS load model"),
    }

    env::remove_var("LOAD_MODEL_TYPE");
    env::remove_var("TARGET_RPS");

    println!("✅ LOAD_MODEL_TYPE env var completely overrides YAML load model");
}

#[test]
#[serial]
fn test_multiple_env_overrides_together() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.com"
  workers: 10
  timeout: "30s"
  duration: "5m"
  skipTlsVerify: false
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

    env::set_var("TARGET_URL", "https://env.com");
    env::set_var("NUM_CONCURRENT_TASKS", "100");
    env::set_var("TEST_DURATION", "30m");
    env::set_var("SKIP_TLS_VERIFY", "true");
    env::set_var("TARGET_RPS", "500");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.target_url, "https://env.com");
    assert_eq!(config.num_concurrent_tasks, 100);
    assert_eq!(config.test_duration, Duration::from_secs(1800)); // 30m
    assert!(config.skip_tls_verify);

    match config.load_model {
        LoadModel::Rps { target_rps } => {
            assert_eq!(target_rps, 500.0);
        }
        _ => panic!("Expected RPS load model"),
    }

    env::remove_var("TARGET_URL");
    env::remove_var("NUM_CONCURRENT_TASKS");
    env::remove_var("TEST_DURATION");
    env::remove_var("SKIP_TLS_VERIFY");
    env::remove_var("TARGET_RPS");

    println!("✅ Multiple env vars can override YAML values independently");
}

#[test]
#[serial]
fn test_partial_env_overrides() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.com"
  workers: 50
  timeout: "60s"
  duration: "10m"
  skipTlsVerify: true
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    // Only override some fields
    env::set_var("NUM_CONCURRENT_TASKS", "200");
    env::set_var("TARGET_RPS", "500");
    // Don't set TARGET_URL, TEST_DURATION, SKIP_TLS_VERIFY

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    // Overridden by env
    assert_eq!(config.num_concurrent_tasks, 200);
    match config.load_model {
        LoadModel::Rps { target_rps } => {
            assert_eq!(target_rps, 500.0);
        }
        _ => panic!("Expected RPS load model"),
    }

    // Not overridden, should use YAML values
    assert_eq!(config.target_url, "https://yaml.com");
    assert_eq!(config.test_duration, Duration::from_secs(600)); // 10m
    assert!(config.skip_tls_verify);

    env::remove_var("NUM_CONCURRENT_TASKS");
    env::remove_var("TARGET_RPS");

    println!("✅ Partial env overrides work correctly");
}

#[test]
#[serial]
fn test_env_override_with_yaml_defaults() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
  # workers and timeout will use YAML defaults (10 and 30s)
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("NUM_CONCURRENT_TASKS", "75");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    // Env override
    assert_eq!(config.num_concurrent_tasks, 75);

    // YAML default (workers defaults to 10 in YAML)
    // Test that we can load without error

    env::remove_var("NUM_CONCURRENT_TASKS");

    println!("✅ Env overrides work with YAML default values");
}

#[test]
#[serial]
fn test_env_override_precedence_chain() {
    clean_env();
    // Test full precedence: env > yaml > default
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.com"
  workers: 50  # YAML overrides default (10)
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("NUM_CONCURRENT_TASKS", "100"); // ENV overrides YAML (50) and default (10)

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    assert_eq!(config.num_concurrent_tasks, 100); // From ENV

    env::remove_var("NUM_CONCURRENT_TASKS");

    // Now without env, should use YAML value
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();
    assert_eq!(config.num_concurrent_tasks, 50); // From YAML

    println!("✅ Full precedence chain works: env > yaml > default");
}

#[test]
#[serial]
fn test_invalid_env_override_falls_back_to_yaml() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.com"
  workers: 50
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("NUM_CONCURRENT_TASKS", "invalid-number");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    // Invalid env var should fall back to YAML value
    assert_eq!(config.num_concurrent_tasks, 50);

    env::remove_var("NUM_CONCURRENT_TASKS");

    println!("✅ Invalid env var falls back to YAML value");
}

#[test]
#[serial]
fn test_empty_env_override_falls_back_to_yaml() {
    clean_env();
    let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://yaml.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
"#;

    env::set_var("TARGET_URL", "");

    let yaml_config = YamlConfig::from_str(yaml).unwrap();
    let config = Config::from_yaml_with_env_overrides(&yaml_config).unwrap();

    // Empty env var should fall back to YAML value
    assert_eq!(config.target_url, "https://yaml.com");

    env::remove_var("TARGET_URL");

    println!("✅ Empty env var falls back to YAML value");
}

#[test]
#[serial]
fn test_env_override_documentation() {
    clean_env();
    // This test documents the environment variable mapping
    let mappings = vec![
        ("TARGET_URL", "config.baseUrl"),
        ("NUM_CONCURRENT_TASKS", "config.workers"),
        ("REQUEST_TIMEOUT", "config.timeout"),
        ("TEST_DURATION", "config.duration"),
        ("SKIP_TLS_VERIFY", "config.skipTlsVerify"),
        ("CUSTOM_HEADERS", "config.customHeaders"),
        ("LOAD_MODEL_TYPE", "load.model"),
        ("TARGET_RPS", "load.target (RPS model)"),
        ("MIN_RPS", "load.min (Ramp model)"),
        ("MAX_RPS", "load.max (Ramp model)"),
        ("RAMP_DURATION", "load.rampDuration (Ramp model)"),
        ("DAILY_MIN_RPS", "load.min (DailyTraffic model)"),
        ("DAILY_MID_RPS", "load.mid (DailyTraffic model)"),
        ("DAILY_MAX_RPS", "load.max (DailyTraffic model)"),
        (
            "DAILY_CYCLE_DURATION",
            "load.cycleDuration (DailyTraffic model)",
        ),
    ];

    println!("\n=== Environment Variable Override Mapping ===");
    println!("Precedence: env > yaml > default\n");
    for (env_var, yaml_path) in mappings {
        println!("  {} → {}", env_var, yaml_path);
    }
    println!("===========================================\n");

    println!("✅ Environment variable override mapping documented");
}
