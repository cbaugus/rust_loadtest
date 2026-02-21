use std::sync::atomic::{AtomicU64, Ordering};

use tokio::time::{self, Duration, Instant};
use tracing::{debug, error, info};

/// Atomic counter for deterministic percentile sampling (Issue #70).
static SAMPLE_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Returns true if this request should be recorded in percentile histograms.
///
/// Uses a deterministic counter so every Nth request is sampled (not random),
/// giving even distribution across all workers without coordination overhead.
/// `rate` is 1-100: at 100 every request is recorded, at 10 every 10th is.
fn should_sample(rate: u8) -> bool {
    if rate >= 100 {
        return true;
    }
    let counter = SAMPLE_COUNTER.fetch_add(1, Ordering::Relaxed);
    counter % 100 < rate as u64
}

use crate::connection_pool::GLOBAL_POOL_STATS;
use crate::errors::ErrorCategory;
use crate::executor::ScenarioExecutor;
use crate::load_models::LoadModel;
use crate::memory_guard::is_percentile_tracking_active;
use crate::metrics::{
    CONCURRENT_REQUESTS, REQUEST_DURATION_SECONDS, REQUEST_ERRORS_BY_CATEGORY,
    REQUEST_STATUS_CODES, REQUEST_TOTAL, SCENARIO_REQUESTS_TOTAL,
};
use crate::percentiles::{
    GLOBAL_REQUEST_PERCENTILES, GLOBAL_SCENARIO_PERCENTILES, GLOBAL_STEP_PERCENTILES,
};
use crate::scenario::{Scenario, ScenarioContext};
use crate::throughput::GLOBAL_THROUGHPUT_TRACKER;

/// Configuration for a worker task.
pub struct WorkerConfig {
    pub task_id: usize,
    pub url: String,
    pub request_type: String,
    pub send_json: bool,
    pub json_payload: Option<String>,
    pub test_duration: Duration,
    pub load_model: LoadModel,
    pub num_concurrent_tasks: usize,
    pub percentile_tracking_enabled: bool,
    pub percentile_sampling_rate: u8,
}

