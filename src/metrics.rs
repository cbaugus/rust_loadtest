use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use prometheus::{
    Encoder, Gauge, Histogram, HistogramVec, IntCounter, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::env;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

lazy_static::lazy_static! {
    pub static ref METRIC_NAMESPACE: String =
        env::var("METRIC_NAMESPACE").unwrap_or_else(|_| "rust_loadtest".to_string());

    // === Single Request Metrics ===

    pub static ref REQUEST_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new("requests_total", "Total number of HTTP requests made")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref REQUEST_STATUS_CODES: IntCounterVec =
        IntCounterVec::new(
            Opts::new("requests_status_codes_total", "Number of HTTP requests by status code")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["status_code"]
        ).unwrap();

    pub static ref CONCURRENT_REQUESTS: Gauge =
        Gauge::with_opts(
            Opts::new("concurrent_requests", "Number of HTTP requests currently in flight")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref REQUEST_DURATION_SECONDS: Histogram =
        Histogram::with_opts(
            prometheus::HistogramOpts::new(
                "request_duration_seconds",
                "HTTP request latencies in seconds."
            ).namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    // === Scenario Metrics ===

    pub static ref SCENARIO_EXECUTIONS_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new("scenario_executions_total", "Total number of scenario executions")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario", "status"]  // status: success, failed
        ).unwrap();

    pub static ref SCENARIO_DURATION_SECONDS: HistogramVec =
        HistogramVec::new(
            prometheus::HistogramOpts::new(
                "scenario_duration_seconds",
                "Scenario execution duration in seconds"
            ).namespace(METRIC_NAMESPACE.as_str()),
            &["scenario"]
        ).unwrap();

    pub static ref SCENARIO_STEPS_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new("scenario_steps_total", "Total number of scenario steps executed")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario", "step", "status"]  // status: success, failed
        ).unwrap();

    pub static ref SCENARIO_STEP_DURATION_SECONDS: HistogramVec =
        HistogramVec::new(
            prometheus::HistogramOpts::new(
                "scenario_step_duration_seconds",
                "Scenario step duration in seconds"
            ).namespace(METRIC_NAMESPACE.as_str()),
            &["scenario", "step"]
        ).unwrap();

    pub static ref SCENARIO_ASSERTIONS_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new("scenario_assertions_total", "Total number of scenario assertions")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario", "step", "result"]  // result: passed, failed
        ).unwrap();

    pub static ref CONCURRENT_SCENARIOS: Gauge =
        Gauge::with_opts(
            Opts::new("concurrent_scenarios", "Number of scenario executions currently running")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();
}

/// Registers all metrics with the default Prometheus registry.
pub fn register_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Single request metrics
    prometheus::default_registry().register(Box::new(REQUEST_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_STATUS_CODES.clone()))?;
    prometheus::default_registry().register(Box::new(CONCURRENT_REQUESTS.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_DURATION_SECONDS.clone()))?;

    // Scenario metrics
    prometheus::default_registry().register(Box::new(SCENARIO_EXECUTIONS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_DURATION_SECONDS.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_STEPS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_STEP_DURATION_SECONDS.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_ASSERTIONS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(CONCURRENT_SCENARIOS.clone()))?;

    Ok(())
}

/// HTTP handler for the Prometheus metrics endpoint.
pub async fn metrics_handler(
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

/// Starts the Prometheus metrics HTTP server.
pub async fn start_metrics_server(port: u16, registry: Arc<Mutex<Registry>>) {
    let addr = ([0, 0, 0, 0], port).into();

    let make_svc = make_service_fn(move |_conn| {
        let registry_clone = registry.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let registry_clone_inner = registry_clone.clone();
                async move { metrics_handler(req, registry_clone_inner).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    info!(
        port = port,
        addr = %addr,
        "Metrics server listening"
    );

    if let Err(e) = server.await {
        error!(error = %e, "Metrics server error");
    }
}

/// Gathers and encodes metrics as a string for final output.
pub fn gather_metrics_string(registry: &Arc<Mutex<Registry>>) -> String {
    let encoder = TextEncoder::new();
    let metric_families = registry.lock().unwrap().gather();
    let mut buffer = Vec::new();
    encoder.encode(&metric_families, &mut buffer).unwrap();
    String::from_utf8(buffer).unwrap_or_else(|e| {
        eprintln!("Error encoding metrics to UTF-8: {}", e);
        String::from("# ERROR ENCODING METRICS TO UTF-8")
    })
}
