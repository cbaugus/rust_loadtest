//! Configuration merging and default values (Issue #39).
//!
//! This module implements configuration precedence:
//! Environment Variables > YAML File > Default Values

use std::collections::HashMap;
use std::env;
use std::time::Duration;

/// Default configuration values for all optional fields.
#[derive(Debug, Clone)]
pub struct ConfigDefaults {
    /// Default number of workers
    pub workers: usize,

    /// Default request timeout
    pub timeout: Duration,

    /// Default skip TLS verify
    pub skip_tls_verify: bool,

    /// Default scenario weight
    pub scenario_weight: f64,

    /// Default load model
    pub load_model: String,
}

impl Default for ConfigDefaults {
    fn default() -> Self {
        Self {
            workers: 10,
            timeout: Duration::from_secs(30),
            skip_tls_verify: false,
            scenario_weight: 1.0,
            load_model: "concurrent".to_string(),
        }
    }
}

impl ConfigDefaults {
    /// Get default configuration values.
    pub fn new() -> Self {
        Self::default()
    }

    /// Get default workers count.
    pub fn workers() -> usize {
        10
    }

    /// Get default timeout duration.
    pub fn timeout() -> Duration {
        Duration::from_secs(30)
    }

    /// Get default skip TLS verify flag.
    pub fn skip_tls_verify() -> bool {
        false
    }

    /// Get default scenario weight.
    pub fn scenario_weight() -> f64 {
        1.0
    }

    /// Get default load model.
    pub fn load_model() -> String {
        "concurrent".to_string()
    }
}

/// Configuration precedence resolver.
///
/// Resolves configuration values according to precedence:
/// 1. Environment variables (highest priority)
/// 2. YAML file values
/// 3. Default values (lowest priority)
pub struct ConfigMerger;

impl ConfigMerger {
    /// Merge workers with precedence: env > yaml > default.
    pub fn merge_workers(yaml_value: Option<usize>, env_var: &str) -> usize {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            if let Ok(parsed) = env_val.parse::<usize>() {
                return parsed;
            }
        }

        // Fall back to YAML value or default
        yaml_value.unwrap_or_else(ConfigDefaults::workers)
    }

    /// Merge timeout with precedence: env > yaml > default.
    pub fn merge_timeout(yaml_value: Option<Duration>, env_var: &str) -> Duration {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            if let Ok(parsed) = crate::utils::parse_duration_string(&env_val) {
                return parsed;
            }
        }

        // Fall back to YAML value or default
        yaml_value.unwrap_or_else(ConfigDefaults::timeout)
    }

    /// Merge skip TLS verify with precedence: env > yaml > default.
    pub fn merge_skip_tls_verify(yaml_value: Option<bool>, env_var: &str) -> bool {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            return env_val.to_lowercase() == "true";
        }

        // Fall back to YAML value or default
        yaml_value.unwrap_or_else(ConfigDefaults::skip_tls_verify)
    }

    /// Merge scenario weight with precedence: yaml > default.
    pub fn merge_scenario_weight(yaml_value: Option<f64>) -> f64 {
        yaml_value.unwrap_or_else(ConfigDefaults::scenario_weight)
    }

    /// Merge string value with precedence: env > yaml > default.
    pub fn merge_string(
        yaml_value: Option<String>,
        env_var: &str,
        default: String,
    ) -> String {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            if !env_val.is_empty() {
                return env_val;
            }
        }

        // Fall back to YAML value or default
        yaml_value.unwrap_or(default)
    }

    /// Merge optional string with precedence: env > yaml.
    pub fn merge_optional_string(
        yaml_value: Option<String>,
        env_var: &str,
    ) -> Option<String> {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            if !env_val.is_empty() {
                return Some(env_val);
            }
        }

        // Fall back to YAML value
        yaml_value
    }

    /// Merge RPS value with precedence: env > yaml.
    pub fn merge_rps(yaml_value: Option<f64>, env_var: &str) -> Option<f64> {
        // Check environment variable first
        if let Ok(env_val) = env::var(env_var) {
            if let Ok(parsed) = env_val.parse::<f64>() {
                return Some(parsed);
            }
        }

        // Fall back to YAML value
        yaml_value
    }
}

/// Configuration precedence documentation.
pub struct ConfigPrecedence;

