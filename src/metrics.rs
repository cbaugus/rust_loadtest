use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use prometheus::{
    Encoder, Gauge, Histogram, IntCounter, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::env;
use std::sync::{Arc, Mutex};

lazy_static::lazy_static! {
    pub static ref METRIC_NAMESPACE: String =
        env::var("METRIC_NAMESPACE").unwrap_or_else(|_| "rust_loadtest".to_string());

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
}

/// Registers all metrics with the default Prometheus registry.
pub fn register_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    prometheus::default_registry().register(Box::new(REQUEST_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_STATUS_CODES.clone()))?;
    prometheus::default_registry().register(Box::new(CONCURRENT_REQUESTS.clone()))?;
    prometheus::default_registry().register(Box::new(REQUEST_DURATION_SECONDS.clone()))?;
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
    println!(
        "Prometheus metrics server listening on http://0.0.0.0:{}",
        port
    );

    if let Err(e) = server.await {
        eprintln!("Metrics server error: {}", e);
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
