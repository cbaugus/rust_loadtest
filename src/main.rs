#[macro_use]
extern crate lazy_static;

use reqwest;
use tokio::{self, time::{self, Duration}};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use prometheus::{Encoder, Gauge, IntCounter, IntCounterVec, Opts, Registry, TextEncoder};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::env;
use std::str::FromStr; // Needed for parsing enums from strings

// Define Prometheus metrics
lazy_static::lazy_static! {
    static ref REQUEST_TOTAL: IntCounter =
        IntCounter::new("requests_total", "Total number of HTTP requests made").unwrap();
    static ref REQUEST_STATUS_CODES: IntCounterVec =
        IntCounterVec::new(
            Opts::new("requests_status_codes_total", "Number of HTTP requests by status code"),
            &["status_code"]
        )
        .unwrap();
    static ref CONCURRENT_REQUESTS: Gauge =
        Gauge::new("concurrent_requests", "Number of HTTP requests currently in flight").unwrap();
}

// --- NEW: Enum for different load models ---
#[derive(Debug, PartialEq, Eq, Clone)]
enum LoadModel {
    Concurrent,
    Rps,
    RampRps,
}

impl FromStr for LoadModel {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "concurrent" => Ok(LoadModel::Concurrent),
            "rps" => Ok(LoadModel::Rps),
            "ramp-rps" => Ok(LoadModel::RampRps),
            _ => Err(format!("Invalid LOAD_MODEL: '{}'. Use 'concurrent', 'rps', or 'ramp-rps'.", s)),
        }
    }
}
// --- END NEW: Enum for different load models ---


