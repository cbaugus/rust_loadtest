use std::sync::{Arc, Mutex};
use tokio::time::{self, Duration};

use rust_loadtest::client::build_client;
use rust_loadtest::config::Config;
use rust_loadtest::metrics::{gather_metrics_string, register_metrics, start_metrics_server};
use rust_loadtest::worker::{run_worker, WorkerConfig};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Register Prometheus metrics
    register_metrics()?;

    // Load configuration from environment variables
    let config = Config::from_env()?;

    // Build HTTP client with TLS and header configuration
    let client_config = config.to_client_config();
    let client_result = build_client(&client_config)?;
    let client = client_result.client;

    // Print configuration summary
    config.print_summary(&client_result.parsed_headers);

    // Start the Prometheus metrics HTTP server
    let metrics_port = 9090;
    let registry_arc = Arc::new(Mutex::new(prometheus::default_registry().clone()));

    {
        let registry = registry_arc.clone();
        tokio::spawn(async move {
            start_metrics_server(metrics_port, registry).await;
        });
    }

    // Main loop to run for a duration
    let start_time = time::Instant::now();

    let mut handles = Vec::new();
    for i in 0..config.num_concurrent_tasks {
        let worker_config = WorkerConfig {
            task_id: i,
            url: config.target_url.clone(),
            request_type: config.request_type.clone(),
            send_json: config.send_json,
            json_payload: config.json_payload.clone(),
            test_duration: config.test_duration,
            load_model: config.load_model.clone(),
            num_concurrent_tasks: config.num_concurrent_tasks,
        };

        let client_clone = client.clone();
        let start_time_clone = start_time;

        let handle = tokio::spawn(async move {
            run_worker(client_clone, worker_config, start_time_clone).await;
        });
        handles.push(handle);
    }

    // Wait for the total test duration to pass
    tokio::time::sleep(config.test_duration).await;
    println!("Main test duration completed. Signalling tasks to stop.");

    // Brief pause to allow in-flight metrics to be updated
    tokio::time::sleep(Duration::from_secs(2)).await;
    println!("Collecting and printing final metrics...");

    // Gather and print final metrics
    let final_metrics_output = gather_metrics_string(&registry_arc);
    println!("\n--- FINAL METRICS ---\n{}", final_metrics_output);
    println!("--- END OF FINAL METRICS ---\n");

    println!("Pausing for 2 minutes to allow final Prometheus scrape...");
    tokio::time::sleep(Duration::from_secs(120)).await;
    println!("2-minute pause complete. Exiting.");

    Ok(())
}
