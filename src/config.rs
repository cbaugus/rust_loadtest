use std::env;
use thiserror::Error;
use tokio::time::Duration;

use crate::client::ClientConfig;
use crate::load_models::LoadModel;
use crate::utils::parse_duration_string;

/// Configuration errors with descriptive messages.
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Missing required environment variable: {0}")]
    MissingEnvVar(String),

    #[error("Invalid value for {var}: {message}")]
    InvalidValue { var: String, message: String },

    #[error("mTLS configuration incomplete: both CLIENT_CERT_PATH and CLIENT_KEY_PATH must be set together, or neither")]
    IncompleteMtls,

    #[error("Load model '{model}' requires: {required}")]
    MissingLoadModelParams { model: String, required: String },

    #[error("Invalid duration format for {var}: {message}")]
    InvalidDuration { var: String, message: String },

    #[error("URL validation failed: {0}")]
    InvalidUrl(String),

    #[error("Parse error: {0}")]
    ParseError(String),
}

/// Main configuration for the load test.
#[derive(Debug, Clone)]
pub struct Config {
    pub target_url: String,
    pub request_type: String,
    pub send_json: bool,
    pub json_payload: Option<String>,
    pub num_concurrent_tasks: usize,
    pub test_duration: Duration,
    pub load_model: LoadModel,
    pub skip_tls_verify: bool,
    pub resolve_target_addr: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
    pub custom_headers: Option<String>,
}

/// Helper to get a required environment variable.
fn env_required(name: &str) -> Result<String, ConfigError> {
    env::var(name).map_err(|_| ConfigError::MissingEnvVar(name.into()))
}

/// Helper to parse an environment variable with a default value.
fn env_parse_or<T: std::str::FromStr>(name: &str, default: T) -> Result<T, ConfigError>
where
    T::Err: std::fmt::Display,
{
    match env::var(name) {
        Ok(val) => val.parse().map_err(|e: T::Err| ConfigError::InvalidValue {
            var: name.into(),
            message: e.to_string(),
        }),
        Err(_) => Ok(default),
    }
}

