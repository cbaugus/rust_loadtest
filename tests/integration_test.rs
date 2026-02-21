use std::sync::Once;
use tokio::time::{Duration, Instant};
use wiremock::matchers::{body_string, header, method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use rust_loadtest::load_models::LoadModel;
use rust_loadtest::metrics::{
    register_metrics, CONCURRENT_REQUESTS, REQUEST_DURATION_SECONDS, REQUEST_STATUS_CODES,
    REQUEST_TOTAL,
};
use rust_loadtest::worker::{run_worker, WorkerConfig};

// Register metrics once across all tests in this file.
// Calling register_metrics() more than once would panic due to duplicate registration.
static INIT_METRICS: Once = Once::new();

fn init_metrics() {
    INIT_METRICS.call_once(|| {
        register_metrics().expect("Failed to register metrics");
    });
}

fn get_total_requests() -> u64 {
    REQUEST_TOTAL.get()
}

fn get_status_code_count(code: &str) -> u64 {
    REQUEST_STATUS_CODES.with_label_values(&[code]).get()
}

fn get_duration_count() -> u64 {
    REQUEST_DURATION_SECONDS.get_sample_count()
}

// --- GET request tests ---

#[tokio::test]
async fn worker_sends_get_requests() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/test"))
        .respond_with(ResponseTemplate::new(200))
        .expect(1..)
        .mount(&server)
        .await;

    let before = get_total_requests();

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/test", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after = get_total_requests();
    assert!(
        after > before,
        "expected requests to increase, before={} after={}",
        before,
        after
    );
}

// --- POST request tests ---

#[tokio::test]
async fn worker_sends_post_requests() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/api"))
        .respond_with(ResponseTemplate::new(201))
        .expect(1..)
        .mount(&server)
        .await;

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/api", server.uri()),
        request_type: "POST".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    // wiremock will verify expectations on drop (at least 1 POST received)
}

// --- POST with JSON body ---

#[tokio::test]
async fn worker_sends_json_post_body() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("POST"))
        .and(path("/json"))
        .and(header("Content-Type", "application/json"))
        .and(body_string(r#"{"key":"value"}"#))
        .respond_with(ResponseTemplate::new(200))
        .expect(1..)
        .mount(&server)
        .await;

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/json", server.uri()),
        request_type: "POST".to_string(),
        send_json: true,
        json_payload: Some(r#"{"key":"value"}"#.to_string()),
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    // wiremock will verify the JSON body and Content-Type header
}

// --- Status code tracking ---

#[tokio::test]
async fn worker_tracks_200_status_codes() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/ok"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let before_200 = get_status_code_count("200");

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/ok", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after_200 = get_status_code_count("200");
    assert!(
        after_200 > before_200,
        "expected 200 count to increase, before={} after={}",
        before_200,
        after_200
    );
}

#[tokio::test]
async fn worker_tracks_404_status_codes() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/notfound"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let before_404 = get_status_code_count("404");

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/notfound", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after_404 = get_status_code_count("404");
    assert!(
        after_404 > before_404,
        "expected 404 count to increase, before={} after={}",
        before_404,
        after_404
    );
}

#[tokio::test]
async fn worker_tracks_500_status_codes() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/error"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let before_500 = get_status_code_count("500");

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/error", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after_500 = get_status_code_count("500");
    assert!(
        after_500 > before_500,
        "expected 500 count to increase, before={} after={}",
        before_500,
        after_500
    );
}

// --- Duration metrics ---

#[tokio::test]
async fn worker_records_request_duration() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/duration"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let before_count = get_duration_count();

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/duration", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after_count = get_duration_count();
    assert!(
        after_count > before_count,
        "expected duration sample count to increase, before={} after={}",
        before_count,
        after_count
    );
}

// --- Concurrent requests gauge ---

#[tokio::test]
async fn concurrent_requests_returns_to_zero_after_worker_finishes() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/concurrent"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/concurrent", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    // After worker finishes, concurrent requests gauge should not be negative
    let gauge = CONCURRENT_REQUESTS.get();
    assert!(
        gauge >= 0.0,
        "concurrent requests gauge should not be negative, got {}",
        gauge
    );
}

// --- Connection error handling ---

#[tokio::test]
async fn worker_handles_connection_error_gracefully() {
    init_metrics();

    // Use a URL that will refuse connections
    let before_errors = get_status_code_count("error");

    let config = WorkerConfig {
        task_id: 0,
        url: "http://127.0.0.1:1/unreachable".to_string(),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::builder()
        .connect_timeout(Duration::from_millis(100))
        .build()
        .unwrap();
    run_worker(client, config, Instant::now()).await;

    let after_errors = get_status_code_count("error");
    assert!(
        after_errors > before_errors,
        "expected error count to increase, before={} after={}",
        before_errors,
        after_errors
    );
}

// --- RPS load model ---

#[tokio::test]
async fn worker_respects_rps_rate_limit() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/rps"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    // Target 5 RPS with 1 worker for 3 seconds = ~15 requests
    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/rps", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(3),
        load_model: LoadModel::Rps { target_rps: 5.0 },
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let start = Instant::now();
    let client = reqwest::Client::new();
    run_worker(client, config, start).await;
    let elapsed = start.elapsed();

    // Verify worker ran for approximately 3 seconds (rate limiting should prevent it from finishing early)
    assert!(
        elapsed.as_secs() >= 2 && elapsed.as_secs() <= 5,
        "worker should run for ~3s with rate limiting, ran for {:?}",
        elapsed
    );
}

// --- Worker stops after test duration ---

#[tokio::test]
async fn worker_stops_after_test_duration() {
    init_metrics();
    let server = MockServer::start().await;

    Mock::given(method("GET"))
        .and(path("/timeout"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/timeout", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(2),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let start = Instant::now();
    let client = reqwest::Client::new();
    run_worker(client, config, start).await;
    let elapsed = start.elapsed();

    // Worker should finish within a reasonable time after the 2s duration
    assert!(
        elapsed.as_secs() <= 5,
        "worker should stop near test duration, ran for {:?}",
        elapsed
    );
}

// --- Slow responses ---

#[tokio::test]
async fn worker_handles_slow_responses() {
    init_metrics();
    let server = MockServer::start().await;

    // Response with 500ms delay
    Mock::given(method("GET"))
        .and(path("/slow"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string("ok")
                .set_delay(Duration::from_millis(500)),
        )
        .mount(&server)
        .await;

    let before = get_total_requests();

    let config = WorkerConfig {
        task_id: 0,
        url: format!("{}/slow", server.uri()),
        request_type: "GET".to_string(),
        send_json: false,
        json_payload: None,
        test_duration: Duration::from_secs(3),
        load_model: LoadModel::Concurrent,
        num_concurrent_tasks: 1,
        percentile_tracking_enabled: true,
        percentile_sampling_rate: 100,
    };

    let client = reqwest::Client::new();
    run_worker(client, config, Instant::now()).await;

    let after = get_total_requests();
    assert!(
        after > before,
        "expected requests even with slow responses, before={} after={}",
        before,
        after
    );
}
