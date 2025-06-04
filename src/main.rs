#[macro_use]
extern crate lazy_static;

use reqwest;
use tokio::{self, time::{self, Duration}};
use std::collections::HashMap; // Not used in this snippet, but kept if you need it elsewhere
use std::sync::{Arc, Mutex};
use prometheus::{Encoder, Gauge, IntCounter, IntCounterVec, Opts, Registry, TextEncoder};
use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use std::env;
use std::str::FromStr; // Needed for parsing numbers from strings


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
    pub fn calculate_current_rps(&self, elapsed_total_secs: f64, overall_test_duration_secs: f64) -> f64 {
        match self {
            LoadModel::Concurrent => f64::MAX, // As fast as possible per task, limited by concurrency
            LoadModel::Rps { target_rps } => *target_rps,
            LoadModel::RampRps { min_rps, max_rps, ramp_duration } => {
                let total_ramp_secs = ramp_duration.as_secs_f64(); // This should be overall_test_duration_secs usually

                let mut current_target_rps = 0.0;

                if total_ramp_secs > 0.0 {
                    let one_third_duration = total_ramp_secs / 3.0;

                    if elapsed_total_secs <= one_third_duration {
                        // Ramp-up phase (first 1/3)
                        current_target_rps = min_rps + (max_rps - min_rps) * (elapsed_total_secs / one_third_duration);
                    } else if elapsed_total_secs <= 2.0 * one_third_duration {
                        // Max load phase (middle 1/3)
                        current_target_rps = *max_rps;
                    } else {
                        // Ramp-down phase (last 1/3)
                        let ramp_down_elapsed = elapsed_total_secs - 2.0 * one_third_duration;
                        current_target_rps = max_rps - (max_rps - min_rps) * (ramp_down_elapsed / one_third_duration);
                        // Ensure it doesn't go below min_rps
                        if current_target_rps < *min_rps {
                            current_target_rps = *min_rps;
                        }
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
                // Ensure the elapsed time wraps around the cycle duration
                let time_in_cycle = elapsed_total_secs % cycle_duration_secs;

                // Calculate absolute time boundaries for each segment within the cycle
                let morning_ramp_end = cycle_duration_secs * morning_ramp_ratio;
                let peak_sustain_end = morning_ramp_end + (cycle_duration_secs * peak_sustain_ratio);
                let mid_decline_end = peak_sustain_end + (cycle_duration_secs * mid_decline_ratio);
                let mid_sustain_end = mid_decline_end + (cycle_duration_secs * mid_sustain_ratio);
                let evening_decline_end = mid_sustain_end + (cycle_duration_secs * evening_decline_ratio);
                // Night sustain implicitly lasts until cycle_duration_secs

                let mut current_target_rps = *min_rps; // Default to min_rps (night)

                if cycle_duration_secs <= 0.0 {
                    return *max_rps; // Handle zero cycle duration, default to max
                }

                if time_in_cycle < morning_ramp_end {
                    // Phase 1: Morning Ramp-up (min_rps to max_rps)
                    let ramp_elapsed = time_in_cycle;
                    let ramp_duration = morning_ramp_end;
                    if ramp_duration > 0.0 {
                        current_target_rps = min_rps + (max_rps - min_rps) * (ramp_elapsed / ramp_duration);
                    } else {
                        current_target_rps = *max_rps; // Instant ramp to max if duration is zero
                    }
                } else if time_in_cycle < peak_sustain_end {
                    // Phase 2: Peak Sustain (max_rps)
                    current_target_rps = *max_rps;
                } else if time_in_cycle < mid_decline_end {
                    // Phase 3: Mid-Day Decline (max_rps to mid_rps)
                    let decline_elapsed = time_in_cycle - peak_sustain_end;
                    let decline_duration = mid_decline_end - peak_sustain_end;
                    if decline_duration > 0.0 {
                        current_target_rps = max_rps - (max_rps - mid_rps) * (decline_elapsed / decline_duration);
                    } else {
                        current_target_rps = *mid_rps; // Instant decline to mid if duration is zero
                    }
                } else if time_in_cycle < mid_sustain_end {
                    // Phase 4: Mid-Day Sustain (mid_rps)
                    current_target_rps = *mid_rps;
                } else if time_in_cycle < evening_decline_end {
                    // Phase 5: Evening Decline (mid_rps to min_rps)
                    let decline_elapsed = time_in_cycle - mid_sustain_end;
                    let decline_duration = evening_decline_end - mid_sustain_end;
                    if decline_duration > 0.0 {
                        current_target_rps = mid_rps - (mid_rps - min_rps) * (decline_elapsed / decline_duration);
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

    let client = reqwest::Client::new();

    // --- READ FROM ENVIRONMENT VARIABLES ---
    let url = env::var("TARGET_URL")
        .expect("TARGET_URL environment variable must be set");

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


    println!("Starting load test:");
    println!("  Target URL: {}", url);
    println!("  Concurrent Tasks: {}", num_concurrent_tasks);
    println!("  Overall Test Duration: {:?}", overall_test_duration);
    println!("  Load Model: {:?}", load_model);


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
    let overall_test_duration_secs = overall_test_duration.as_secs_f64(); // Get this once

    let mut handles = Vec::new();
    for i in 0..num_concurrent_tasks {
        let client_clone = client.clone();
        let url_clone = url.to_string();
        let overall_test_duration_clone = overall_test_duration.clone();
        let start_time_clone = start_time.clone();
        let load_model_clone = load_model.clone(); // Clone load model for each task
        let num_concurrent_tasks_clone = num_concurrent_tasks.clone(); // Clone for use in worker task

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