/// Helper to parse a boolean environment variable.
fn env_bool(name: &str, default: bool) -> bool {
    env::var(name)
        .unwrap_or_else(|_| default.to_string())
        .to_lowercase()
        == "true"
}

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, ConfigError> {
        let target_url = env_required("TARGET_URL")?;

        let request_type = env::var("REQUEST_TYPE").unwrap_or_else(|_| "POST".to_string());

        let send_json = env_bool("SEND_JSON", false);

        let json_payload = if send_json {
            Some(
                env_required("JSON_PAYLOAD").map_err(|_| ConfigError::MissingLoadModelParams {
                    model: "SEND_JSON=true".into(),
                    required: "JSON_PAYLOAD".into(),
                })?,
            )
        } else {
            None
        };

        let num_concurrent_tasks: usize = env_parse_or("NUM_CONCURRENT_TASKS", 10)?;

        let test_duration_str = env::var("TEST_DURATION").unwrap_or_else(|_| "2h".to_string());
        let test_duration = parse_duration_string(&test_duration_str).map_err(|e| {
            ConfigError::InvalidDuration {
                var: "TEST_DURATION".into(),
                message: e,
            }
        })?;

        let load_model = Self::parse_load_model(&test_duration_str)?;

        let skip_tls_verify = env_bool("SKIP_TLS_VERIFY", false);

        let resolve_target_addr = env::var("RESOLVE_TARGET_ADDR").ok();
        let client_cert_path = env::var("CLIENT_CERT_PATH").ok();
        let client_key_path = env::var("CLIENT_KEY_PATH").ok();
        let custom_headers = env::var("CUSTOM_HEADERS").ok();

        let config = Config {
            target_url,
            request_type,
            send_json,
            json_payload,
            num_concurrent_tasks,
            test_duration,
            load_model,
            skip_tls_verify,
            resolve_target_addr,
            client_cert_path,
            client_key_path,
            custom_headers,
        };

        config.validate()?;
        Ok(config)
    }

    fn parse_load_model(test_duration_str: &str) -> Result<LoadModel, ConfigError> {
        let model_type = env::var("LOAD_MODEL_TYPE").unwrap_or_else(|_| "Concurrent".to_string());

        match model_type.as_str() {
            "Concurrent" => Ok(LoadModel::Concurrent),
            "Rps" => {
                let target_rps: f64 = env_required("TARGET_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "Rps".into(),
                        required: "TARGET_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "TARGET_RPS".into(),
                        message: e.to_string(),
                    })?;
                Ok(LoadModel::Rps { target_rps })
            }
            "RampRps" => {
                let min_rps: f64 = env_required("MIN_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "RampRps".into(),
                        required: "MIN_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "MIN_RPS".into(),
                        message: e.to_string(),
                    })?;
                let max_rps: f64 = env_required("MAX_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "RampRps".into(),
                        required: "MAX_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "MAX_RPS".into(),
                        message: e.to_string(),
                    })?;
                let ramp_duration_str =
                    env::var("RAMP_DURATION").unwrap_or_else(|_| test_duration_str.to_string());
                let ramp_duration = parse_duration_string(&ramp_duration_str).map_err(|e| {
                    ConfigError::InvalidDuration {
                        var: "RAMP_DURATION".into(),
                        message: e,
                    }
                })?;
                Ok(LoadModel::RampRps {
                    min_rps,
                    max_rps,
                    ramp_duration,
                })
            }
            "DailyTraffic" => {
                let min_rps: f64 = env_required("DAILY_MIN_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "DailyTraffic".into(),
                        required: "DAILY_MIN_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "DAILY_MIN_RPS".into(),
                        message: e.to_string(),
                    })?;
                let mid_rps: f64 = env_required("DAILY_MID_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "DailyTraffic".into(),
                        required: "DAILY_MID_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "DAILY_MID_RPS".into(),
                        message: e.to_string(),
                    })?;
                let max_rps: f64 = env_required("DAILY_MAX_RPS")
                    .map_err(|_| ConfigError::MissingLoadModelParams {
                        model: "DailyTraffic".into(),
                        required: "DAILY_MAX_RPS".into(),
                    })?
                    .parse()
                    .map_err(|e: std::num::ParseFloatError| ConfigError::InvalidValue {
                        var: "DAILY_MAX_RPS".into(),
                        message: e.to_string(),
                    })?;
                let cycle_duration_str = env_required("DAILY_CYCLE_DURATION").map_err(|_| {
                    ConfigError::MissingLoadModelParams {
                        model: "DailyTraffic".into(),
                        required: "DAILY_CYCLE_DURATION".into(),
                    }
                })?;
                let cycle_duration = parse_duration_string(&cycle_duration_str).map_err(|e| {
                    ConfigError::InvalidDuration {
                        var: "DAILY_CYCLE_DURATION".into(),
                        message: e,
                    }
                })?;

                let morning_ramp_ratio: f64 = env_parse_or("MORNING_RAMP_RATIO", 0.125)?;
                let peak_sustain_ratio: f64 = env_parse_or("PEAK_SUSTAIN_RATIO", 0.167)?;
                let mid_decline_ratio: f64 = env_parse_or("MID_DECLINE_RATIO", 0.125)?;
                let mid_sustain_ratio: f64 = env_parse_or("MID_SUSTAIN_RATIO", 0.167)?;
                let evening_decline_ratio: f64 = env_parse_or("EVENING_DECLINE_RATIO", 0.167)?;

                let total_ratios = morning_ramp_ratio
                    + peak_sustain_ratio
                    + mid_decline_ratio
                    + mid_sustain_ratio
                    + evening_decline_ratio;
                if total_ratios > 1.0 {
                    eprintln!(
                        "Warning: Sum of DailyTraffic segment ratios exceeds 1.0 (Total: {}). Night sustain phase will be negative or very short.",
                        total_ratios
                    );
                }

                Ok(LoadModel::DailyTraffic {
                    min_rps,
                    mid_rps,
                    max_rps,
                    cycle_duration,
                    morning_ramp_ratio,
                    peak_sustain_ratio,
                    mid_decline_ratio,
                    mid_sustain_ratio,
                    evening_decline_ratio,
                })
            }
            _ => Err(ConfigError::InvalidValue {
                var: "LOAD_MODEL_TYPE".into(),
                message: format!(
                    "Unknown load model '{}'. Valid options: Concurrent, Rps, RampRps, DailyTraffic",
                    model_type
                ),
            }),
        }
    }

    /// Validates the configuration for consistency and correctness.
    fn validate(&self) -> Result<(), ConfigError> {
        // Validate URL format
        if !self.target_url.starts_with("http://") && !self.target_url.starts_with("https://") {
            return Err(ConfigError::InvalidUrl(
                "TARGET_URL must start with http:// or https://".into(),
            ));
        }

        // Validate num_concurrent_tasks
        if self.num_concurrent_tasks == 0 {
            return Err(ConfigError::InvalidValue {
                var: "NUM_CONCURRENT_TASKS".into(),
                message: "Must be greater than 0".into(),
            });
        }

        // Validate mTLS (both cert and key, or neither)
        if self.client_cert_path.is_some() != self.client_key_path.is_some() {
            return Err(ConfigError::IncompleteMtls);
        }

        Ok(())
    }

    /// Creates a default Config for testing purposes.
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Config {
            target_url: "https://example.com".into(),
            request_type: "GET".into(),
            send_json: false,
            json_payload: None,
            num_concurrent_tasks: 10,
            test_duration: Duration::from_secs(60),
            load_model: LoadModel::Concurrent,
            skip_tls_verify: false,
            resolve_target_addr: None,
            client_cert_path: None,
            client_key_path: None,
            custom_headers: None,
        }
    }

    /// Creates a ClientConfig from this Config.
    pub fn to_client_config(&self) -> ClientConfig {
        ClientConfig {
            skip_tls_verify: self.skip_tls_verify,
            resolve_target_addr: self.resolve_target_addr.clone(),
            client_cert_path: self.client_cert_path.clone(),
            client_key_path: self.client_key_path.clone(),
            custom_headers: self.custom_headers.clone(),
        }
    }

    /// Prints the configuration summary to stdout.
    pub fn print_summary(&self, parsed_headers: &reqwest::header::HeaderMap) {
        println!("Starting load test:");
        println!("  Target URL: {}", self.target_url);
        println!("  Request type: {}", self.request_type);
        println!("  Concurrent Tasks: {}", self.num_concurrent_tasks);
        println!("  Overall Test Duration: {:?}", self.test_duration);
        println!("  Load Model: {:?}", self.load_model);
        println!("  Skip TLS Verify: {}", self.skip_tls_verify);

        if self.client_cert_path.is_some() && self.client_key_path.is_some() {
            println!("  mTLS Enabled: Yes (using CLIENT_CERT_PATH and CLIENT_KEY_PATH)");
        } else {
            println!("  mTLS Enabled: No (CLIENT_CERT_PATH or CLIENT_KEY_PATH not set, or only one was set)");
        }

        if let Some(ref headers_str) = self.custom_headers {
            if !headers_str.is_empty() && !parsed_headers.is_empty() {
                println!("  Custom Headers Enabled: Yes");
                for (name, value) in parsed_headers.iter() {
                    println!(
                        "    {}: {}",
                        name,
                        value.to_str().unwrap_or("<non-ASCII or sensitive value>")
                    );
                }
            } else {
                println!("  Custom Headers Enabled: No (CUSTOM_HEADERS was set but resulted in no valid headers or was empty after parsing)");
            }
        } else {
            println!("  Custom Headers Enabled: No (CUSTOM_HEADERS not set)");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Mutex to serialize tests that modify environment variables,
    // since Rust runs tests in parallel within the same process.
    static ENV_MUTEX: Mutex<()> = Mutex::new(());

    // Helper to clear all load-test-related env vars before each test
    fn clear_env_vars() {
        let vars = [
            "TARGET_URL",
            "REQUEST_TYPE",
            "SEND_JSON",
            "JSON_PAYLOAD",
            "NUM_CONCURRENT_TASKS",
            "TEST_DURATION",
            "LOAD_MODEL_TYPE",
            "TARGET_RPS",
            "MIN_RPS",
            "MAX_RPS",
            "RAMP_DURATION",
            "DAILY_MIN_RPS",
            "DAILY_MID_RPS",
            "DAILY_MAX_RPS",
            "DAILY_CYCLE_DURATION",
            "MORNING_RAMP_RATIO",
            "PEAK_SUSTAIN_RATIO",
            "MID_DECLINE_RATIO",
            "MID_SUSTAIN_RATIO",
            "EVENING_DECLINE_RATIO",
            "SKIP_TLS_VERIFY",
            "RESOLVE_TARGET_ADDR",
            "CLIENT_CERT_PATH",
            "CLIENT_KEY_PATH",
            "CUSTOM_HEADERS",
        ];
        for var in vars {
            env::remove_var(var);
        }
    }

    #[test]
    fn defaults_with_minimal_config() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");

        let config = Config::from_env().unwrap();
        assert_eq!(config.target_url, "https://example.com");
        assert_eq!(config.request_type, "POST");
        assert!(!config.send_json);
        assert!(config.json_payload.is_none());
        assert_eq!(config.num_concurrent_tasks, 10);
        assert_eq!(config.test_duration, Duration::from_secs(7200)); // 2h default
        assert!(!config.skip_tls_verify);
        assert!(config.resolve_target_addr.is_none());
        assert!(config.client_cert_path.is_none());
        assert!(config.client_key_path.is_none());
        assert!(config.custom_headers.is_none());

        clear_env_vars();
    }

    #[test]
    fn concurrent_model_is_default() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");

        let config = Config::from_env().unwrap();
        assert!(
            matches!(config.load_model, LoadModel::Concurrent),
            "expected Concurrent, got {:?}",
            config.load_model
        );

        clear_env_vars();
    }

    #[test]
    fn rps_model_parsed() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("LOAD_MODEL_TYPE", "Rps");
        env::set_var("TARGET_RPS", "500.0");

        let config = Config::from_env().unwrap();
        match config.load_model {
            LoadModel::Rps { target_rps } => {
                assert!((target_rps - 500.0).abs() < 0.001);
            }
            other => panic!("expected Rps, got {:?}", other),
        }

        clear_env_vars();
    }

    #[test]
    fn ramp_rps_model_parsed() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("LOAD_MODEL_TYPE", "RampRps");
        env::set_var("MIN_RPS", "10.0");
        env::set_var("MAX_RPS", "1000.0");
        env::set_var("RAMP_DURATION", "1h");

        let config = Config::from_env().unwrap();
        match config.load_model {
            LoadModel::RampRps {
                min_rps,
                max_rps,
                ramp_duration,
            } => {
                assert!((min_rps - 10.0).abs() < 0.001);
                assert!((max_rps - 1000.0).abs() < 0.001);
                assert_eq!(ramp_duration, Duration::from_secs(3600));
            }
            other => panic!("expected RampRps, got {:?}", other),
        }

        clear_env_vars();
    }

    #[test]
    fn ramp_rps_defaults_duration_to_test_duration() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("LOAD_MODEL_TYPE", "RampRps");
        env::set_var("MIN_RPS", "10.0");
        env::set_var("MAX_RPS", "100.0");
        env::set_var("TEST_DURATION", "30m");
        // RAMP_DURATION not set, should default to TEST_DURATION

        let config = Config::from_env().unwrap();
        match config.load_model {
            LoadModel::RampRps { ramp_duration, .. } => {
                assert_eq!(ramp_duration, Duration::from_secs(1800));
            }
            other => panic!("expected RampRps, got {:?}", other),
        }

        clear_env_vars();
    }

    #[test]
    fn daily_traffic_model_parsed() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("LOAD_MODEL_TYPE", "DailyTraffic");
        env::set_var("DAILY_MIN_RPS", "10.0");
        env::set_var("DAILY_MID_RPS", "50.0");
        env::set_var("DAILY_MAX_RPS", "100.0");
        env::set_var("DAILY_CYCLE_DURATION", "1d");

        let config = Config::from_env().unwrap();
        match config.load_model {
            LoadModel::DailyTraffic {
                min_rps,
                mid_rps,
                max_rps,
                cycle_duration,
                ..
            } => {
                assert!((min_rps - 10.0).abs() < 0.001);
                assert!((mid_rps - 50.0).abs() < 0.001);
                assert!((max_rps - 100.0).abs() < 0.001);
                assert_eq!(cycle_duration, Duration::from_secs(86400));
            }
            other => panic!("expected DailyTraffic, got {:?}", other),
        }

        clear_env_vars();
    }

    #[test]
    fn custom_request_type() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("REQUEST_TYPE", "GET");

        let config = Config::from_env().unwrap();
        assert_eq!(config.request_type, "GET");

        clear_env_vars();
    }

    #[test]
    fn send_json_with_payload() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("SEND_JSON", "true");
        env::set_var("JSON_PAYLOAD", r#"{"key":"value"}"#);

        let config = Config::from_env().unwrap();
        assert!(config.send_json);
        assert_eq!(config.json_payload.unwrap(), r#"{"key":"value"}"#);

        clear_env_vars();
    }

    #[test]
    fn custom_concurrent_tasks() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("NUM_CONCURRENT_TASKS", "50");

        let config = Config::from_env().unwrap();
        assert_eq!(config.num_concurrent_tasks, 50);

        clear_env_vars();
    }

    #[test]
    fn custom_test_duration() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("TEST_DURATION", "30m");

        let config = Config::from_env().unwrap();
        assert_eq!(config.test_duration, Duration::from_secs(1800));

        clear_env_vars();
    }

    #[test]
    fn skip_tls_verify_true() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("SKIP_TLS_VERIFY", "true");

        let config = Config::from_env().unwrap();
        assert!(config.skip_tls_verify);

        clear_env_vars();
    }

    #[test]
    fn optional_fields_populated() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("RESOLVE_TARGET_ADDR", "example.com:1.2.3.4:443");
        env::set_var("CLIENT_CERT_PATH", "/path/to/cert.pem");
        env::set_var("CLIENT_KEY_PATH", "/path/to/key.pem");
        env::set_var("CUSTOM_HEADERS", "Authorization:Bearer token");

        let config = Config::from_env().unwrap();
        assert_eq!(
            config.resolve_target_addr.unwrap(),
            "example.com:1.2.3.4:443"
        );
        assert_eq!(config.client_cert_path.unwrap(), "/path/to/cert.pem");
        assert_eq!(config.client_key_path.unwrap(), "/path/to/key.pem");
        assert_eq!(config.custom_headers.unwrap(), "Authorization:Bearer token");

        clear_env_vars();
    }

    #[test]
    fn to_client_config_maps_fields() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("SKIP_TLS_VERIFY", "true");
        env::set_var("RESOLVE_TARGET_ADDR", "host:1.2.3.4:443");

        let config = Config::from_env().unwrap();
        let client_config = config.to_client_config();

        assert!(client_config.skip_tls_verify);
        assert_eq!(
            client_config.resolve_target_addr.unwrap(),
            "host:1.2.3.4:443"
        );
        assert!(client_config.client_cert_path.is_none());
        assert!(client_config.client_key_path.is_none());

        clear_env_vars();
    }

    #[test]
    fn missing_target_url_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();
        // TARGET_URL not set
        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::MissingEnvVar(ref var) if var == "TARGET_URL"),
            "expected MissingEnvVar(TARGET_URL), got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn unknown_load_model_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("LOAD_MODEL_TYPE", "InvalidModel");

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref var, .. } if var == "LOAD_MODEL_TYPE"),
            "expected InvalidValue for LOAD_MODEL_TYPE, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn send_json_without_payload_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("SEND_JSON", "true");
        // JSON_PAYLOAD not set

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::MissingLoadModelParams { .. }),
            "expected MissingLoadModelParams, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn invalid_url_format_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "not-a-valid-url");

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidUrl(_)),
            "expected InvalidUrl, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn zero_concurrent_tasks_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("NUM_CONCURRENT_TASKS", "0");

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::InvalidValue { ref var, .. } if var == "NUM_CONCURRENT_TASKS"),
            "expected InvalidValue for NUM_CONCURRENT_TASKS, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn incomplete_mtls_cert_only_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("CLIENT_CERT_PATH", "/path/to/cert.pem");
        // CLIENT_KEY_PATH not set

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::IncompleteMtls),
            "expected IncompleteMtls, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn incomplete_mtls_key_only_returns_error() {
        let _lock = ENV_MUTEX.lock().unwrap_or_else(|e| e.into_inner());
        clear_env_vars();

        env::set_var("TARGET_URL", "https://example.com");
        env::set_var("CLIENT_KEY_PATH", "/path/to/key.pem");
        // CLIENT_CERT_PATH not set

        let result = Config::from_env();
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, ConfigError::IncompleteMtls),
            "expected IncompleteMtls, got {:?}",
            err
        );
        clear_env_vars();
    }

    #[test]
    fn for_testing_creates_valid_config() {
        let config = Config::for_testing();
        assert_eq!(config.target_url, "https://example.com");
        assert_eq!(config.num_concurrent_tasks, 10);
        assert!(matches!(config.load_model, LoadModel::Concurrent));
    }
}
