extern crate lazy_static;

use reqwest;
use reqwest::header::{HeaderMap, HeaderName, HeaderValue}; // Added
use std::net::SocketAddr; // Added for DNS override
use tokio::{self, time::{self, Duration}};
use std::sync::{Arc, Mutex};
use prometheus::{Encoder, Gauge, IntCounter, IntCounterVec, Opts, Registry, TextEncoder, Histogram};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::env;
use std::str::FromStr; // Needed for parsing numbers from strings
use std::fs::File;
use std::io::Read;
use rustls_pemfile;

// Define Prometheus metrics
lazy_static::lazy_static! {
    static ref METRIC_NAMESPACE: String =
        env::var("METRIC_NAMESPACE").unwrap_or_else(|_| "rust_loadtest".to_string());

    static ref REQUEST_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new("requests_total", "Total number of HTTP requests made")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    static ref REQUEST_STATUS_CODES: IntCounterVec =
        IntCounterVec::new(
            Opts::new("requests_status_codes_total", "Number of HTTP requests by status code")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["status_code"]
        ).unwrap();

    static ref CONCURRENT_REQUESTS: Gauge =
        Gauge::with_opts(
            Opts::new("concurrent_requests", "Number of HTTP requests currently in flight")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    static ref REQUEST_DURATION_SECONDS: Histogram =
        Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "request_duration_seconds",
                "HTTP request latencies in seconds."
            ).namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();
}

// --- NEW: Enum for different load models with their data ---
#[derive(Debug, Clone)] // Removed PartialEq, Eq because f64 doesn't implement them reliably
pub enum LoadModel {
    Concurrent, // No specific RPS limit, just max concurrency
    Rps { target_rps: f64 }, // Fixed RPS target
    RampRps { // Linear ramp up/down
        min_rps: f64,
        max_rps: f64,
        ramp_duration: Duration, // Total duration for the ramp profile (e.g., 2 hours for the 1/3, 1/8, remainder pattern)
    },
    DailyTraffic { // Complex daily traffic pattern
        min_rps: f64,             // Base load (e.g., night-time traffic)
        mid_rps: f64,             // Mid-level load (e.g., afternoon traffic)
        max_rps: f64,             // Peak load (e.g., morning rush)
        cycle_duration: Duration, // Duration of one full daily cycle (e.g., Duration::from_hours(24))
        // Ratios defining the segments within one cycle. Sum of ratios should be <= 1.0
        morning_ramp_ratio: f64,    // From min_rps to max_rps
        peak_sustain_ratio: f64,    // Hold max_rps
        mid_decline_ratio: f64,     // From max_rps to mid_rps
        mid_sustain_ratio: f64,     // Hold mid_rps
        evening_decline_ratio: f64, // From mid_rps to min_rps
        // (Night sustain is implied by the end of the cycle)
    },
}

