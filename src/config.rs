use std::env;
use tokio::time::Duration;

use crate::client::ClientConfig;
use crate::load_models::LoadModel;
use crate::utils::parse_duration_string;

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

impl Config {
    /// Loads configuration from environment variables.
    pub fn from_env() -> Result<Self, Box<dyn std::error::Error + Send + Sync>> {
        let target_url =
            env::var("TARGET_URL").expect("TARGET_URL environment variable must be set");

        let request_type = env::var("REQUEST_TYPE").unwrap_or_else(|_| "POST".to_string());

        let send_json = env::var("SEND_JSON")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        let json_payload = if send_json {
            Some(
                env::var("JSON_PAYLOAD")
                    .expect("JSON_PAYLOAD environment variable must be set when SEND_JSON=true"),
            )
        } else {
            None
        };

        let num_concurrent_tasks: usize = env::var("NUM_CONCURRENT_TASKS")
            .unwrap_or_else(|_| "10".to_string())
            .parse()
            .expect("NUM_CONCURRENT_TASKS must be a valid number");

        let test_duration_str = env::var("TEST_DURATION").unwrap_or_else(|_| "2h".to_string());
        let test_duration = parse_duration_string(&test_duration_str).map_err(|e| {
            format!(
                "Invalid TEST_DURATION format: '{}'. {}",
                test_duration_str, e
            )
        })?;

        let load_model = Self::parse_load_model(&test_duration_str)?;

        let skip_tls_verify = env::var("SKIP_TLS_VERIFY")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        let resolve_target_addr = env::var("RESOLVE_TARGET_ADDR").ok();
        let client_cert_path = env::var("CLIENT_CERT_PATH").ok();
        let client_key_path = env::var("CLIENT_KEY_PATH").ok();
        let custom_headers = env::var("CUSTOM_HEADERS").ok();

        Ok(Config {
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
        })
    }

    fn parse_load_model(
        test_duration_str: &str,
    ) -> Result<LoadModel, Box<dyn std::error::Error + Send + Sync>> {
        let model_type = env::var("LOAD_MODEL_TYPE").unwrap_or_else(|_| "Concurrent".to_string());

        match model_type.as_str() {
            "Concurrent" => Ok(LoadModel::Concurrent),
            "Rps" => {
                let target_rps: f64 = env::var("TARGET_RPS")
                    .expect("TARGET_RPS must be set for Rps model")
                    .parse()?;
                Ok(LoadModel::Rps { target_rps })
            }
            "RampRps" => {
                let min_rps: f64 = env::var("MIN_RPS")
                    .expect("MIN_RPS must be set for RampRps")
                    .parse()?;
                let max_rps: f64 = env::var("MAX_RPS")
                    .expect("MAX_RPS must be set for RampRps")
                    .parse()?;
                let ramp_duration_str = env::var("RAMP_DURATION")
                    .unwrap_or_else(|_| test_duration_str.to_string());
                let ramp_duration = parse_duration_string(&ramp_duration_str)?;
                Ok(LoadModel::RampRps {
                    min_rps,
                    max_rps,
                    ramp_duration,
                })
            }
            "DailyTraffic" => {
                let min_rps: f64 = env::var("DAILY_MIN_RPS")
                    .expect("DAILY_MIN_RPS must be set for DailyTraffic model")
                    .parse()?;
                let mid_rps: f64 = env::var("DAILY_MID_RPS")
                    .expect("DAILY_MID_RPS must be set for DailyTraffic model")
                    .parse()?;
                let max_rps: f64 = env::var("DAILY_MAX_RPS")
                    .expect("DAILY_MAX_RPS must be set for DailyTraffic model")
                    .parse()?;
                let cycle_duration_str = env::var("DAILY_CYCLE_DURATION")
                    .expect("DAILY_CYCLE_DURATION must be set for DailyTraffic model");
                let cycle_duration = parse_duration_string(&cycle_duration_str)?;

                let morning_ramp_ratio: f64 = env::var("MORNING_RAMP_RATIO")
                    .unwrap_or_else(|_| "0.125".to_string())
                    .parse()?;
                let peak_sustain_ratio: f64 = env::var("PEAK_SUSTAIN_RATIO")
                    .unwrap_or_else(|_| "0.167".to_string())
                    .parse()?;
                let mid_decline_ratio: f64 = env::var("MID_DECLINE_RATIO")
                    .unwrap_or_else(|_| "0.125".to_string())
                    .parse()?;
                let mid_sustain_ratio: f64 = env::var("MID_SUSTAIN_RATIO")
                    .unwrap_or_else(|_| "0.167".to_string())
                    .parse()?;
                let evening_decline_ratio: f64 = env::var("EVENING_DECLINE_RATIO")
                    .unwrap_or_else(|_| "0.167".to_string())
                    .parse()?;

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
            _ => panic!("Unknown LOAD_MODEL_TYPE: {}", model_type),
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

    /// Prints the configuration summary.
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
