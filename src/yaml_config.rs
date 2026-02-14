//! YAML configuration file support (Issue #37).
//!
//! This module provides YAML-based configuration as an alternative to
//! environment variables. YAML files enable version-controlled test plans,
//! reusable scenarios, and easier configuration management.

use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;
use std::time::Duration as StdDuration;
use thiserror::Error;

use crate::config_validation::{
    HttpMethodValidator, LoadModelValidator, RangeValidator, UrlValidator, ValidationContext,
};
use crate::config_version::VersionChecker;
use crate::load_models::LoadModel;
use crate::scenario::{Assertion, Extractor, RequestConfig, Scenario, Step, ThinkTime};

/// Errors that can occur when loading or parsing YAML configuration.
#[derive(Error, Debug)]
pub enum YamlConfigError {
    #[error("Failed to read config file: {0}")]
    FileRead(#[from] std::io::Error),

    #[error("Failed to parse YAML: {0}")]
    YamlParse(#[from] serde_yaml::Error),

    #[error("Invalid configuration: {0}")]
    Validation(String),

    #[error("Missing required field: {0}")]
    MissingField(String),
}

/// Duration format for YAML (e.g., "30s", "5m", "2h").
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum YamlDuration {
    Seconds(u64),
    String(String),
}

impl YamlDuration {
    pub fn to_std_duration(&self) -> Result<StdDuration, YamlConfigError> {
        match self {
            YamlDuration::Seconds(s) => Ok(StdDuration::from_secs(*s)),
            YamlDuration::String(s) => crate::utils::parse_duration_string(s)
                .map_err(|e| YamlConfigError::Validation(format!("Invalid duration '{}': {}", s, e))),
        }
    }
}

/// Metadata about the test configuration.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YamlMetadata {
    pub name: Option<String>,
    pub description: Option<String>,
    pub author: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

/// Global configuration settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlGlobalConfig {
    #[serde(rename = "baseUrl")]
    pub base_url: String,

    #[serde(default = "default_timeout")]
    pub timeout: YamlDuration,

    #[serde(default = "default_workers")]
    pub workers: usize,

    pub duration: YamlDuration,

    #[serde(rename = "skipTlsVerify", default)]
    pub skip_tls_verify: bool,

    #[serde(rename = "customHeaders")]
    pub custom_headers: Option<String>,
}

fn default_timeout() -> YamlDuration {
    YamlDuration::Seconds(30)
}

fn default_workers() -> usize {
    10
}

/// Load model configuration in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "model", rename_all = "lowercase")]
pub enum YamlLoadModel {
    Concurrent,
    Rps {
        target: f64,
    },
    Ramp {
        min: f64,
        max: f64,
        #[serde(rename = "rampDuration")]
        ramp_duration: YamlDuration,
    },
    #[serde(rename = "dailytraffic")]
    DailyTraffic {
        min: f64,
        mid: f64,
        max: f64,
        #[serde(rename = "cycleDuration")]
        cycle_duration: YamlDuration,
    },
}

impl YamlLoadModel {
    pub fn to_load_model(&self) -> Result<LoadModel, YamlConfigError> {
        match self {
            YamlLoadModel::Concurrent => Ok(LoadModel::Concurrent),
            YamlLoadModel::Rps { target } => Ok(LoadModel::Rps { target_rps: *target }),
            YamlLoadModel::Ramp { min, max, ramp_duration } => {
                Ok(LoadModel::RampRps {
                    min_rps: *min,
                    max_rps: *max,
                    ramp_duration: ramp_duration.to_std_duration()?,
                })
            }
            YamlLoadModel::DailyTraffic { min, mid, max, cycle_duration } => {
                Ok(LoadModel::DailyTraffic {
                    min_rps: *min,
                    mid_rps: *mid,
                    max_rps: *max,
                    cycle_duration: cycle_duration.to_std_duration()?,
                })
            }
        }
    }
}

/// Scenario definition in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlScenario {
    pub name: String,

    #[serde(default = "default_weight")]
    pub weight: f64,

    pub steps: Vec<YamlStep>,

    /// Optional data file for data-driven testing
    #[serde(rename = "dataFile")]
    pub data_file: Option<YamlDataFile>,

    /// Optional scenario-level configuration overrides
    #[serde(default)]
    pub config: YamlScenarioConfig,
}

/// Data file configuration for data-driven scenarios.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlDataFile {
    /// Path to the data file (CSV or JSON)
    pub path: String,

    /// Data file format (csv, json)
    #[serde(default = "default_data_format")]
    pub format: String,

    /// How to iterate through data (sequential, random, cycle)
    #[serde(default = "default_data_strategy")]
    pub strategy: String,
}

fn default_data_format() -> String {
    "csv".to_string()
}

fn default_data_strategy() -> String {
    "sequential".to_string()
}