// --- NEW: calculate_current_rps method for LoadModel ---
impl LoadModel {
    // Helper function to calculate the current target RPS based on the model and elapsed time
    // This function will be called repeatedly by each worker task.
    pub fn calculate_current_rps(&self, elapsed_total_secs: f64, _overall_test_duration_secs: f64) -> f64 {
        match self {
            LoadModel::Concurrent => f64::MAX, // As fast as possible per task, limited by concurrency
            LoadModel::Rps { target_rps } => *target_rps,
            LoadModel::RampRps { min_rps, max_rps, ramp_duration } => {
                let total_ramp_secs = ramp_duration.as_secs_f64();
                let current_target_rps: f64; // Declare without initial assignment

                if total_ramp_secs > 0.0 {
                    let one_third_duration = total_ramp_secs / 3.0;

                    if elapsed_total_secs <= one_third_duration {
                        // Ramp-up phase (first 1/3)
                        current_target_rps = min_rps + (max_rps - min_rps) * (elapsed_total_secs / one_third_duration);

                        // current_target_rps = 1000 + (400000 - 1000) * (300 / 300);
                    } else if elapsed_total_secs <= 2.0 * one_third_duration {
                        // Max load phase (middle 1/3)
                        current_target_rps = *max_rps;
                    } else {
                        // Ramp-down phase (last 1/3)
                        let ramp_down_elapsed = elapsed_total_secs - 2.0 * one_third_duration;
                        let mut rps = max_rps - (max_rps - min_rps) * (ramp_down_elapsed / one_third_duration);
                        // Ensure it doesn't go below min_rps
                        if rps < *min_rps {
                            rps = *min_rps;
                        }
                        current_target_rps = rps;
                    }
                } else {
                    current_target_rps = *max_rps; // If duration is 0, just use max_rps
                }
                current_target_rps
            },
            LoadModel::DailyTraffic {
                min_rps,
                mid_rps,
                max_rps,
                cycle_duration,
                morning_ramp_ratio,
                peak_sustain_ratio,
                mid_decline_ratio,
                mid_sustain_ratio,
                evening_decline_ratio,
            } => {
                let cycle_duration_secs = cycle_duration.as_secs_f64();
                let time_in_cycle = elapsed_total_secs % cycle_duration_secs;

                let morning_ramp_end = cycle_duration_secs * morning_ramp_ratio;
                let peak_sustain_end = morning_ramp_end + (cycle_duration_secs * peak_sustain_ratio);
                let mid_decline_end = peak_sustain_end + (cycle_duration_secs * mid_decline_ratio);
                let mid_sustain_end = mid_decline_end + (cycle_duration_secs * mid_sustain_ratio);
                let evening_decline_end = mid_sustain_end + (cycle_duration_secs * evening_decline_ratio);

                let current_target_rps: f64; // Declare without initial assignment

                if cycle_duration_secs <= 0.0 {
                    current_target_rps = *max_rps; // Handle zero cycle duration, default to max
                } else if time_in_cycle < morning_ramp_end {
                    // Phase 1: Morning Ramp-up (min_rps to max_rps)
                    let ramp_elapsed = time_in_cycle;
                    let ramp_duration_segment = morning_ramp_end;
                    if ramp_duration_segment > 0.0 {
                        current_target_rps = min_rps + (max_rps - min_rps) * (ramp_elapsed / ramp_duration_segment);
                    } else {
                        current_target_rps = *max_rps; // Instant ramp to max if duration is zero
                    }
                } else if time_in_cycle < peak_sustain_end {
                    // Phase 2: Peak Sustain (max_rps)
                    current_target_rps = *max_rps;
                } else if time_in_cycle < mid_decline_end {
                    // Phase 3: Mid-Day Decline (max_rps to mid_rps)
                    let decline_elapsed = time_in_cycle - peak_sustain_end;
                    let decline_duration_segment = mid_decline_end - peak_sustain_end;
                    if decline_duration_segment > 0.0 {
                        current_target_rps = max_rps - (max_rps - mid_rps) * (decline_elapsed / decline_duration_segment);
                    } else {
                        current_target_rps = *mid_rps; // Instant decline to mid if duration is zero
                    }
                } else if time_in_cycle < mid_sustain_end {
                    // Phase 4: Mid-Day Sustain (mid_rps)
                    current_target_rps = *mid_rps;
                } else if time_in_cycle < evening_decline_end {
                    // Phase 5: Evening Decline (mid_rps to min_rps)
                    let decline_elapsed = time_in_cycle - mid_sustain_end;
                    let decline_duration_segment = evening_decline_end - mid_sustain_end;
                    if decline_duration_segment > 0.0 {
                        current_target_rps = mid_rps - (mid_rps - min_rps) * (decline_elapsed / decline_duration_segment);
                    } else {
                        current_target_rps = *min_rps; // Instant decline to min if duration is zero
                    }
                } else {
                    // Phase 6: Night Sustain (min_rps) - implicit until end of cycle
                    current_target_rps = *min_rps;
                }
                current_target_rps
            },
        }
    }
}
// --- END NEW: calculate_current_rps method ---


// --- Function to parse the duration string (copy-pasted from previous answer) ---
fn parse_duration_string(s: &str) -> Result<Duration, String> {
    let s = s.trim();

    if s.is_empty() {
        return Err("Duration string cannot be empty".to_string());
    }

    let unit_char = s.chars().last().unwrap();
    let value_str = &s[0..s.len() - 1];

    let value = match u64::from_str(value_str) {
        Ok(v) => v,
        Err(_) => return Err(format!("Invalid numeric value in duration: '{}'", value_str)),
    };

    match unit_char {
        'm' => Ok(Duration::from_secs(value * 60)),
        'h' => Ok(Duration::from_secs(value * 60 * 60)),
        'd' => Ok(Duration::from_secs(value * 24 * 60 * 60)),
        _ => Err(format!("Unknown duration unit: '{}'. Use 'm', 'h', or 'd'.", unit_char)),
    }
}
// --- END Function to parse the duration string ---


