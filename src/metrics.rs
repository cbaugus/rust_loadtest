use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use prometheus::{
    Encoder, Gauge, HistogramVec, IntCounter, IntCounterVec, Opts, Registry, TextEncoder,
};
use std::env;
use std::sync::{Arc, Mutex};
use tracing::{error, info};

lazy_static::lazy_static! {
    pub static ref METRIC_NAMESPACE: String =
        env::var("METRIC_NAMESPACE").unwrap_or_else(|_| "rust_loadtest".to_string());

    // === Single Request Metrics ===

    pub static ref REQUEST_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new("requests_total", "Total number of HTTP requests made")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["region"]
        ).unwrap();

    pub static ref REQUEST_STATUS_CODES: IntCounterVec =
        IntCounterVec::new(
            Opts::new("requests_status_codes_total", "Number of HTTP requests by status code")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["status_code", "region"]
        ).unwrap();

    pub static ref CONCURRENT_REQUESTS: prometheus::GaugeVec =
        prometheus::GaugeVec::new(
            Opts::new("concurrent_requests", "Number of HTTP requests currently in flight")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["region"]
        ).unwrap();

    pub static ref REQUEST_DURATION_SECONDS: HistogramVec =
        HistogramVec::new(
            prometheus::HistogramOpts::new(
                "request_duration_seconds",
                "HTTP request latencies in seconds."
            ).namespace(METRIC_NAMESPACE.as_str()),
            &["region"]
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

    pub static ref SCENARIO_STEP_STATUS_CODES: IntCounterVec =
        IntCounterVec::new(
            Opts::new("scenario_step_status_codes_total", "HTTP status codes per scenario step")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario", "step", "status_code"]
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

    // === Per-Scenario Throughput Metrics (Issue #35) ===

    pub static ref SCENARIO_REQUESTS_TOTAL: IntCounterVec =
        IntCounterVec::new(
            Opts::new("scenario_requests_total", "Total number of requests per scenario")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario"]
        ).unwrap();

    pub static ref SCENARIO_THROUGHPUT_RPS: prometheus::GaugeVec =
        prometheus::GaugeVec::new(
            Opts::new("scenario_throughput_rps", "Current throughput (requests per second) per scenario")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["scenario"]
        ).unwrap();

    // === Error Categorization Metrics (Issue #34) ===

    pub static ref REQUEST_ERRORS_BY_CATEGORY: IntCounterVec =
        IntCounterVec::new(
            Opts::new("request_errors_by_category", "Number of errors by category")
                .namespace(METRIC_NAMESPACE.as_str()),
            &["category", "region"]
        ).unwrap();

    // === Connection Pool Metrics (Issue #36) ===

    pub static ref CONNECTION_POOL_MAX_IDLE: Gauge =
        Gauge::with_opts(
            Opts::new("connection_pool_max_idle_per_host", "Maximum idle connections per host (configuration)")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref CONNECTION_POOL_IDLE_TIMEOUT_SECONDS: Gauge =
        Gauge::with_opts(
            Opts::new("connection_pool_idle_timeout_seconds", "Idle connection timeout in seconds (configuration)")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref CONNECTION_POOL_REQUESTS_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new("connection_pool_requests_total", "Total requests tracked for pool analysis")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref CONNECTION_POOL_LIKELY_REUSED: IntCounter =
        IntCounter::with_opts(
            Opts::new("connection_pool_likely_reused_total", "Requests that likely reused existing connections")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref CONNECTION_POOL_LIKELY_NEW: IntCounter =
        IntCounter::with_opts(
            Opts::new("connection_pool_likely_new_total", "Requests that likely established new connections")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref CONNECTION_POOL_REUSE_RATE: Gauge =
        Gauge::with_opts(
            Opts::new("connection_pool_reuse_rate_percent", "Percentage of requests reusing connections")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    // === Memory Usage Metrics (Issue #69) ===

    pub static ref PROCESS_MEMORY_RSS_BYTES: Gauge =
        Gauge::with_opts(
            Opts::new("process_memory_rss_bytes", "Resident set size (RSS) memory in bytes")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref PROCESS_MEMORY_VIRTUAL_BYTES: Gauge =
        Gauge::with_opts(
            Opts::new("process_memory_virtual_bytes", "Virtual memory size in bytes")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref HISTOGRAM_COUNT: Gauge =
        Gauge::with_opts(
            Opts::new("histogram_count", "Number of active HDR histograms")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    pub static ref HISTOGRAM_MEMORY_ESTIMATE_BYTES: Gauge =
        Gauge::with_opts(
            Opts::new("histogram_memory_estimate_bytes", "Estimated memory used by histograms")
                .namespace(METRIC_NAMESPACE.as_str())
        ).unwrap();

    // === Memory Guard & Percentile Tracking Metrics (Issue #72) ===

    pub static ref PERCENTILE_TRACKING_ACTIVE_GAUGE: Gauge =
        Gauge::with_opts(
            Opts::new(
                "percentile_tracking_active",
                "1 if percentile tracking is active, 0 if disabled by memory guard",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    pub static ref MEMORY_WARNING_THRESHOLD_EXCEEDED_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new(
                "memory_warning_threshold_exceeded_total",
                "Number of times the memory warning threshold has been exceeded",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    pub static ref MEMORY_CRITICAL_THRESHOLD_EXCEEDED_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new(
                "memory_critical_threshold_exceeded_total",
                "Number of times the memory critical threshold has been exceeded",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    pub static ref HISTOGRAM_LABELS_EVICTED_TOTAL: IntCounter =
        IntCounter::with_opts(
            Opts::new(
                "histogram_labels_evicted_total",
                "Total number of histogram labels evicted due to LRU capacity limit",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    // === Test Configuration Metrics ===

    pub static ref PERCENTILE_SAMPLING_RATE_PERCENT: Gauge =
        Gauge::with_opts(
            Opts::new(
                "percentile_sampling_rate_percent",
                "Configured percentile sampling rate (1-100 percent of requests recorded)",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    pub static ref WORKERS_CONFIGURED_TOTAL: Gauge =
        Gauge::with_opts(
            Opts::new(
                "workers_configured_total",
                "Number of concurrent worker tasks configured",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
        )
        .unwrap();

    // === Cluster Node Info (Issue #45) ===

    /// Info gauge set to 1 when the node is running. Labels identify the node
    /// within its cluster. In standalone mode: state="standalone".
    pub static ref CLUSTER_NODE_INFO: prometheus::GaugeVec =
        prometheus::GaugeVec::new(
            Opts::new(
                "cluster_node_info",
                "Cluster node identity and state (1 = running). Labels: node_id, region, state.",
            )
            .namespace(METRIC_NAMESPACE.as_str()),
            &["node_id", "region", "state"],
        )
        .unwrap();
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
    prometheus::default_registry().register(Box::new(SCENARIO_STEP_STATUS_CODES.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_ASSERTIONS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(CONCURRENT_SCENARIOS.clone()))?;

    // Per-scenario throughput metrics
    prometheus::default_registry().register(Box::new(SCENARIO_REQUESTS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(SCENARIO_THROUGHPUT_RPS.clone()))?;

    // Error categorization metrics
    prometheus::default_registry().register(Box::new(REQUEST_ERRORS_BY_CATEGORY.clone()))?;

    // Connection pool metrics
    prometheus::default_registry().register(Box::new(CONNECTION_POOL_MAX_IDLE.clone()))?;
    prometheus::default_registry()
        .register(Box::new(CONNECTION_POOL_IDLE_TIMEOUT_SECONDS.clone()))?;
    prometheus::default_registry().register(Box::new(CONNECTION_POOL_REQUESTS_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(CONNECTION_POOL_LIKELY_REUSED.clone()))?;
    prometheus::default_registry().register(Box::new(CONNECTION_POOL_LIKELY_NEW.clone()))?;
    prometheus::default_registry().register(Box::new(CONNECTION_POOL_REUSE_RATE.clone()))?;

    // Memory usage metrics
    prometheus::default_registry().register(Box::new(PROCESS_MEMORY_RSS_BYTES.clone()))?;
    prometheus::default_registry().register(Box::new(PROCESS_MEMORY_VIRTUAL_BYTES.clone()))?;
    prometheus::default_registry().register(Box::new(HISTOGRAM_COUNT.clone()))?;
    prometheus::default_registry().register(Box::new(HISTOGRAM_MEMORY_ESTIMATE_BYTES.clone()))?;

    // Memory guard & percentile tracking metrics
    prometheus::default_registry().register(Box::new(PERCENTILE_TRACKING_ACTIVE_GAUGE.clone()))?;
    prometheus::default_registry()
        .register(Box::new(MEMORY_WARNING_THRESHOLD_EXCEEDED_TOTAL.clone()))?;
    prometheus::default_registry()
        .register(Box::new(MEMORY_CRITICAL_THRESHOLD_EXCEEDED_TOTAL.clone()))?;
    prometheus::default_registry().register(Box::new(HISTOGRAM_LABELS_EVICTED_TOTAL.clone()))?;

    // Test configuration metrics
    prometheus::default_registry().register(Box::new(PERCENTILE_SAMPLING_RATE_PERCENT.clone()))?;
    prometheus::default_registry().register(Box::new(WORKERS_CONFIGURED_TOTAL.clone()))?;

    // Cluster node info (Issue #45)
    prometheus::default_registry().register(Box::new(CLUSTER_NODE_INFO.clone()))?;

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

/// Updates memory usage metrics (Issue #69).
///
/// Reads process memory stats from /proc on Linux and estimates
/// histogram memory usage based on active label count.
pub fn update_memory_metrics() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Platform-specific memory stats
    #[cfg(target_os = "linux")]
    {
        use procfs::process::Process;

        match Process::myself() {
            Ok(me) => {
                if let Ok(stat) = me.stat() {
                    // RSS in bytes (Resident Set Size)
                    let rss_bytes = stat.rss * 4096; // RSS is in pages, typically 4KB per page
                    PROCESS_MEMORY_RSS_BYTES.set(rss_bytes as f64);

                    // Virtual memory size in bytes
                    PROCESS_MEMORY_VIRTUAL_BYTES.set(stat.vsize as f64);
                }
            }
            Err(e) => {
                // Don't fail if we can't read memory stats
                tracing::debug!(error = %e, "Failed to read /proc memory stats");
            }
        }
    }

    // Histogram metrics (platform-independent)
    use crate::percentiles::{
        GLOBAL_REQUEST_PERCENTILES, GLOBAL_SCENARIO_PERCENTILES, GLOBAL_STEP_PERCENTILES,
    };

    let scenario_count = GLOBAL_SCENARIO_PERCENTILES.len();
    let step_count = GLOBAL_STEP_PERCENTILES.len();
    let request_count = if GLOBAL_REQUEST_PERCENTILES.stats().is_some() {
        1
    } else {
        0
    };
    let total_histograms = scenario_count + step_count + request_count;

    HISTOGRAM_COUNT.set(total_histograms as f64);

    // Estimate: 3MB per histogram (conservative average)
    let estimated_bytes = total_histograms * 3_000_000;
    HISTOGRAM_MEMORY_ESTIMATE_BYTES.set(estimated_bytes as f64);

    Ok(())
}
