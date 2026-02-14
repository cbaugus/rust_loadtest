//! Integration tests for configuration merging (Issue #39).
//!
//! These tests validate configuration precedence: env > yaml > defaults.

use rust_loadtest::config_merge::{ConfigDefaults, ConfigMerger, ConfigPrecedence};
use std::env;
use std::time::Duration;

#[test]
fn test_default_values() {
    let defaults = ConfigDefaults::new();

    assert_eq!(defaults.workers, 10);
    assert_eq!(defaults.timeout, Duration::from_secs(30));
    assert_eq!(defaults.skip_tls_verify, false);
    assert_eq!(defaults.scenario_weight, 1.0);
    assert_eq!(defaults.load_model, "concurrent");

    // Test static methods too
    assert_eq!(ConfigDefaults::workers(), 10);
    assert_eq!(ConfigDefaults::timeout(), Duration::from_secs(30));
    assert_eq!(ConfigDefaults::skip_tls_verify(), false);
    assert_eq!(ConfigDefaults::scenario_weight(), 1.0);
    assert_eq!(ConfigDefaults::load_model(), "concurrent");

    println!("✅ Default values are correct");
}

#[test]
fn test_workers_precedence_default() {
    // No YAML, no env -> use default
    let result = ConfigMerger::merge_workers(None, "WORKERS_TEST_1");
    assert_eq!(result, 10);

    println!("✅ Workers use default when not specified");
}

#[test]
fn test_workers_precedence_yaml() {
    // YAML provided, no env -> use YAML
    let result = ConfigMerger::merge_workers(Some(50), "WORKERS_TEST_2");
    assert_eq!(result, 50);

    println!("✅ Workers use YAML value when provided");
}

#[test]
fn test_workers_precedence_env_override() {
    // YAML=50, ENV=100 -> use ENV
    env::set_var("WORKERS_TEST_3", "100");
    let result = ConfigMerger::merge_workers(Some(50), "WORKERS_TEST_3");
    assert_eq!(result, 100);
    env::remove_var("WORKERS_TEST_3");

    println!("✅ Environment variable overrides YAML for workers");
}

#[test]
fn test_workers_precedence_full_chain() {
    // Test all three: default < yaml < env

    // 1. Default only
    let result = ConfigMerger::merge_workers(None, "WORKERS_CHAIN_1");
    assert_eq!(result, 10, "Should use default");

    // 2. YAML overrides default
    let result = ConfigMerger::merge_workers(Some(50), "WORKERS_CHAIN_2");
    assert_eq!(result, 50, "Should use YAML");

    // 3. Env overrides YAML and default
    env::set_var("WORKERS_CHAIN_3", "100");
    let result = ConfigMerger::merge_workers(Some(50), "WORKERS_CHAIN_3");
    assert_eq!(result, 100, "Should use env");
    env::remove_var("WORKERS_CHAIN_3");

    println!("✅ Workers precedence chain works: env > yaml > default");
}

#[test]
fn test_timeout_precedence() {
    // Default
    let result = ConfigMerger::merge_timeout(None, "TIMEOUT_TEST_1");
    assert_eq!(result, Duration::from_secs(30));

    // YAML
    let result = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "TIMEOUT_TEST_2");
    assert_eq!(result, Duration::from_secs(60));

    // Env override
    env::set_var("TIMEOUT_TEST_3", "90s");
    let result = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "TIMEOUT_TEST_3");
    assert_eq!(result, Duration::from_secs(90));
    env::remove_var("TIMEOUT_TEST_3");

    println!("✅ Timeout precedence works: env > yaml > default");
}

#[test]
fn test_skip_tls_verify_precedence() {
    // Default
    let result = ConfigMerger::merge_skip_tls_verify(None, "TLS_TEST_1");
    assert_eq!(result, false);

    // YAML
    let result = ConfigMerger::merge_skip_tls_verify(Some(true), "TLS_TEST_2");
    assert_eq!(result, true);

    // Env override with "true"
    env::set_var("TLS_TEST_3", "true");
    let result = ConfigMerger::merge_skip_tls_verify(Some(false), "TLS_TEST_3");
    assert_eq!(result, true);
    env::remove_var("TLS_TEST_3");

    // Env override with "false"
    env::set_var("TLS_TEST_4", "false");
    let result = ConfigMerger::merge_skip_tls_verify(Some(true), "TLS_TEST_4");
    assert_eq!(result, false);
    env::remove_var("TLS_TEST_4");

    println!("✅ Skip TLS verify precedence works");
}

#[test]
fn test_scenario_weight_precedence() {
    // Default
    let result = ConfigMerger::merge_scenario_weight(None);
    assert_eq!(result, 1.0);

    // YAML
    let result = ConfigMerger::merge_scenario_weight(Some(2.5));
    assert_eq!(result, 2.5);

    println!("✅ Scenario weight uses YAML or default");
}