// --- Function to parse the duration string (copy-pasted from previous answer) ---
fn parse_duration_string(s: &str) -> Result<Duration, String> {
    let s = s.trim();

    if s.is_empty() {
        return Err("Duration string cannot be empty".to_string());
    }

    let unit_char = s.chars().last().unwrap();
    let value_str = &s[0..s.len() - 1];

    let value = match u64::from_str(value_str) { // Ensure u64 here
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

    let client = reqwest::Client::new();

    // --- READ FROM ENVIRONMENT VARIABLES ---
    let url = env::var("TARGET_URL")
        .expect("TARGET_URL environment variable must be set");

    let num_concurrent_tasks_str = env::var("NUM_CONCURRENT_TASKS")
        .unwrap_or_else(|_| "10".to_string());
    let num_concurrent_tasks: usize = num_concurrent_tasks_str.parse()
        .expect("NUM_CONCURRENT_TASKS must be a valid number");

    let test_duration_str = env::var("TEST_DURATION")
        .unwrap_or_else(|_| "2h".to_string());
    let test_duration = parse_duration_string(&test_duration_str)
        .expect(&format!("Invalid TEST_DURATION format: '{}'. Use formats like '10m', '5h', '3d'.", test_duration_str));

    // --- NEW: Read new load model related environment variables ---
    let load_model_str = env::var("LOAD_MODEL")
        .unwrap_or_else(|_| "concurrent".to_string());
    let load_model: LoadModel = load_model_str.parse()
        .expect("Invalid LOAD_MODEL environment variable");

    let target_rps_str = env::var("TARGET_RPS")
        .unwrap_or_else(|_| "0".to_string()); // Default to 0, meaning no RPS limit if not set
    let target_rps: f64 = target_rps_str.parse()
        .expect("TARGET_RPS must be a valid number");

    let min_rps_str = env::var("MIN_RPS")
        .unwrap_or_else(|_| "0".to_string());
    let min_rps: f64 = min_rps_str.parse()
        .expect("MIN_RPS must be a valid number");

    let max_rps_str = env::var("MAX_RPS")
        .unwrap_or_else(|_| "0".to_string());
    let max_rps: f64 = max_rps_str.parse()
        .expect("MAX_RPS must be a valid number");
    // --- END NEW: Read new load model related environment variables ---


    println!("Starting load test:");
    println!("  Target URL: {}", url);
    println!("  Concurrent Tasks: {}", num_concurrent_tasks);
    println!("  Test Duration: {:?}", test_duration);
    println!("  (Total seconds: {})", test_duration.as_secs());
    println!("  Load Model: {:?}", load_model);
    match load_model {
        LoadModel::Rps => println!("  Target RPS: {}", target_rps),
        LoadModel::RampRps => {
            println!("  Ramping RPS from {} to {}", min_rps, max_rps);
        },
        _ => {}
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

    let mut handles = Vec::new();
    for i in 0..num_concurrent_tasks {
        let client_clone = client.clone();
        let url_clone = url.to_string();
        let test_duration_clone = test_duration.clone();
        let start_time_clone = start_time.clone();
        let load_model_clone = load_model.clone(); // Clone load model for each task
        let target_rps_clone = target_rps;
        let min_rps_clone = min_rps;
        let max_rps_clone = max_rps;

        let handle = tokio::spawn(async move {
            loop {
                // Check if the total test duration has passed for this task
                if time::Instant::now().duration_since(start_time_clone) >= test_duration_clone {
                    println!("Task {} stopping after duration limit.", i);
                    break; // Exit this task's loop
                }

                CONCURRENT_REQUESTS.inc();
                REQUEST_TOTAL.inc();

                match client_clone.get(&url_clone).send().await {
                    Ok(response) => {
                        let status = response.status().as_u16().to_string();
                        REQUEST_STATUS_CODES.with_label_values(&[&status]).inc();
                    },
                    Err(e) => {
                        REQUEST_STATUS_CODES.with_label_values(&["error"]).inc();
                        eprintln!("Task {}: Request to {} failed: {}", i, url_clone, e);
                    }
                }
                CONCURRENT_REQUESTS.dec();

                // --- NEW: Apply delay based on load model ---
                let mut delay_ms = 10; // Default small delay to prevent busy-loop

                match load_model_clone {
                    LoadModel::Rps => {
                        if target_rps_clone > 0.0 {
                            let rps_per_task = target_rps_clone / num_concurrent_tasks as f64;
                            if rps_per_task > 0.0 {
                                delay_ms = (1000.0 / rps_per_task) as u64;
                            } else {
                                delay_ms = u64::MAX; // Effectively stop if RPS per task is 0
                            }
                        }
                    },
                    LoadModel::RampRps => {
                        if max_rps_clone > 0.0 {
                            let elapsed_time_secs = time::Instant::now().duration_since(start_time_clone).as_secs_f64();
                            let total_duration_secs = test_duration_clone.as_secs_f64();

                            let mut current_target_rps = 0.0;

                            if total_duration_secs > 0.0 {
                                let one_third_duration = total_duration_secs / 3.0;

                                if elapsed_time_secs <= one_third_duration {
                                    // Ramp-up phase (first 1/3)
                                    current_target_rps = min_rps_clone + (max_rps_clone - min_rps_clone) * (elapsed_time_secs / one_third_duration);
                                } else if elapsed_time_secs <= 2.0 * one_third_duration {
                                    // Max load phase (middle 1/3)
                                    current_target_rps = max_rps_clone;
                                } else {
                                    // Ramp-down phase (last 1/3)
                                    let ramp_down_elapsed = elapsed_time_secs - 2.0 * one_third_duration;
                                    current_target_rps = max_rps_clone - (max_rps_clone - min_rps_clone) * (ramp_down_elapsed / one_third_duration);
                                    // Ensure it doesn't go below min_rps
                                    if current_target_rps < min_rps_clone {
                                        current_target_rps = min_rps_clone;
                                    }
                                }
                            } else {
                                current_target_rps = max_rps_clone; // If duration is 0, just use max_rps
                            }

                            // Ensure current_target_rps is not negative or extremely small before division
                            if current_target_rps < 0.0 {
                                current_target_rps = 0.0;
                            }

                            let rps_per_task = current_target_rps / num_concurrent_tasks as f64;
                            if rps_per_task > 0.0 {
                                delay_ms = (1000.0 / rps_per_task) as u64;
                            } else {
                                delay_ms = u64::MAX; // Effectively stop if RPS per task is 0 (or very close to it)
                            }
                        }
                    },
                    LoadModel::Concurrent => {
                        // No specific RPS-based delay, rely on `num_concurrent_tasks`
                        // and the default small delay.
                    }
                }
                tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                // --- END NEW: Apply delay based on load model ---
            }
        });
        handles.push(handle);
    }

    // Wait for the total test duration to pass
    tokio::time::sleep(test_duration).await;
    println!("Main test duration completed. Signalling tasks to stop.");

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