/// Scenario-level configuration overrides.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct YamlScenarioConfig {
    /// Override global timeout for this scenario
    pub timeout: Option<YamlDuration>,

    /// Number of times to retry failed requests in this scenario
    #[serde(rename = "retryCount")]
    pub retry_count: Option<u32>,

    /// Delay between retries
    #[serde(rename = "retryDelay")]
    pub retry_delay: Option<YamlDuration>,
}

fn default_weight() -> f64 {
    1.0
}

/// Think time configuration in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum YamlThinkTime {
    /// Fixed think time (e.g., "3s")
    Fixed(YamlDuration),

    /// Random think time with min/max range
    Random {
        min: YamlDuration,
        max: YamlDuration,
    },
}

impl YamlThinkTime {
    pub fn to_think_time(&self) -> Result<crate::scenario::ThinkTime, YamlConfigError> {
        match self {
            YamlThinkTime::Fixed(duration) => {
                Ok(crate::scenario::ThinkTime::Fixed(duration.to_std_duration()?))
            }
            YamlThinkTime::Random { min, max } => {
                Ok(crate::scenario::ThinkTime::Random {
                    min: min.to_std_duration()?,
                    max: max.to_std_duration()?,
                })
            }
        }
    }
}

/// Step definition in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlStep {
    pub name: Option<String>,

    pub request: YamlRequest,

    #[serde(default)]
    pub extract: Vec<YamlExtractor>,

    #[serde(default)]
    pub assertions: Vec<YamlAssertion>,

    #[serde(rename = "thinkTime")]
    pub think_time: Option<YamlThinkTime>,
}

/// Request configuration in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlRequest {
    pub method: String,
    pub path: String,

    #[serde(rename = "queryParams")]
    pub query_params: Option<std::collections::HashMap<String, String>>,

    pub headers: Option<std::collections::HashMap<String, String>>,

    pub body: Option<String>,
}

/// Extractor definition in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum YamlExtractor {
    #[serde(rename = "jsonPath")]
    JsonPath {
        name: String,
        #[serde(rename = "jsonPath")]
        json_path: String,
    },
    Regex {
        name: String,
        regex: String,
    },
    Header {
        name: String,
        header: String,
    },
    Cookie {
        name: String,
        cookie: String,
    },
}

/// Assertion definition in YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum YamlAssertion {
    #[serde(rename = "statusCode")]
    StatusCode {
        expected: u16,
    },
    #[serde(rename = "responseTime")]
    ResponseTime {
        max: YamlDuration,
    },
    #[serde(rename = "jsonPath")]
    JsonPath {
        path: String,
        expected: Option<String>,
    },
    #[serde(rename = "bodyContains")]
    BodyContains {
        text: String,
    },
    #[serde(rename = "bodyMatches")]
    BodyMatches {
        regex: String,
    },
    #[serde(rename = "headerExists")]
    HeaderExists {
        header: String,
    },
}

/// Root YAML configuration structure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct YamlConfig {
    pub version: String,

    #[serde(default)]
    pub metadata: YamlMetadata,

    pub config: YamlGlobalConfig,

    pub load: YamlLoadModel,

    pub scenarios: Vec<YamlScenario>,
}