#[test]
fn test_string_precedence() {
    // Default only
    let result = ConfigMerger::merge_string(None, "STRING_TEST_1", "default".to_string());
    assert_eq!(result, "default");

    // YAML overrides default
    let result = ConfigMerger::merge_string(
        Some("yaml".to_string()),
        "STRING_TEST_2",
        "default".to_string(),
    );
    assert_eq!(result, "yaml");

    // Env overrides YAML and default
    env::set_var("STRING_TEST_3", "env");
    let result = ConfigMerger::merge_string(
        Some("yaml".to_string()),
        "STRING_TEST_3",
        "default".to_string(),
    );
    assert_eq!(result, "env");
    env::remove_var("STRING_TEST_3");

    println!("✅ String precedence works: env > yaml > default");
}

#[test]
fn test_optional_string_precedence() {
    // No value
    let result = ConfigMerger::merge_optional_string(None, "OPT_STRING_TEST_1");
    assert_eq!(result, None);

    // YAML only
    let result = ConfigMerger::merge_optional_string(Some("yaml".to_string()), "OPT_STRING_TEST_2");
    assert_eq!(result, Some("yaml".to_string()));

    // Env overrides YAML
    env::set_var("OPT_STRING_TEST_3", "env");
    let result = ConfigMerger::merge_optional_string(Some("yaml".to_string()), "OPT_STRING_TEST_3");
    assert_eq!(result, Some("env".to_string()));
    env::remove_var("OPT_STRING_TEST_3");

    println!("✅ Optional string precedence works: env > yaml");
}

#[test]
fn test_rps_precedence() {
    // No value
    let result = ConfigMerger::merge_rps(None, "RPS_TEST_1");
    assert_eq!(result, None);

    // YAML only
    let result = ConfigMerger::merge_rps(Some(100.0), "RPS_TEST_2");
    assert_eq!(result, Some(100.0));

    // Env overrides YAML
    env::set_var("RPS_TEST_3", "200.5");
    let result = ConfigMerger::merge_rps(Some(100.0), "RPS_TEST_3");
    assert_eq!(result, Some(200.5));
    env::remove_var("RPS_TEST_3");

    println!("✅ RPS precedence works: env > yaml");
}

#[test]
fn test_env_invalid_value_fallback() {
    // Invalid env value should fall back to YAML or default
    env::set_var("ENV_INVALID_1", "not-a-number");
    let result = ConfigMerger::merge_workers(Some(50), "ENV_INVALID_1");
    assert_eq!(result, 50, "Should fall back to YAML when env is invalid");
    env::remove_var("ENV_INVALID_1");

    env::set_var("ENV_INVALID_2", "not-a-number");
    let result = ConfigMerger::merge_workers(None, "ENV_INVALID_2");
    assert_eq!(result, 10, "Should fall back to default when env is invalid");
    env::remove_var("ENV_INVALID_2");

    println!("✅ Invalid env values fall back to YAML or default");
}

#[test]
fn test_env_empty_value_fallback() {
    // Empty env value should fall back to YAML or default
    env::set_var("ENV_EMPTY_1", "");
    let result = ConfigMerger::merge_string(
        Some("yaml".to_string()),
        "ENV_EMPTY_1",
        "default".to_string(),
    );
    assert_eq!(result, "yaml", "Empty env should use YAML");
    env::remove_var("ENV_EMPTY_1");

    env::set_var("ENV_EMPTY_2", "");
    let result = ConfigMerger::merge_string(None, "ENV_EMPTY_2", "default".to_string());
    assert_eq!(result, "default", "Empty env should use default");
    env::remove_var("ENV_EMPTY_2");

    println!("✅ Empty env values fall back to YAML or default");
}

#[test]
fn test_multiple_fields_precedence() {
    // Set multiple env vars
    env::set_var("MULTI_WORKERS", "100");
    env::set_var("MULTI_TIMEOUT", "90s");
    env::set_var("MULTI_TLS", "true");

    // All should use env values
    let workers = ConfigMerger::merge_workers(Some(50), "MULTI_WORKERS");
    let timeout = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "MULTI_TIMEOUT");
    let tls = ConfigMerger::merge_skip_tls_verify(Some(false), "MULTI_TLS");

    assert_eq!(workers, 100);
    assert_eq!(timeout, Duration::from_secs(90));
    assert_eq!(tls, true);

    // Clean up
    env::remove_var("MULTI_WORKERS");
    env::remove_var("MULTI_TIMEOUT");
    env::remove_var("MULTI_TLS");

    println!("✅ Multiple fields respect env precedence independently");
}

