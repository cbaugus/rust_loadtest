use tokio::time::{self, Duration, Instant};
use tracing::{debug, error, info};

use crate::load_models::LoadModel;
use crate::metrics::{
    CONCURRENT_REQUESTS, REQUEST_DURATION_SECONDS, REQUEST_STATUS_CODES, REQUEST_TOTAL,
};

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
            Ok(response) => {
                let status = response.status().as_u16();
                let status_str = status.to_string();
                REQUEST_STATUS_CODES.with_label_values(&[&status_str]).inc();

                debug!(
                    task_id = config.task_id,
                    url = %config.url,
                    status_code = status,
                    latency_ms = request_start_time.elapsed().as_millis() as u64,
                    "Request completed"
                );
            }
            Err(e) => {
                REQUEST_STATUS_CODES.with_label_values(&["error"]).inc();
                error!(
                    task_id = config.task_id,
                    url = %config.url,
                    error = %e,
                    "Request failed"
                );
            }
        }

        REQUEST_DURATION_SECONDS.observe(request_start_time.elapsed().as_secs_f64());
        CONCURRENT_REQUESTS.dec();

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
        _ => {
            error!(
                request_type = %config.request_type,
                "Unsupported request type, falling back to GET"
            );
            client.get(&config.url)
        }
    }
}