#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Register metrics with the default Prometheus registry
    prometheus::default_registry().register(Box::new(REQUEST_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_STATUS_CODES.clone()))?;
    prometheus::default_registry().register(Box::new(CONCURRENT_REQUESTS.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_DURATION_SECONDS.clone()))?;

    // --- NEW: Configure reqwest::Client for HTTPS and TLS verification ---
    let skip_tls_verify_str = env::var("SKIP_TLS_VERIFY").unwrap_or_else(|_| "false".to_string());
    let skip_tls_verify = skip_tls_verify_str.to_lowercase() == "true";

    let mut client_builder = reqwest::Client::builder();

    // --- NEW: DNS Override Configuration ---
    // Reads RESOLVE_TARGET_ADDR="hostname:ip_address:port"
    // Example: "example.com:192.168.1.10:8080"
    // This means any request to "example.com" (regardless of port in URL)
    // will be directed to 192.168.1.10:8080.
    if let Ok(resolve_str) = env::var("RESOLVE_TARGET_ADDR") {
        if !resolve_str.is_empty() {
            println!("Attempting to apply DNS override from RESOLVE_TARGET_ADDR: {}", resolve_str);
            let parts: Vec<&str> = resolve_str.split(':').collect();
            if parts.len() == 3 {
                let hostname_to_override = parts[0].trim();
                let ip_to_resolve_to = parts[1].trim();
                let port_to_connect_to_str = parts[2].trim();

                if hostname_to_override.is_empty() {
                    return Err("RESOLVE_TARGET_ADDR: hostname part cannot be empty. Format: 'hostname:ip:port'".into());
                }
                if ip_to_resolve_to.is_empty() {
                    return Err("RESOLVE_TARGET_ADDR: IP address part cannot be empty. Format: 'hostname:ip:port'".into());
                }
                if port_to_connect_to_str.is_empty() {
                    return Err("RESOLVE_TARGET_ADDR: port part cannot be empty. Format: 'hostname:ip:port'".into());
                }

                match port_to_connect_to_str.parse::<u16>() {
                    Ok(port_to_connect_to) => {
                        let socket_addr_str = format!("{}:{}", ip_to_resolve_to, port_to_connect_to);
                        match socket_addr_str.parse::<SocketAddr>() {
                            Ok(socket_addr) => {
                                client_builder = client_builder.resolve(hostname_to_override, socket_addr);
                                println!("Successfully configured DNS override: '{}' will resolve to {}", hostname_to_override, socket_addr);
                            }
                            Err(e) => {
                                return Err(format!("Failed to parse IP/Port '{}' into SocketAddr for RESOLVE_TARGET_ADDR: {}. Ensure IP and port are valid. Format: 'hostname:ip:port'", socket_addr_str, e).into());
                            }
                        }
                    }
                    Err(e) => {
                        return Err(format!("Failed to parse port '{}' in RESOLVE_TARGET_ADDR: {}. Must be a valid u16. Format: 'hostname:ip:port'", port_to_connect_to_str, e).into());
                    }
                }
            } else {
                // RESOLVE_TARGET_ADDR is set and not empty, but format is wrong.
                return Err(format!("RESOLVE_TARGET_ADDR environment variable ('{}') is not in the expected format 'hostname:ip:port'", resolve_str).into());
            }
        } else {
            // RESOLVE_TARGET_ADDR is set but empty.
            println!("RESOLVE_TARGET_ADDR is set but empty, no DNS override will be applied.");
        }
        // If RESOLVE_TARGET_ADDR is not set at all, env::var("RESOLVE_TARGET_ADDR") returns Err,
        // and this whole 'if let' block is skipped, which is the correct behavior (no override).
    }
    // --- END NEW: DNS Override Configuration ---

    // --- mTLS Configuration ---
    let client_cert_path_env = env::var("CLIENT_CERT_PATH").ok();
    let client_key_path_env = env::var("CLIENT_KEY_PATH").ok();

    if let (Some(cert_path), Some(key_path)) = (client_cert_path_env.as_ref(), client_key_path_env.as_ref()) {
        println!("Attempting to load mTLS certificate from: {}", cert_path);
        println!("Attempting to load mTLS private key from: {}", key_path);

        let mut cert_file = File::open(cert_path)
            .map_err(|e| format!("Failed to open client certificate file '{}': {}", cert_path, e))?;
        let mut cert_pem_buf = Vec::new();
        cert_file.read_to_end(&mut cert_pem_buf)
            .map_err(|e| format!("Failed to read client certificate file '{}': {}", cert_path, e))?;

        let mut key_file = File::open(key_path)
            .map_err(|e| format!("Failed to open client key file '{}': {}", key_path, e))?;
        let mut key_pem_buf = Vec::new();
        key_file.read_to_end(&mut key_pem_buf)
            .map_err(|e| format!("Failed to read client key file '{}': {}", key_path, e))?;

        // Validate certificate PEM
        let mut cert_pem_cursor = std::io::Cursor::new(cert_pem_buf.as_slice());
        let certs_result: Vec<_> = rustls_pemfile::certs(&mut cert_pem_cursor).collect();
        if certs_result.is_empty() {
            return Err(format!("No PEM certificates found in {}", cert_path).into());
        }
        for cert in certs_result {
            if let Err(e) = cert {
                return Err(format!("Failed to parse PEM certificates from '{}': {}", cert_path, e).into());
            }
        }

        // Validate private key PEM (must be PKCS#8)
        let mut key_pem_cursor = std::io::Cursor::new(key_pem_buf.as_slice());
        let keys_result: Vec<_> = rustls_pemfile::pkcs8_private_keys(&mut key_pem_cursor).collect();
        if keys_result.is_empty() {
            return Err(format!("No PKCS#8 private keys found in '{}'. Ensure the file contains a valid PEM-encoded PKCS#8 private key.", key_path).into());
        }
        for key in keys_result {
            if let Err(e) = key {
                return Err(format!("Failed to parse private key from '{}' as PKCS#8: {}. Please ensure the key is PEM-encoded and in PKCS#8 format.", key_path, e).into());
            }
        }

        // Combine certificate PEM and key PEM into one buffer for reqwest::Identity.
        let mut combined_pem_buf = Vec::new();
        combined_pem_buf.extend_from_slice(&cert_pem_buf);
        if !cert_pem_buf.ends_with(b"\n") && !key_pem_buf.starts_with(b"\n") {
            combined_pem_buf.push(b'\n'); // Add a newline separator if not present
        }
        combined_pem_buf.extend_from_slice(&key_pem_buf);

        let identity = reqwest::Identity::from_pem(&combined_pem_buf)
            .map_err(|e| format!("Failed to create reqwest::Identity from PEM (cert+key): {}. Ensure the key is PKCS#8 and the certificate is valid.", e))?;

        client_builder = client_builder.identity(identity);
        println!("Successfully configured mTLS with client certificate and key.");

    } else if client_cert_path_env.is_some() != client_key_path_env.is_some() {
        // Only one of the two paths is set, which is an error
        if client_cert_path_env.is_some() {
            return Err("CLIENT_CERT_PATH is set, but CLIENT_KEY_PATH is missing for mTLS.".into());
        } else {
            return Err("CLIENT_KEY_PATH is set, but CLIENT_CERT_PATH is missing for mTLS.".into());
        }
    }
    // --- END mTLS Configuration ---

    // --- NEW: Custom Headers Configuration ---
    let custom_headers_str = env::var("CUSTOM_HEADERS").unwrap_or_else(|_| "".to_string());
    let mut parsed_headers = HeaderMap::new();

    if !custom_headers_str.is_empty() {
        println!("Attempting to parse CUSTOM_HEADERS: {}", custom_headers_str);
        for header_pair_str in custom_headers_str.split(',') {
            let header_pair_str_trimmed = header_pair_str.trim();
            if header_pair_str_trimmed.is_empty() {
                continue; // Skip empty parts (e.g., due to trailing comma or multiple commas)
            }
            let parts: Vec<&str> = header_pair_str_trimmed.splitn(2, ':').collect();
            if parts.len() == 2 {
                let name_str = parts[0].trim();
                let value_str = parts[1].trim();

                if name_str.is_empty() {
                    return Err(format!("Invalid header format: Header name cannot be empty in '{}'.", header_pair_str_trimmed).into());
                }

                let header_name = HeaderName::from_str(name_str)
                    .map_err(|e| format!("Invalid header name: {}. Name: '{}'", e, name_str))?;
                let header_value = HeaderValue::from_str(value_str)
                    .map_err(|e| format!("Invalid header value for '{}': {}. Value: '{}'", name_str, e, value_str))?;
                parsed_headers.insert(header_name, header_value);
            } else {
                return Err(format!("Invalid header format in CUSTOM_HEADERS: '{}'. Expected 'Name:Value'.", header_pair_str_trimmed).into());
            }
        }
    }

    // Apply headers to client_builder if any were parsed
    if !parsed_headers.is_empty() {
        client_builder = client_builder.default_headers(parsed_headers.clone()); // Clone for logging later
        println!("Successfully configured custom default headers.");
    }
    // --- END NEW: Custom Headers Configuration ---

    let client = if skip_tls_verify {
        println!("WARNING: Skipping TLS certificate verification.");
        client_builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true) // Often needed with invalid certs
            .build()?
    } else {
        client_builder.build()?
    };
    // --- END NEW: Configure reqwest::Client ---

    // --- READ FROM ENVIRONMENT VARIABLES ---
    let url = env::var("TARGET_URL")
        .expect("TARGET_URL environment variable must be set");

    // --- NEW: Optionally send JSON payload ---
    let send_json = env::var("SEND_JSON").unwrap_or_else(|_| "false".to_string()).to_lowercase() == "true";
    let json_payload = if send_json {
        Some(env::var("JSON_PAYLOAD")
            .expect("JSON_PAYLOAD environment variable must be set when SEND_JSON=true"))
    } else {
        None
    };

    let num_concurrent_tasks_str = env::var("NUM_CONCURRENT_TASKS")
        .unwrap_or_else(|_| "10".to_string());
    let num_concurrent_tasks: usize = num_concurrent_tasks_str.parse()
        .expect("NUM_CONCURRENT_TASKS must be a valid number");

    let overall_test_duration_str = env::var("TEST_DURATION")
        .unwrap_or_else(|_| "2h".to_string());
    let overall_test_duration = parse_duration_string(&overall_test_duration_str)
        .expect(&format!("Invalid TEST_DURATION format: '{}'. Use formats like '10m', '5h', '3d'.", overall_test_duration_str));


    // --- NEW: Load Model Configuration from Environment Variables ---
    let load_model = {
        let model_type = env::var("LOAD_MODEL_TYPE")
            .unwrap_or_else(|_| "Concurrent".to_string()); // Default to Concurrent

        match model_type.as_str() {
            "Concurrent" => LoadModel::Concurrent,
            "Rps" => {
                let target_rps: f64 = env::var("TARGET_RPS")
                    .expect("TARGET_RPS must be set for Rps model").parse()?;
                LoadModel::Rps { target_rps }
            },
            "RampRps" => {
                let min_rps: f64 = env::var("MIN_RPS")
                    .expect("MIN_RPS must be set for RampRps").parse()?;
                let max_rps: f64 = env::var("MAX_RPS")
                    .expect("MAX_RPS must be set for RampRps").parse()?;
                let ramp_duration_str = env::var("RAMP_DURATION") // Use RAMP_DURATION for this specific model's ramp
                    .unwrap_or_else(|_| overall_test_duration_str.clone()).to_string(); // Default to overall test duration
                let ramp_duration = parse_duration_string(&ramp_duration_str)?;
                LoadModel::RampRps { min_rps, max_rps, ramp_duration }
            },
            "DailyTraffic" => {
                let min_rps: f64 = env::var("DAILY_MIN_RPS")
                    .expect("DAILY_MIN_RPS must be set for DailyTraffic model").parse()?;
                let mid_rps: f64 = env::var("DAILY_MID_RPS")
                    .expect("DAILY_MID_RPS must be set for DailyTraffic model").parse()?;
                let max_rps: f64 = env::var("DAILY_MAX_RPS")
                    .expect("DAILY_MAX_RPS must be set for DailyTraffic model").parse()?;
                let cycle_duration_str = env::var("DAILY_CYCLE_DURATION")
                    .expect("DAILY_CYCLE_DURATION must be set for DailyTraffic model");
                let cycle_duration = parse_duration_string(&cycle_duration_str)?;

                // Ratios for DailyTraffic segments (sum should be <= 1.0)
                let morning_ramp_ratio: f64 = env::var("MORNING_RAMP_RATIO").unwrap_or_else(|_| "0.125".to_string()).parse()?;
                let peak_sustain_ratio: f64 = env::var("PEAK_SUSTAIN_RATIO").unwrap_or_else(|_| "0.167".to_string()).parse()?;
                let mid_decline_ratio: f64 = env::var("MID_DECLINE_RATIO").unwrap_or_else(|_| "0.125".to_string()).parse()?;
                let mid_sustain_ratio: f64 = env::var("MID_SUSTAIN_RATIO").unwrap_or_else(|_| "0.167".to_string()).parse()?;
                let evening_decline_ratio: f64 = env::var("EVENING_DECLINE_RATIO").unwrap_or_else(|_| "0.167".to_string()).parse()?;

                // Basic validation of ratios
                let total_ratios = morning_ramp_ratio + peak_sustain_ratio + mid_decline_ratio + mid_sustain_ratio + evening_decline_ratio;
                if total_ratios > 1.0 {
                    eprintln!("Warning: Sum of DailyTraffic segment ratios exceeds 1.0 (Total: {}). Night sustain phase will be negative or very short.", total_ratios);
                }

                LoadModel::DailyTraffic {
                    min_rps, mid_rps, max_rps, cycle_duration,
                    morning_ramp_ratio, peak_sustain_ratio, mid_decline_ratio,
                    mid_sustain_ratio, evening_decline_ratio,
                }
            },
            _ => panic!("Unknown LOAD_MODEL_TYPE: {}", model_type),
        }
    };
    // --- END NEW: Load Model Configuration ---

    // --- NEW: Optionally change request type ---
    let request_type = env::var("REQUEST_TYPE").unwrap_or_else(|_| "POST".to_string());

    println!("Starting load test:");
    println!("  Target URL: {}", url);
    println!("  Request type: {}", request_type);
    println!("  Concurrent Tasks: {}", num_concurrent_tasks);
    println!("  Overall Test Duration: {:?}", overall_test_duration);
    println!("  Load Model: {:?}", load_model);
    println!("  Skip TLS Verify: {}", skip_tls_verify);
    if env::var("CLIENT_CERT_PATH").is_ok() && env::var("CLIENT_KEY_PATH").is_ok() {
        println!("  mTLS Enabled: Yes (using CLIENT_CERT_PATH and CLIENT_KEY_PATH)");
    } else {
        println!("  mTLS Enabled: No (CLIENT_CERT_PATH or CLIENT_KEY_PATH not set, or only one was set)");
    }

    if !custom_headers_str.is_empty() {
        if !parsed_headers.is_empty() {
            println!("  Custom Headers Enabled: Yes");
            for (name, value) in parsed_headers.iter() {
                println!("    {}: {}", name, value.to_str().unwrap_or("<non-ASCII or sensitive value>"));
            }
        } else {
             println!("  Custom Headers Enabled: No (CUSTOM_HEADERS was set but resulted in no valid headers or was empty after parsing)");
        }
    } else {
        println!("  Custom Headers Enabled: No (CUSTOM_HEADERS not set)");
    }


    // Start the Prometheus metrics HTTP server in a separate Tokio task
    let metrics_port = 9090; // Default Prometheus scrape port
    let metrics_addr = ([0, 0, 0, 0], metrics_port).into();

    let registry_arc = Arc::new(Mutex::new(prometheus::default_registry().clone()));

    let serve_metrics = {
        let registry = registry_arc.clone();
        async move {
            let make_svc = make_service_fn(move |_conn| {
                let registry_clone = registry.clone();
                async move {
                    Ok::<_, hyper::Error>(service_fn(move |req| {
                        let registry_clone_inner = registry_clone.clone();
                        async move {
                            metrics_handler(req, registry_clone_inner).await
                        }
                    }))
                }
            });

            let server = Server::bind(&metrics_addr).serve(make_svc);
            println!("Prometheus metrics server listening on http://0.0.0.0:{}", metrics_port);
            if let Err(e) = server.await {
                eprintln!("Metrics server error: {}", e);
            }
        }
    };
    tokio::spawn(serve_metrics);

    // Main loop to run for a duration
    let start_time = time::Instant::now();
    let _overall_test_duration_secs = overall_test_duration.as_secs_f64(); // Fixed unused variable warnings

    let mut handles = Vec::new();
    for i in 0..num_concurrent_tasks {
        let client_clone = client.clone();
        let url_clone = url.to_string();
        let overall_test_duration_clone = overall_test_duration.clone();
        let start_time_clone = start_time.clone();
        let load_model_clone = load_model.clone();
        let num_concurrent_tasks_clone = num_concurrent_tasks.clone();
        let send_json_clone = send_json;
        let json_payload_clone = json_payload.clone();

        let handle = tokio::spawn(async move {
            loop {
                let elapsed_total_secs = time::Instant::now().duration_since(start_time_clone).as_secs_f64();

                // Check if the total test duration has passed for this task
                if elapsed_total_secs >= overall_test_duration_clone.as_secs_f64() {
                    println!("Task {} stopping after overall duration limit.", i);
                    break; // Exit this task's loop
                }

                // Calculate current target RPS based on the chosen load model and elapsed time
                let current_target_rps = load_model_clone.calculate_current_rps(elapsed_total_secs, overall_test_duration_clone.as_secs_f64());

                // Calculate delay per task to achieve the current_target_rps
                // Handle division by zero or extremely small RPS
                let delay_ms = if current_target_rps > 0.0 {
                    (num_concurrent_tasks_clone as f64 * 1000.0 / current_target_rps).round() as u64
                } else {
                    u64::MAX // Effectively stop requests for this task if RPS is 0
                };

                // Add metrics tracking as before
                CONCURRENT_REQUESTS.inc();
                REQUEST_TOTAL.inc();

                let request_start_time = time::Instant::now(); // Start timer

                // --- CHANGED: Support GET request type ---
                if request_type == "GET" {
                    let req = client_client.get(&url_clone)
                else if request_type == "POST" {
                    // --- CHANGED: Conditionally send POST with or without JSON ---
                    let req = client_clone.post(&url_clone);
                    let req = if send_json_clone {
                        req.header("Content-Type", "application/json")
                            .body(json_payload_clone.clone().unwrap())
                    } else {
                        req
                    };
                } else {
                    eprintln!("Request type {} not currently supported", request_type);
                }

                match req.send().await {
                    Ok(response) => {
                        let status = response.status().as_u16().to_string();
                        REQUEST_STATUS_CODES.with_label_values(&[&status]).inc();
                        // Do not save the JWT token, just drop the response
                    },
                    Err(e) => {
                        REQUEST_STATUS_CODES.with_label_values(&["error"]).inc();
                        eprintln!("Task {}: Request to {} failed: {}", i, url_clone, e);
                    }
                }
                REQUEST_DURATION_SECONDS.observe(request_start_time.elapsed().as_secs_f64()); // Observe duration
                CONCURRENT_REQUESTS.dec();

                // Apply the calculated delay
                if delay_ms > 0 && delay_ms != u64::MAX {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                } else if delay_ms == u64::MAX {
                    tokio::time::sleep(Duration::from_secs(3600)).await; // Sleep for a very long time if RPS is 0
                }
                // If delay_ms is 0, no sleep, burst as fast as possible.
            }
        });
        handles.push(handle);
    }

    // Wait for the total test duration to pass
    tokio::time::sleep(overall_test_duration).await;
    println!("Main test duration completed. Signalling tasks to stop.");

    // Add a brief pause to allow in-flight metrics to be updated by worker threads
    // before we collect and print the final metrics.
    // This is a pragmatic approach; for very high precision, a more complex
    // synchronization mechanism with worker tasks would be needed.
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("Collecting and printing final metrics...");

    // Gather and print final metrics
    let final_metrics_output = {
        let encoder = TextEncoder::new();
        let metric_families = registry_arc.lock().unwrap().gather();
        let mut buffer = Vec::new();
        encoder.encode(&metric_families, &mut buffer).unwrap();
        String::from_utf8(buffer).unwrap_or_else(|e| {
            eprintln!("Error encoding metrics to UTF-8: {}", e);
            String::from("# ERROR ENCODING METRICS TO UTF-8")
        })
    };

    println!("\n--- FINAL METRICS ---\n{}", final_metrics_output);
    println!("--- END OF FINAL METRICS ---\n");

    println!("Pausing for 2 minutes to allow final Prometheus scrape...");
    tokio::time::sleep(Duration::from_secs(120)).await;
    println!("2-minute pause complete. Exiting.");

    // The program will exit here, and all spawned tasks will be dropped.
    Ok(())
}


async fn metrics_handler(
    _req: Request<Body>,
    registry: Arc<Mutex<Registry>>,
) -> Result<Response<Body>, hyper::Error> {
    let encoder = TextEncoder::new();
    let metric_families = registry.lock().unwrap().gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();

    let response = Response::builder()
        .status(200)
        .header("Content-Type", encoder.format_type())
        .body(Body::from(buffer))
        .unwrap();

    Ok(response)
}