impl ConfigPrecedence {
    /// Get documentation for configuration precedence.
    pub fn documentation() -> &'static str {
        r#"
# Configuration Precedence

Configuration values are resolved in the following order (highest to lowest priority):

1. **Environment Variables** (Highest Priority)
   - Override both YAML and defaults
   - Useful for CI/CD, Docker, Kubernetes
   - Example: NUM_CONCURRENT_TASKS=50

2. **YAML Configuration File**
   - Override defaults
   - Version-controlled test definitions
   - Example: config.workers: 20

3. **Default Values** (Lowest Priority)
   - Used when not specified in YAML or environment
   - Sensible defaults for common use cases

## Default Values

- workers: 10
- timeout: 30s
- skipTlsVerify: false
- scenario weight: 1.0
- load model: "concurrent"

## Environment Variable Mapping

| YAML Path         | Environment Variable      | Default |
|-------------------|---------------------------|---------|
| config.workers    | NUM_CONCURRENT_TASKS      | 10      |
| config.timeout    | REQUEST_TIMEOUT           | 30s     |
| config.skipTlsVerify | SKIP_TLS_VERIFY        | false   |
| config.baseUrl    | TARGET_URL                | (required) |
| config.duration   | TEST_DURATION             | (required) |
| load.target       | TARGET_RPS                | -       |

## Examples

### Example 1: All Defaults
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "5m"
  # workers: will use default 10
  # timeout: will use default 30s
load:
  model: "concurrent"  # default
scenarios:
  - name: "Test"
    # weight: will use default 1.0
    steps: [...]
```

### Example 2: YAML Overrides Defaults
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  workers: 50        # overrides default 10
  timeout: "60s"     # overrides default 30s
  duration: "5m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Test"
    weight: 2.0      # overrides default 1.0
    steps: [...]
```

### Example 3: Environment Overrides Everything
```bash
# YAML has workers: 50
# Environment has NUM_CONCURRENT_TASKS=100
# Result: 100 workers (env wins)

NUM_CONCURRENT_TASKS=100 \
TARGET_RPS=200 \
rust_loadtest --config test.yaml
```

### Example 4: Mixed Precedence
```yaml
# test.yaml
config:
  baseUrl: "https://api.example.com"
  workers: 50        # from YAML
  timeout: "60s"     # from YAML
  duration: "5m"
```

```bash
# Run with environment override
NUM_CONCURRENT_TASKS=100 rust_loadtest --config test.yaml

# Result:
# - baseUrl: from YAML (https://api.example.com)
# - workers: 100 (from ENV, overrides YAML's 50)
# - timeout: 60s (from YAML)
# - duration: 5m (from YAML)
```

## Best Practices

1. **Use YAML for base configuration**
   - Version control your test definitions
   - Document test scenarios
   - Set reasonable defaults

2. **Use environment variables for overrides**
   - CI/CD pipeline customization
   - Container/Kubernetes configuration
   - Quick parameter changes

3. **Rely on defaults for common settings**
   - Timeout, workers, scenario weights
   - Reduces config file verbosity
   - Sensible defaults for most use cases
"#
    }

    /// Print precedence documentation to stdout.
    pub fn print_documentation() {
        println!("{}", Self::documentation());
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_config_defaults() {
        let defaults = ConfigDefaults::new();

        assert_eq!(defaults.workers, 10);
        assert_eq!(defaults.timeout, Duration::from_secs(30));
        assert_eq!(defaults.skip_tls_verify, false);
        assert_eq!(defaults.scenario_weight, 1.0);
        assert_eq!(defaults.load_model, "concurrent");

        println!("✅ Config defaults are correct");
    }

    #[test]
    fn test_merge_workers_yaml_only() {
        // No env var set, should use YAML value
        let result = ConfigMerger::merge_workers(Some(50), "TEST_WORKERS_1");
        assert_eq!(result, 50);

        println!("✅ Merge workers from YAML works");
    }

    #[test]
    fn test_merge_workers_default_only() {
        // No env var, no YAML value, should use default
        let result = ConfigMerger::merge_workers(None, "TEST_WORKERS_2");
        assert_eq!(result, 10);

        println!("✅ Merge workers uses default when not specified");
    }

    #[test]
    fn test_merge_workers_env_override() {
        // Set environment variable
        env::set_var("TEST_WORKERS_3", "100");

        // Env should override YAML value
        let result = ConfigMerger::merge_workers(Some(50), "TEST_WORKERS_3");
        assert_eq!(result, 100);

        // Clean up
        env::remove_var("TEST_WORKERS_3");

        println!("✅ Environment variable overrides YAML for workers");
    }

    #[test]
    fn test_merge_timeout_yaml_only() {
        let result = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "TEST_TIMEOUT_1");
        assert_eq!(result, Duration::from_secs(60));

        println!("✅ Merge timeout from YAML works");
    }

    #[test]
    fn test_merge_timeout_default_only() {
        let result = ConfigMerger::merge_timeout(None, "TEST_TIMEOUT_2");
        assert_eq!(result, Duration::from_secs(30));

        println!("✅ Merge timeout uses default when not specified");
    }

    #[test]
    fn test_merge_timeout_env_override() {
        env::set_var("TEST_TIMEOUT_3", "90s");

        let result = ConfigMerger::merge_timeout(Some(Duration::from_secs(60)), "TEST_TIMEOUT_3");
        assert_eq!(result, Duration::from_secs(90));

        env::remove_var("TEST_TIMEOUT_3");

        println!("✅ Environment variable overrides YAML for timeout");
    }

    #[test]
    fn test_merge_skip_tls_verify() {
        // Default
        assert_eq!(
            ConfigMerger::merge_skip_tls_verify(None, "TEST_SKIP_TLS_1"),
            false
        );

        // YAML
        assert_eq!(
            ConfigMerger::merge_skip_tls_verify(Some(true), "TEST_SKIP_TLS_2"),
            true
        );

        // Env override
        env::set_var("TEST_SKIP_TLS_3", "true");
        assert_eq!(
            ConfigMerger::merge_skip_tls_verify(Some(false), "TEST_SKIP_TLS_3"),
            true
        );
        env::remove_var("TEST_SKIP_TLS_3");

        println!("✅ Skip TLS verify merging works");
    }

    #[test]
    fn test_merge_scenario_weight() {
        assert_eq!(ConfigMerger::merge_scenario_weight(None), 1.0);
        assert_eq!(ConfigMerger::merge_scenario_weight(Some(2.5)), 2.5);

        println!("✅ Scenario weight merging works");
    }

    #[test]
    fn test_merge_string_precedence() {
        // Default only
        let result = ConfigMerger::merge_string(None, "TEST_STR_1", "default".to_string());
        assert_eq!(result, "default");

        // YAML overrides default
        let result = ConfigMerger::merge_string(
            Some("yaml".to_string()),
            "TEST_STR_2",
            "default".to_string(),
        );
        assert_eq!(result, "yaml");

        // Env overrides YAML and default
        env::set_var("TEST_STR_3", "env");
        let result = ConfigMerger::merge_string(
            Some("yaml".to_string()),
            "TEST_STR_3",
            "default".to_string(),
        );
        assert_eq!(result, "env");
        env::remove_var("TEST_STR_3");

        println!("✅ String merging precedence works correctly");
    }

    #[test]
    fn test_merge_optional_string() {
        // No value
        assert_eq!(
            ConfigMerger::merge_optional_string(None, "TEST_OPT_STR_1"),
            None
        );

        // YAML value
        assert_eq!(
            ConfigMerger::merge_optional_string(Some("yaml".to_string()), "TEST_OPT_STR_2"),
            Some("yaml".to_string())
        );

        // Env overrides YAML
        env::set_var("TEST_OPT_STR_3", "env");
        assert_eq!(
            ConfigMerger::merge_optional_string(Some("yaml".to_string()), "TEST_OPT_STR_3"),
            Some("env".to_string())
        );
        env::remove_var("TEST_OPT_STR_3");

        println!("✅ Optional string merging works");
    }

    #[test]
    fn test_merge_rps() {
        // No value
        assert_eq!(ConfigMerger::merge_rps(None, "TEST_RPS_1"), None);

        // YAML value
        assert_eq!(ConfigMerger::merge_rps(Some(100.0), "TEST_RPS_2"), Some(100.0));

        // Env overrides YAML
        env::set_var("TEST_RPS_3", "200.5");
        assert_eq!(
            ConfigMerger::merge_rps(Some(100.0), "TEST_RPS_3"),
            Some(200.5)
        );
        env::remove_var("TEST_RPS_3");

        println!("✅ RPS merging works");
    }

    #[test]
    fn test_precedence_order() {
        env::set_var("TEST_PRECEDENCE", "env-value");

        // Test with all three sources
        let result = ConfigMerger::merge_string(
            Some("yaml-value".to_string()),
            "TEST_PRECEDENCE",
            "default-value".to_string(),
        );

        assert_eq!(result, "env-value");

        env::remove_var("TEST_PRECEDENCE");

        // Test with YAML and default (no env)
        let result = ConfigMerger::merge_string(
            Some("yaml-value".to_string()),
            "TEST_PRECEDENCE",
            "default-value".to_string(),
        );

        assert_eq!(result, "yaml-value");

        // Test with default only
        let result = ConfigMerger::merge_string(None, "TEST_PRECEDENCE", "default-value".to_string());

        assert_eq!(result, "default-value");

        println!("✅ Precedence order: env > yaml > default works correctly");
    }

    #[test]
    fn test_documentation_exists() {
        let docs = ConfigPrecedence::documentation();
        assert!(!docs.is_empty());
        assert!(docs.contains("Precedence"));
        assert!(docs.contains("Environment Variables"));
        assert!(docs.contains("Default Values"));

        println!("✅ Precedence documentation exists");
    }
}