/// Runs a single worker task that sends HTTP requests according to the load model.
pub async fn run_worker(client: reqwest::Client, config: WorkerConfig, start_time: Instant) {
    debug!(
        task_id = config.task_id,
        url = %config.url,
        load_model = ?config.load_model,
        "Worker starting"
    );

    loop {
        let elapsed_total_secs = Instant::now().duration_since(start_time).as_secs_f64();

        // Check if the total test duration has passed
        if elapsed_total_secs >= config.test_duration.as_secs_f64() {
            info!(
                task_id = config.task_id,
                elapsed_secs = elapsed_total_secs,
                "Worker stopping after duration limit"
            );
            break;
        }

        // Calculate current target RPS
        let current_target_rps = config
            .load_model
            .calculate_current_rps(elapsed_total_secs, config.test_duration.as_secs_f64());

        // Calculate delay per task to achieve the current_target_rps
        let delay_ms = if current_target_rps > 0.0 {
            (config.num_concurrent_tasks as f64 * 1000.0 / current_target_rps).round() as u64
        } else {
            u64::MAX
        };

        // Track metrics
        CONCURRENT_REQUESTS.inc();
        REQUEST_TOTAL.inc();

        let request_start_time = time::Instant::now();

        // Build and send request
        let req = build_request(&client, &config);

        match req.send().await {
            Ok(mut response) => {
                let status = response.status().as_u16();
                let status_str = status.to_string();
                REQUEST_STATUS_CODES.with_label_values(&[&status_str]).inc();

                // Categorize HTTP errors (Issue #34)
                if let Some(category) = ErrorCategory::from_status_code(status) {
                    REQUEST_ERRORS_BY_CATEGORY
                        .with_label_values(&[category.label()])
                        .inc();
                }

                // Issue #74: CRITICAL - Must consume response body in chunks to prevent buffering
                // At 50K RPS, unconsumed bodies accumulate in memory causing rapid OOM
                // Stream and discard body without allocating full buffer
                while let Ok(Some(_chunk)) = response.chunk().await {
                    // Chunk read and immediately dropped - minimal memory footprint
                }

                debug!(
                    task_id = config.task_id,
                    url = %config.url,
                    status_code = status,
                    "Request completed"
                );
            }
            Err(e) => {
                REQUEST_STATUS_CODES.with_label_values(&["error"]).inc();

                // Categorize request error (Issue #34)
                let error_category = ErrorCategory::from_reqwest_error(&e);
                REQUEST_ERRORS_BY_CATEGORY
                    .with_label_values(&[error_category.label()])
                    .inc();

                error!(
                    task_id = config.task_id,
                    url = %config.url,
                    error = %e,
                    error_category = %error_category.label(),
                    "Request failed"
                );
            }
        }

        let actual_latency_ms = request_start_time.elapsed().as_millis() as u64;
        REQUEST_DURATION_SECONDS.observe(request_start_time.elapsed().as_secs_f64());
        CONCURRENT_REQUESTS.dec();

        // Record latency in percentile tracker (Issue #33, #66, #70, #72)
        // Check both config flag AND runtime flag (can be disabled by memory guard)
        if config.percentile_tracking_enabled
            && is_percentile_tracking_active()
            && should_sample(config.percentile_sampling_rate)
        {
            GLOBAL_REQUEST_PERCENTILES.record_ms(actual_latency_ms);
        }

        // Record connection pool statistics (Issue #36)
        GLOBAL_POOL_STATS.record_request(actual_latency_ms);

        // Apply the calculated delay
        if delay_ms > 0 && delay_ms != u64::MAX {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        } else if delay_ms == u64::MAX {
            // Sleep for a very long time if RPS is 0
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
        // If delay_ms is 0, no sleep, burst as fast as possible.
    }
}

fn build_request(client: &reqwest::Client, config: &WorkerConfig) -> reqwest::RequestBuilder {
    match config.request_type.as_str() {
        "GET" => client.get(&config.url),
        "POST" => {
            let req = client.post(&config.url);
            if config.send_json {
                req.header("Content-Type", "application/json")
                    .body(config.json_payload.clone().unwrap_or_default())
            } else {
                req
            }
        }
        "PUT" => {
            let req = client.put(&config.url);
            if config.send_json {
                req.header("Content-Type", "application/json")
                    .body(config.json_payload.clone().unwrap_or_default())
            } else {
                req
            }
        }
        "PATCH" => {
            let req = client.patch(&config.url);
            if config.send_json {
                req.header("Content-Type", "application/json")
                    .body(config.json_payload.clone().unwrap_or_default())
            } else {
                req
            }
        }
        "DELETE" => client.delete(&config.url),
        "HEAD" => client.head(&config.url),
        "OPTIONS" => client.request(reqwest::Method::OPTIONS, &config.url),
        _ => {
            error!(
                request_type = %config.request_type,
                "Unsupported request type, falling back to GET"
            );
            client.get(&config.url)
        }
    }
}

/// Configuration for a scenario-based worker task.
pub struct ScenarioWorkerConfig {
    pub task_id: usize,
    pub base_url: String,
    pub scenario: Scenario,
    pub test_duration: Duration,
    pub load_model: LoadModel,
    pub num_concurrent_tasks: usize,
    pub percentile_tracking_enabled: bool,
    pub percentile_sampling_rate: u8,
}

/// Runs a scenario-based worker task that executes multi-step scenarios according to the load model.
///
/// This worker executes complete scenarios (multiple steps) instead of individual requests.
/// Each scenario execution counts as one "virtual user" completing their journey.
///
/// # Cookie and Session Management
///
/// For proper session isolation, each scenario execution gets its own cookie-enabled
/// HTTP client. This ensures cookies from one virtual user don't leak to another.
pub async fn run_scenario_worker(
    _client: reqwest::Client, // Ignored - we create per-execution clients
    config: ScenarioWorkerConfig,
    start_time: Instant,
) {
    debug!(
        task_id = config.task_id,
        scenario = %config.scenario.name,
        steps = config.scenario.steps.len(),
        load_model = ?config.load_model,
        "Scenario worker starting"
    );

    loop {
        let elapsed_total_secs = Instant::now().duration_since(start_time).as_secs_f64();

        // Check if the total test duration has passed
        if elapsed_total_secs >= config.test_duration.as_secs_f64() {
            info!(
                task_id = config.task_id,
                scenario = %config.scenario.name,
                elapsed_secs = elapsed_total_secs,
                "Scenario worker stopping after duration limit"
            );
            break;
        }

        // Calculate current target RPS (scenarios per second in this case)
        let current_target_sps = config
            .load_model
            .calculate_current_rps(elapsed_total_secs, config.test_duration.as_secs_f64());

        // Calculate delay per task to achieve the current_target_sps
        let delay_ms = if current_target_sps > 0.0 {
            (config.num_concurrent_tasks as f64 * 1000.0 / current_target_sps).round() as u64
        } else {
            u64::MAX
        };

        // Create new cookie-enabled client for this virtual user
        // This ensures cookie isolation between scenario executions
        let client = reqwest::Client::builder()
            .cookie_store(true) // Enable automatic cookie management
            .timeout(std::time::Duration::from_secs(30))
            .build()
            .unwrap_or_else(|_| reqwest::Client::new());

        // Create executor with isolated client
        let executor = ScenarioExecutor::new(config.base_url.clone(), client);

        // Create new context for this scenario execution
        let mut context = ScenarioContext::new();

        // Execute the scenario
        let result = executor.execute(&config.scenario, &mut context).await;

        debug!(
            task_id = config.task_id,
            scenario = %config.scenario.name,
            success = result.success,
            duration_ms = result.total_time_ms,
            steps_completed = result.steps_completed,
            "Scenario execution completed"
        );

        // Record scenario latency in percentile tracker (Issue #33, #66, #70, #72)
        // Check both config flag AND runtime flag (can be disabled by memory guard)
        if config.percentile_tracking_enabled
            && is_percentile_tracking_active()
            && should_sample(config.percentile_sampling_rate)
        {
            GLOBAL_SCENARIO_PERCENTILES.record(&config.scenario.name, result.total_time_ms);

            // Record individual step latencies (Issue #33, #66, #70, #72)
            for step in &result.steps {
                let label = format!("{}:{}", config.scenario.name, step.step_name);
                GLOBAL_STEP_PERCENTILES.record(&label, step.response_time_ms);
            }
        }

        // Record throughput (Issue #35)
        SCENARIO_REQUESTS_TOTAL
            .with_label_values(&[&config.scenario.name])
            .inc();
        GLOBAL_THROUGHPUT_TRACKER.record(
            &config.scenario.name,
            std::time::Duration::from_millis(result.total_time_ms),
        );

        // Apply the calculated delay between scenario executions
        if delay_ms > 0 && delay_ms != u64::MAX {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
        } else if delay_ms == u64::MAX {
            // Sleep for a very long time if SPS is 0
            tokio::time::sleep(Duration::from_secs(3600)).await;
        }
        // If delay_ms is 0, no sleep, execute scenarios as fast as possible
    }
}