#[test]
fn test_precedence_documentation() {
    let docs = ConfigPrecedence::documentation();

    assert!(!docs.is_empty());
    assert!(docs.contains("Precedence"));
    assert!(docs.contains("Environment Variables"));
    assert!(docs.contains("YAML Configuration File"));
    assert!(docs.contains("Default Values"));
    assert!(docs.contains("workers: 10"));
    assert!(docs.contains("timeout: 30s"));

    println!("✅ Precedence documentation is comprehensive");
    println!("   Documentation length: {} chars", docs.len());
}

#[test]
fn test_timeout_duration_formats() {
    // Test various duration formats via env
    env::set_var("TIMEOUT_FMT_1", "30s");
    let result = ConfigMerger::merge_timeout(None, "TIMEOUT_FMT_1");
    assert_eq!(result, Duration::from_secs(30));
    env::remove_var("TIMEOUT_FMT_1");

    env::set_var("TIMEOUT_FMT_2", "5m");
    let result = ConfigMerger::merge_timeout(None, "TIMEOUT_FMT_2");
    assert_eq!(result, Duration::from_secs(300));
    env::remove_var("TIMEOUT_FMT_2");

    env::set_var("TIMEOUT_FMT_3", "2h");
    let result = ConfigMerger::merge_timeout(None, "TIMEOUT_FMT_3");
    assert_eq!(result, Duration::from_secs(7200));
    env::remove_var("TIMEOUT_FMT_3");

    println!("✅ Timeout duration formats work with env override");
}

#[test]
fn test_precedence_isolation() {
    // Test that different fields don't interfere with each other
    env::set_var("ISOLATION_WORKERS", "100");
    // Don't set ISOLATION_TIMEOUT

    let workers = ConfigMerger::merge_workers(Some(50), "ISOLATION_WORKERS");
    let timeout = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "ISOLATION_TIMEOUT");

    assert_eq!(workers, 100, "Workers should use env");
    assert_eq!(timeout, Duration::from_secs(60), "Timeout should use YAML");

    env::remove_var("ISOLATION_WORKERS");

    println!("✅ Field precedence is independent and isolated");
}

#[test]
fn test_case_sensitivity_boolean() {
    // Test boolean env var case insensitivity
    env::set_var("BOOL_TEST_1", "TRUE");
    assert_eq!(ConfigMerger::merge_skip_tls_verify(None, "BOOL_TEST_1"), true);
    env::remove_var("BOOL_TEST_1");

    env::set_var("BOOL_TEST_2", "True");
    assert_eq!(ConfigMerger::merge_skip_tls_verify(None, "BOOL_TEST_2"), true);
    env::remove_var("BOOL_TEST_2");

    env::set_var("BOOL_TEST_3", "true");
    assert_eq!(ConfigMerger::merge_skip_tls_verify(None, "BOOL_TEST_3"), true);
    env::remove_var("BOOL_TEST_3");

    env::set_var("BOOL_TEST_4", "false");
    assert_eq!(ConfigMerger::merge_skip_tls_verify(None, "BOOL_TEST_4"), false);
    env::remove_var("BOOL_TEST_4");

    println!("✅ Boolean env vars are case insensitive");
}

#[test]
fn test_full_precedence_scenario() {
    // Simulate a realistic scenario with all three sources
    println!("\n=== Testing Full Precedence Scenario ===");

    // Defaults (implicit)
    println!("1. Defaults: workers=10, timeout=30s, tls=false");

    // YAML config (simulated)
    let yaml_workers = Some(50);
    let yaml_timeout = Some(Duration::from_secs(60));
    let yaml_tls = Some(false);
    println!("2. YAML: workers=50, timeout=60s, tls=false");

    // Environment overrides (for some fields)
    env::set_var("FULL_WORKERS", "100");
    // No env for timeout - should use YAML
    // No env for tls - should use YAML
    println!("3. Environment: workers=100");

    // Resolve with precedence
    let final_workers = ConfigMerger::merge_workers(yaml_workers, "FULL_WORKERS");
    let final_timeout = ConfigMerger::merge_timeout(yaml_timeout, "FULL_TIMEOUT");
    let final_tls = ConfigMerger::merge_skip_tls_verify(yaml_tls, "FULL_TLS");

    println!("\n4. Final values:");
    println!("   workers: {} (from env)", final_workers);
    println!("   timeout: {}s (from YAML)", final_timeout.as_secs());
    println!("   tls: {} (from YAML)", final_tls);

    assert_eq!(final_workers, 100, "Workers from env");
    assert_eq!(final_timeout, Duration::from_secs(60), "Timeout from YAML");
    assert_eq!(final_tls, false, "TLS from YAML");

    env::remove_var("FULL_WORKERS");

    println!("✅ Full precedence scenario works correctly");
}