impl YamlConfig {
    /// Load configuration from a YAML file.
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, YamlConfigError> {
        let content = fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Parse configuration from a YAML string.
    pub fn from_str(content: &str) -> Result<Self, YamlConfigError> {
        let config: YamlConfig = serde_yaml::from_str(content)?;
        config.validate()?;
        Ok(config)
    }

    /// Validate the configuration using enhanced validation system.
    fn validate(&self) -> Result<(), YamlConfigError> {
        let mut ctx = ValidationContext::new();

        // Validate version using VersionChecker
        ctx.enter("version");
        if let Err(e) = VersionChecker::parse_and_validate(&self.version) {
            ctx.field_error(e.to_string());
        }
        ctx.exit();

        // Validate config section
        ctx.enter("config");

        // Validate base URL
        ctx.enter("baseUrl");
        if let Err(e) = UrlValidator::validate(&self.config.base_url) {
            ctx.field_error(e.to_string());
        }
        ctx.exit();

        // Validate workers
        ctx.enter("workers");
        if let Err(e) = RangeValidator::validate_positive_u64(self.config.workers as u64, "workers")
        {
            ctx.field_error(e.to_string());
        }
        if let Err(e) = RangeValidator::validate_u64(
            self.config.workers as u64,
            1,
            10000,
            "workers",
        ) {
            ctx.field_error(format!(
                "Workers should be between 1 and 10000, got: {}",
                self.config.workers
            ));
        }
        ctx.exit();

        ctx.exit(); // config

        // Validate load model
        ctx.enter("load");
        match &self.load {
            YamlLoadModel::Rps { target } => {
                if let Err(e) = LoadModelValidator::validate_rps(*target) {
                    ctx.field_error(e.to_string());
                }
            }
            YamlLoadModel::Ramp { min, max, .. } => {
                if let Err(e) = LoadModelValidator::validate_ramp(*min, *max) {
                    ctx.field_error(e.to_string());
                }
            }
            YamlLoadModel::DailyTraffic { min, mid, max, .. } => {
                if let Err(e) = LoadModelValidator::validate_daily_traffic(*min, *mid, *max) {
                    ctx.field_error(e.to_string());
                }
            }
            YamlLoadModel::Concurrent => {} // No validation needed
        }
        ctx.exit(); // load

        // Validate scenarios
        ctx.enter("scenarios");
        if self.scenarios.is_empty() {
            ctx.field_error("At least one scenario must be defined".to_string());
        }

        for (idx, scenario) in self.scenarios.iter().enumerate() {
            ctx.enter(&format!("[{}]", idx));
            ctx.enter("name");
            if scenario.name.is_empty() {
                ctx.field_error("Scenario name cannot be empty".to_string());
            }
            ctx.exit();

            // Validate weight
            ctx.enter("weight");
            if let Err(e) = RangeValidator::validate_positive_f64(scenario.weight, "weight") {
                ctx.field_error(e.to_string());
            }
            ctx.exit();

            // Validate steps
            ctx.enter("steps");
            if scenario.steps.is_empty() {
                ctx.field_error(format!(
                    "Scenario '{}' must have at least one step",
                    scenario.name
                ));
            }

            for (step_idx, step) in scenario.steps.iter().enumerate() {
                ctx.enter(&format!("[{}]", step_idx));
                ctx.enter("request");

                // Validate HTTP method
                ctx.enter("method");
                if let Err(e) = HttpMethodValidator::validate(&step.request.method) {
                    ctx.field_error(e.to_string());
                }
                ctx.exit();

                // Validate path
                ctx.enter("path");
                if step.request.path.is_empty() {
                    ctx.field_error("Request path cannot be empty".to_string());
                }
                ctx.exit();

                ctx.exit(); // request
                ctx.exit(); // step
            }

            ctx.exit(); // steps
            ctx.exit(); // scenario
        }
        ctx.exit(); // scenarios

        // Convert validation context to result
        ctx.into_result()
            .map_err(|e| YamlConfigError::Validation(e.to_string()))
    }

    /// Convert YAML scenarios to Scenario structs.
    pub fn to_scenarios(&self) -> Result<Vec<Scenario>, YamlConfigError> {
        let mut scenarios = Vec::new();

        for yaml_scenario in &self.scenarios {
            let mut steps = Vec::new();

            for (idx, yaml_step) in yaml_scenario.steps.iter().enumerate() {
                let step_name = yaml_step.name.clone()
                    .unwrap_or_else(|| format!("Step {}", idx + 1));

                // Build request config
                let mut headers = std::collections::HashMap::new();
                if let Some(yaml_headers) = &yaml_step.request.headers {
                    headers.extend(yaml_headers.clone());
                }

                // Build body with query params if present
                let path = if let Some(query_params) = &yaml_step.request.query_params {
                    let query_string: Vec<String> = query_params.iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect();
                    format!("{}?{}", yaml_step.request.path, query_string.join("&"))
                } else {
                    yaml_step.request.path.clone()
                };

                let request = RequestConfig {
                    method: yaml_step.request.method.clone(),
                    path,
                    body: yaml_step.request.body.clone(),
                    headers,
                };

                // Convert extractors
                let extractors = yaml_step.extract.iter()
                    .map(|e| self.convert_extractor(e))
                    .collect();

                // Convert assertions
                let assertions = yaml_step.assertions.iter()
                    .map(|a| self.convert_assertion(a))
                    .collect::<Result<Vec<_>, _>>()?;

                // Convert think time
                let think_time = if let Some(think_time_yaml) = &yaml_step.think_time {
                    Some(think_time_yaml.to_think_time()?)
                } else {
                    None
                };

                steps.push(Step {
                    name: step_name,
                    request,
                    extractions: extractors,
                    assertions,
                    think_time,
                });
            }

            scenarios.push(Scenario {
                name: yaml_scenario.name.clone(),
                weight: yaml_scenario.weight,
                steps,
            });
        }

        Ok(scenarios)
    }

    fn convert_extractor(&self, extractor: &YamlExtractor) -> Extractor {
        match extractor {
            YamlExtractor::JsonPath { name, json_path } => {
                Extractor::JsonPath {
                    var_name: name.clone(),
                    json_path: json_path.clone(),
                }
            }
            YamlExtractor::Regex { name, regex } => {
                Extractor::Regex {
                    var_name: name.clone(),
                    pattern: regex.clone(),
                }
            }
            YamlExtractor::Header { name, header } => {
                Extractor::Header {
                    var_name: name.clone(),
                    header_name: header.clone(),
                }
            }
            YamlExtractor::Cookie { name, cookie } => {
                Extractor::Cookie {
                    var_name: name.clone(),
                    cookie_name: cookie.clone(),
                }
            }
        }
    }

    fn convert_assertion(&self, assertion: &YamlAssertion) -> Result<Assertion, YamlConfigError> {
        match assertion {
            YamlAssertion::StatusCode { expected } => {
                Ok(Assertion::StatusCode(*expected))
            }
            YamlAssertion::ResponseTime { max } => {
                Ok(Assertion::ResponseTime(max.to_std_duration()?))
            }
            YamlAssertion::JsonPath { path, expected } => {
                Ok(Assertion::JsonPath {
                    path: path.clone(),
                    expected: expected.clone(),
                })
            }
            YamlAssertion::BodyContains { text } => {
                Ok(Assertion::BodyContains(text.clone()))
            }
            YamlAssertion::BodyMatches { regex } => {
                Ok(Assertion::BodyMatches(regex.clone()))
            }
            YamlAssertion::HeaderExists { header } => {
                Ok(Assertion::HeaderExists(header.clone()))
            }
        }
    }
}

impl Default for YamlConfig {
    fn default() -> Self {
        Self {
            version: "1.0".to_string(),
            metadata: YamlMetadata::default(),
            config: YamlGlobalConfig {
                base_url: "https://example.com".to_string(),
                timeout: YamlDuration::Seconds(30),
                workers: 10,
                duration: YamlDuration::Seconds(60),
                skip_tls_verify: false,
                custom_headers: None,
            },
            load: YamlLoadModel::Concurrent,
            scenarios: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_yaml() {
        let yaml = r#"
version: "1.0"
metadata:
  name: "Test Config"
config:
  baseUrl: "https://api.example.com"
  workers: 5
  duration: "1m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Test Scenario"
    steps:
      - request:
          method: "GET"
          path: "/health"
"#;

        let config = YamlConfig::from_str(yaml).unwrap();
        assert_eq!(config.version, "1.0");
        assert_eq!(config.config.base_url, "https://api.example.com");
        assert_eq!(config.config.workers, 5);
        assert_eq!(config.scenarios.len(), 1);
        assert_eq!(config.scenarios[0].name, "Test Scenario");
    }

    #[test]
    fn test_yaml_duration_parsing() {
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
"#;

        let config = YamlConfig::from_str(yaml).unwrap();
        let duration = config.config.duration.to_std_duration().unwrap();
        assert_eq!(duration, StdDuration::from_secs(30));

        let timeout = config.config.timeout.to_std_duration().unwrap();
        assert_eq!(timeout, StdDuration::from_secs(15));
    }

    #[test]
    fn test_validation_invalid_version() {
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
        assert!(result.unwrap_err().to_string().contains("Unsupported config version"));
    }

    #[test]
    fn test_validation_invalid_url() {
        let yaml = r#"
version: "1.0"
config:
  baseUrl: "invalid-url"
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
        assert!(result.unwrap_err().to_string().contains("Invalid base URL"));
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
        assert!(result.unwrap_err().to_string().contains("At least one scenario"));
    }

    #[test]
    fn test_scenario_conversion() {
        let yaml = r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "1m"
load:
  model: "concurrent"
scenarios:
  - name: "Test Flow"
    weight: 1.5
    steps:
      - name: "Step 1"
        request:
          method: "GET"
          path: "/api/test"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"
"#;

        let config = YamlConfig::from_str(yaml).unwrap();
        let scenarios = config.to_scenarios().unwrap();

        assert_eq!(scenarios.len(), 1);
        assert_eq!(scenarios[0].name, "Test Flow");
        assert_eq!(scenarios[0].weight, 1.5);
        assert_eq!(scenarios[0].steps.len(), 1);
        assert_eq!(scenarios[0].steps[0].name, "Step 1");
        assert_eq!(scenarios[0].steps[0].request.method, "GET");
        assert_eq!(scenarios[0].steps[0].assertions.len(), 1);
        assert!(scenarios[0].steps[0].think_time.is_some());
    }

    #[test]
    fn test_load_model_conversion() {
        let yaml = r#"
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

        let config = YamlConfig::from_str(yaml).unwrap();
        let load_model = config.load.to_load_model().unwrap();

        match load_model {
            LoadModel::RampRps { min_rps, max_rps, ramp_duration } => {
                assert_eq!(min_rps, 10.0);
                assert_eq!(max_rps, 100.0);
                assert_eq!(ramp_duration, StdDuration::from_secs(30));
            }
            _ => panic!("Expected RampRps load model"),
        }
    }
}
