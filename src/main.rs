use mimalloc::MiMalloc;

#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

use std::sync::{Arc, Mutex};
use tokio::sync::{mpsc, watch};
use tokio::time::{self, Duration};
use tracing::{error, info};
use tracing_subscriber::{fmt, EnvFilter};

use rust_loadtest::client::build_client;
use rust_loadtest::cluster::DiscoveryMode;
use rust_loadtest::cluster::{
    start_health_server, ClusterHandle, ConfigSubmission, NodeMetricsSnapshot,
};
use rust_loadtest::config::Config;
use rust_loadtest::connection_pool::{PoolConfig, GLOBAL_POOL_STATS};
use rust_loadtest::consul::{resolve_consul_peers_with_retry, start_consul_tagging};
use rust_loadtest::grpc::proto::load_test_coordinator_client::LoadTestCoordinatorClient;
use rust_loadtest::grpc::proto::TestConfig as ProtoTestConfig;
use rust_loadtest::grpc::{start_grpc_server, PeerClientPool};
use rust_loadtest::memory_guard::{
    init_percentile_tracking_flag, spawn_memory_guard, MemoryGuardConfig,
};
use rust_loadtest::metrics::CLUSTER_NODE_INFO;
use rust_loadtest::metrics::{
    gather_metrics_string, register_metrics, start_metrics_server, update_memory_metrics,
    CONNECTION_POOL_IDLE_TIMEOUT_SECONDS, CONNECTION_POOL_MAX_IDLE,
    PERCENTILE_SAMPLING_RATE_PERCENT, PROCESS_MEMORY_RSS_BYTES, REQUEST_ERRORS_BY_CATEGORY,
    REQUEST_TOTAL, WORKERS_CONFIGURED_TOTAL,
};
use rust_loadtest::percentiles::{
    format_percentile_table, rotate_all_histograms, GLOBAL_REQUEST_PERCENTILES,
    GLOBAL_SCENARIO_PERCENTILES, GLOBAL_STEP_PERCENTILES,
};
use rust_loadtest::raft::{node_id_from_str, start_raft_node};
use rust_loadtest::throughput::{format_throughput_table, GLOBAL_THROUGHPUT_TRACKER};
use rust_loadtest::worker::{run_worker, WorkerConfig};
use rust_loadtest::yaml_config::YamlConfig;

/// Initializes the tracing subscriber for structured logging.
fn init_tracing() {
    let log_format = std::env::var("LOG_FORMAT").unwrap_or_default();

    let env_filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("rust_loadtest=info"));

    if log_format == "json" {
        fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .json()
            .init();
    } else {
        fmt()
            .with_env_filter(env_filter)
            .with_target(true)
            .with_thread_ids(true)
            .init();
    }
}

/// Prints percentile latency statistics.
fn print_percentile_report(enabled: bool, sampling_rate: u8) {
    info!("\n{}", "=".repeat(120));
    info!("PERCENTILE LATENCY REPORT (Issue #33)");
    info!("{}", "=".repeat(120));

    if !enabled {
        info!("\n⚠️  Percentile tracking was DISABLED (PERCENTILE_TRACKING_ENABLED=false)");
        info!("No latency percentile data was collected to reduce memory usage.");
        info!("To enable percentile tracking, set PERCENTILE_TRACKING_ENABLED=true\n");
        info!("{}", "=".repeat(120));
        info!("END OF PERCENTILE REPORT");
        info!("{}\n", "=".repeat(120));
        return;
    }

    if sampling_rate < 100 {
        info!(
            "\n📊 Percentile sampling active: {}% of requests recorded \
             (PERCENTILE_SAMPLING_RATE={})",
            sampling_rate, sampling_rate
        );
    }

    // Single request percentiles
    if let Some(request_stats) = GLOBAL_REQUEST_PERCENTILES.stats() {
        info!("\n## Single Request Latencies\n");
        info!("{}", request_stats.format());
        info!("");
    } else {
        info!("\n## Single Request Latencies\n");
        info!("No single request data collected.\n");
    }

    // Scenario percentiles
    let scenario_stats = GLOBAL_SCENARIO_PERCENTILES.all_stats();
    if !scenario_stats.is_empty() {
        let scenario_table = format_percentile_table("Scenario Latencies", &scenario_stats);
        info!("{}", scenario_table);
    }

    // Step percentiles
    let step_stats = GLOBAL_STEP_PERCENTILES.all_stats();
    if !step_stats.is_empty() {
        let step_table = format_percentile_table("Step Latencies", &step_stats);
        info!("{}", step_table);
    }

    info!("{}", "=".repeat(120));
    info!("END OF PERCENTILE REPORT");
    info!("{}\n", "=".repeat(120));
}

/// Prints per-scenario throughput statistics.
fn print_throughput_report() {
    info!("\n{}", "=".repeat(120));
    info!("PER-SCENARIO THROUGHPUT REPORT (Issue #35)");
    info!("{}", "=".repeat(120));

    let all_stats = GLOBAL_THROUGHPUT_TRACKER.all_stats();

    if !all_stats.is_empty() {
        let table = format_throughput_table(&all_stats);
        info!("{}", table);

        let total_rps = GLOBAL_THROUGHPUT_TRACKER.total_throughput();
        let elapsed = GLOBAL_THROUGHPUT_TRACKER.elapsed();
        info!(
            "\nTotal Throughput: {:.2} scenarios/sec over {:.1}s",
            total_rps,
            elapsed.as_secs_f64()
        );
    } else {
        info!("\nNo scenario throughput data collected.\n");
    }

    info!("{}", "=".repeat(120));
    info!("END OF THROUGHPUT REPORT");
    info!("{}\n", "=".repeat(120));
}

/// Prints connection pool statistics.
fn print_pool_report() {
    info!("\n{}", "=".repeat(120));
    info!("CONNECTION POOL STATISTICS (Issue #36)");
    info!("{}", "=".repeat(120));

    let stats = GLOBAL_POOL_STATS.stats();

    if stats.total_requests > 0 {
        info!("\nConnection Reuse Analysis:");
        info!("  {}", stats.format());

        if let Some(duration) = stats.duration() {
            info!("  Duration: {:.1}s", duration.as_secs_f64());
        }

        info!("\nInterpretation:");
        if stats.reuse_rate() >= 80.0 {
            info!(
                "  ✅ Excellent connection reuse ({:.1}%)",
                stats.reuse_rate()
            );
            info!("     Most requests are reusing pooled connections efficiently.");
        } else if stats.reuse_rate() >= 50.0 {
            info!(
                "  ⚠️  Moderate connection reuse ({:.1}%)",
                stats.reuse_rate()
            );
            info!("     Consider increasing pool size or idle timeout.");
        } else {
            info!("  ❌ Low connection reuse ({:.1}%)", stats.reuse_rate());
            info!("     Many new connections are being established.");
            info!("     Check: pool configuration, connection timeouts, load patterns.");
        }

        info!("\nNote: Connection classification is based on latency patterns:");
        info!("  - Fast requests (<100ms) likely reused pooled connections");
        info!("  - Slow requests (≥100ms) likely established new connections (TLS handshake)");
    } else {
        info!("\nNo connection pool data collected.\n");
    }

    info!("\n{}", "=".repeat(120));
    info!("END OF POOL REPORT");
    info!("{}\n", "=".repeat(120));
}

/// Prints helpful configuration documentation.
fn print_config_help() {
    eprintln!("Required environment variables:");
    eprintln!(
        "  TARGET_URL              - The URL to load test (must start with http:// or https://)"
    );
    eprintln!();
    eprintln!("Optional environment variables:");
    eprintln!("  REQUEST_TYPE            - HTTP method: GET, POST, PUT, PATCH, DELETE, HEAD, OPTIONS (default: GET)");
    eprintln!("  SEND_JSON               - Send JSON payload: true or false (default: false)");
    eprintln!(
        "  JSON_PAYLOAD            - JSON body for POST/PUT/PATCH requests (required if SEND_JSON=true)"
    );
    eprintln!(
        "  NUM_CONCURRENT_TASKS    - Number of concurrent workers (default: 10, must be > 0)"
    );
    eprintln!("  TEST_DURATION           - Total test duration: 10m, 2h, 1d (default: 2h)");
    eprintln!();
    eprintln!("Load model configuration:");
    eprintln!("  LOAD_MODEL_TYPE         - Concurrent, Rps, RampRps, or DailyTraffic (default: Concurrent)");
    eprintln!("    Rps model requires:");
    eprintln!("      TARGET_RPS          - Target requests per second");
    eprintln!("    RampRps model requires:");
    eprintln!("      MIN_RPS             - Starting requests per second");
    eprintln!("      MAX_RPS             - Peak requests per second");
    eprintln!("      RAMP_DURATION       - Duration to ramp (default: TEST_DURATION)");
    eprintln!("    DailyTraffic model requires:");
    eprintln!("      DAILY_MIN_RPS       - Minimum (nighttime) RPS");
    eprintln!("      DAILY_MID_RPS       - Medium (afternoon) RPS");
    eprintln!("      DAILY_MAX_RPS       - Maximum (peak) RPS");
    eprintln!("      DAILY_CYCLE_DURATION - Full cycle duration (e.g., 1d)");
    eprintln!();
    eprintln!("TLS/mTLS configuration:");
    eprintln!("  SKIP_TLS_VERIFY         - Skip TLS certificate verification (default: false)");
    eprintln!("  CLIENT_CERT_PATH        - Path to client certificate for mTLS");
    eprintln!("  CLIENT_KEY_PATH         - Path to client key for mTLS");
    eprintln!("  Note: Both CLIENT_CERT_PATH and CLIENT_KEY_PATH must be set together");
    eprintln!();
    eprintln!("Advanced configuration:");
    eprintln!("  RESOLVE_TARGET_ADDR     - DNS override: hostname:ip:port");
    eprintln!("  CUSTOM_HEADERS          - Comma-separated headers (use \\, for literal commas)");
    eprintln!("  METRIC_NAMESPACE        - Prometheus metric namespace (default: rust_loadtest)");
    eprintln!();
    eprintln!("Connection pool configuration:");
    eprintln!("  POOL_MAX_IDLE_PER_HOST  - Max idle connections per host (default: 32)");
    eprintln!("  POOL_IDLE_TIMEOUT_SECS  - Idle connection timeout in seconds (default: 30)");
    eprintln!(
        "  TCP_NODELAY             - Disable Nagle's algorithm for lower latency (default: true)"
    );
    eprintln!("  REQUEST_TIMEOUT_SECS    - Per-request timeout in seconds (default: 30)");
    eprintln!();
    eprintln!("Cluster configuration (Issue #45 — disabled by default):");
    eprintln!("  CLUSTER_ENABLED         - Enable distributed cluster mode (default: false)");
    eprintln!("  CLUSTER_REGION          - Geographic region label for metrics (e.g. us-central1)");
    eprintln!("  CLUSTER_NODE_ID         - Stable node identity (default: $HOSTNAME)");
    eprintln!("  CLUSTER_BIND_ADDR       - Raft + gRPC listen address (default: 0.0.0.0:7000)");
    eprintln!(
        "  CLUSTER_HEALTH_ADDR     - Health check HTTP listen address (default: 0.0.0.0:8080)"
    );
    eprintln!("  DISCOVERY_MODE          - Peer discovery: static or consul (default: static)");
    eprintln!("  CLUSTER_NODES           - Comma-separated peer list for static discovery");
    eprintln!("                            e.g. 10.1.0.5:7000,10.2.0.5:7000,10.3.0.5:7000");
    eprintln!("  CONSUL_ADDR             - Consul agent address (default: http://127.0.0.1:8500)");
    eprintln!("  CONSUL_SERVICE_NAME     - Consul service name (default: loadtest-cluster)");
    eprintln!();
    eprintln!("Cluster config auto-fetch (Issue #76):");
    eprintln!(
        "  CLUSTER_CONFIG_SOURCE   - External config source: gcs or consul-kv (default: unset)"
    );
    eprintln!(
        "  GCS_CONFIG_BUCKET       - GCS bucket name (required if CLUSTER_CONFIG_SOURCE=gcs)"
    );
    eprintln!("  GCS_CONFIG_OBJECT       - GCS object path, e.g. configs/prod.yaml");
    eprintln!("  CONSUL_CONFIG_KEY       - Consul KV path (default: loadtest/config)");
    eprintln!("  CLUSTER_CONFIG_TIMEOUT_SECS - Fetch timeout in seconds (default: 30)");
    eprintln!();
    eprintln!("Logging configuration:");
    eprintln!("  RUST_LOG                - Log level: error, warn, info, debug, trace");
    eprintln!("                            Examples: RUST_LOG=info, RUST_LOG=rust_loadtest=debug");
    eprintln!("  LOG_FORMAT              - Output format: json or default (human-readable)");
}

/// Worker pool managed by the config-watcher task (Issue #79).
///
/// Holds the stop-signal sender and the JoinHandles of config-watcher-spawned
/// workers (not the initial startup workers — those hold a clone of the same
/// `stop_tx` so they still receive the stop signal).
struct WorkerPool {
    stop_tx: watch::Sender<bool>,
    handles: Vec<tokio::task::JoinHandle<()>>,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // Initialize tracing subscriber
    init_tracing();

    // Register Prometheus metrics
    register_metrics()?;

    // Load configuration from environment variables
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            error!(error = %e, "Configuration error");
            eprintln!("Configuration error: {}\n", e);
            print_config_help();
            std::process::exit(1);
        }
    };

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

    info!(
        metrics_port = metrics_port,
        "Prometheus metrics server started"
    );

    // Initialize cluster node (Issue #45)
    let cluster_handle = ClusterHandle::new(config.cluster.clone());
    CLUSTER_NODE_INFO
        .with_label_values(&[
            &config.cluster.node_id,
            &config.cluster.region,
            cluster_handle.state().as_str(),
        ])
        .set(1.0);

    // Stop-signal channel: shared by all workers.  The config-watcher task
    // (cluster mode) sends `true` to drain workers before reconfiguration.
    // In standalone mode the signal is never sent; workers self-terminate
    // via the duration check.  (Issue #79)
    let (worker_stop_tx, worker_stop_rx) = watch::channel(false);

    // Worker pool managed by the config-watcher (cluster mode only).
    // Initially empty — the startup workers hold stop_rx clones but are not
    // tracked here; the stop signal propagates to them regardless.
    let worker_pool = Arc::new(tokio::sync::Mutex::new(WorkerPool {
        stop_tx: worker_stop_tx,
        handles: Vec::new(),
    }));

    if config.cluster.enabled {
        // Config-submission channel: health server → main handler task.
        // Created first so it can be passed to the health server.
        let (config_sub_tx, mut config_sub_rx) = mpsc::unbounded_channel::<ConfigSubmission>();

        // HTTP health + config-submission endpoint (Issues #45, #79)
        let health_handle = cluster_handle.clone();
        let sub_tx_for_health = config_sub_tx.clone();
        tokio::spawn(async move {
            start_health_server(health_handle, Some(sub_tx_for_health)).await;
        });

        // Consul service registration + tag updates (Issue #47).
        // Register *before* resolving peers so this node is visible to others.
        start_consul_tagging(&cluster_handle);

        // Build the peer list used for Raft initialization (Issues #80 / #81).
        //
        // Static:  addresses come directly from CLUSTER_NODES.
        // Consul:  query the Consul catalog and wait until min_peers others
        //          have registered (with a 60-second timeout).
        //
        // In both cases peer IDs are derived from the address strings, which
        // must match the ID this node derives from CLUSTER_SELF_ADDR.
        let peer_addrs: Vec<String> = if config.cluster.discovery_mode == DiscoveryMode::Consul {
            // min_peers = CLUSTER_MIN_PEERS (default 1).
            // For a 3-node cluster set CLUSTER_MIN_PEERS=2 so we wait for
            // all three to register before electing a leader.
            let min = config.cluster.min_peers + 1; // include self
            resolve_consul_peers_with_retry(
                &config.cluster.consul_addr,
                &config.cluster.consul_service_name,
                min,
                tokio::time::Duration::from_secs(60),
            )
            .await
        } else {
            config.cluster.nodes.clone()
        };

        // Deduplicate: both the Nomad service block and the ConsulClient can
        // register the same service, producing duplicate addresses in the catalog.
        // Use a BTreeMap keyed by node ID so each physical node appears once.
        use std::collections::BTreeMap;
        let peers: Vec<(u64, String)> = {
            let mut seen: BTreeMap<u64, String> = BTreeMap::new();
            for addr in peer_addrs {
                let id = node_id_from_str(&addr);
                seen.entry(id).or_insert(addr);
            }
            seen.into_iter().collect()
        };

        info!(
            mode = config.cluster.discovery_mode.as_str(),
            peers = peers.len(),
            self_addr = config
                .cluster
                .self_addr
                .as_deref()
                .unwrap_or("(not set — using hostname)"),
            "Peer list resolved for Raft initialization"
        );

        // Raft node — embedded leader election (Issue #47)
        let raft_node = start_raft_node(cluster_handle.clone(), peers.clone()).await;

        // gRPC server with Raft transport enabled (Issues #46 / #47)
        let grpc_handle = cluster_handle.clone();
        let grpc_raft = raft_node.clone();
        tokio::spawn(async move {
            start_grpc_server(grpc_handle, Some(grpc_raft)).await;
        });

        // ── Config-submission handler (Issue #79) ─────────────────────────
        // Receives ConfigSubmission from the HTTP POST /cluster/config handler.
        // On leader: commits directly via raft_node.set_config().
        // On follower: proxies to the current leader's DistributeConfig gRPC.
        {
            let raft_for_sub = raft_node.clone();
            let peers_for_sub = peers.clone();
            tokio::spawn(async move {
                // Timeout applied to every config commit or proxy call so the
                // HTTP handler never blocks indefinitely (e.g. when Raft loses
                // quorum during a rolling restart).
                const COMMIT_TIMEOUT_SECS: u64 = 30;

                while let Some(sub) = config_sub_rx.recv().await {
                    let result: Result<(), String> = if raft_for_sub.is_leader() {
                        info!(version = %sub.version, "Committing config to Raft log (leader)");
                        match tokio::time::timeout(
                            Duration::from_secs(COMMIT_TIMEOUT_SECS),
                            raft_for_sub.set_config(sub.yaml, sub.version),
                        )
                        .await
                        {
                            Ok(Ok(_)) => Ok(()),
                            Ok(Err(e)) => Err(e.to_string()),
                            Err(_) => Err(format!(
                                "Raft commit timed out after {}s — \
                                 cluster may have lost quorum, retry in a few seconds",
                                COMMIT_TIMEOUT_SECS
                            )),
                        }
                    } else {
                        // Find the current leader and proxy via gRPC.
                        let leader_id = raft_for_sub.raft.metrics().borrow().current_leader;
                        match leader_id {
                            Some(lid) if lid != raft_for_sub.node_id => {
                                let addr = peers_for_sub
                                    .iter()
                                    .find(|(id, _)| *id == lid)
                                    .map(|(_, a)| a.clone());
                                match addr {
                                    Some(addr) => {
                                        info!(
                                            leader = %addr,
                                            version = %sub.version,
                                            "Proxying config submission to Raft leader"
                                        );
                                        let uri = format!("http://{}", addr);
                                        match tonic::transport::Endpoint::from_shared(uri)
                                            .map_err(|e| e.to_string())
                                        {
                                            Ok(ep) => match ep.connect().await {
                                                Ok(ch) => {
                                                    let mut client =
                                                        LoadTestCoordinatorClient::new(ch);
                                                    match tokio::time::timeout(
                                                        Duration::from_secs(COMMIT_TIMEOUT_SECS),
                                                        client.distribute_config(ProtoTestConfig {
                                                            yaml_content: sub.yaml,
                                                            config_version: sub.version,
                                                            start_at_unix_ms: 0,
                                                        }),
                                                    )
                                                    .await
                                                    {
                                                        Ok(Ok(_)) => Ok(()),
                                                        Ok(Err(e)) => {
                                                            // Translate gRPC failed_precondition
                                                            // (ForwardToLeader) into a clean msg.
                                                            let msg = e.message().to_string();
                                                            if msg.starts_with("not the leader") {
                                                                Err(format!(
                                                                    "leader changed mid-request: \
                                                                     {} — retry in a few seconds",
                                                                    msg
                                                                ))
                                                            } else {
                                                                Err(msg)
                                                            }
                                                        }
                                                        Err(_) => Err(format!(
                                                            "gRPC proxy to leader timed out \
                                                             after {}s",
                                                            COMMIT_TIMEOUT_SECS
                                                        )),
                                                    }
                                                }
                                                Err(e) => Err(format!(
                                                    "gRPC connect to leader failed: {}",
                                                    e
                                                )),
                                            },
                                            Err(e) => Err(e),
                                        }
                                    }
                                    None => {
                                        Err(format!("leader node {} not found in peer list", lid))
                                    }
                                }
                            }
                            _ => Err("no leader elected yet — retry in a few seconds".to_string()),
                        }
                    };
                    if let Err(ref e) = result {
                        error!(error = %e, "Config submission failed");
                    }
                    let _ = sub.respond.send(result);
                }
            });
        }

        // ── Config-watcher / worker-pool reconfiguration (Issue #79) ──────
        // Subscribes to the Raft watch channel (fixed in Issue #78).
        // When a new config is committed:
        //   1. Signals workers to stop (they exit cleanly between requests).
        //   2. Waits 5 s for in-flight requests to finish.
        //   3. Aborts any still-running workers.
        //   4. Parses the new YAML and spawns a fresh worker pool.
        {
            let pool_for_watcher = worker_pool.clone();
            let raft_for_watcher = raft_node.clone();
            let client_for_watcher = client.clone();
            let region_for_watcher = config.cluster.region.clone();
            tokio::spawn(async move {
                let mut config_rx = raft_for_watcher.config_receiver();
                loop {
                    if config_rx.changed().await.is_err() {
                        break;
                    }
                    let yaml = match config_rx.borrow().clone() {
                        Some(y) => y,
                        None => continue,
                    };

                    // Parse YAML → Config.  YAML values are authoritative: the
                    // document was deliberately pushed/fetched to replace the
                    // running config, so startup env-var defaults
                    // (NUM_CONCURRENT_TASKS, TARGET_RPS, TARGET_URL, …) must
                    // not shadow the YAML values.
                    let new_cfg = match serde_yaml::from_str::<YamlConfig>(&yaml) {
                        Ok(yaml_cfg) => match Config::from_yaml(&yaml_cfg) {
                            Ok(c) => c,
                            Err(e) => {
                                error!(error = %e, "Raft config YAML failed Config validation");
                                continue;
                            }
                        },
                        Err(e) => {
                            error!(error = %e, "Failed to parse Raft config YAML");
                            continue;
                        }
                    };

                    info!(
                        workers = new_cfg.num_concurrent_tasks,
                        url = %new_cfg.target_url,
                        load_model = ?new_cfg.load_model,
                        "Raft config committed — draining worker pool (Issue #79)"
                    );

                    // Signal graceful stop (workers exit after current request).
                    {
                        let state = pool_for_watcher.lock().await;
                        let _ = state.stop_tx.send(true);
                    }
                    // 5 s grace period for in-flight requests to complete.
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    // Abort any handles still running past the grace window.
                    let stale: Vec<_> = pool_for_watcher.lock().await.handles.drain(..).collect();
                    for h in stale {
                        h.abort();
                    }

                    // Rebuild HTTP client in case TLS/pool config changed.
                    let new_client =
                        match rust_loadtest::client::build_client(&new_cfg.to_client_config()) {
                            Ok(r) => r.client,
                            Err(e) => {
                                error!(
                                    error = %e,
                                    "Failed to build HTTP client for new config — reusing existing"
                                );
                                client_for_watcher.clone()
                            }
                        };

                    let (new_stop_tx, new_stop_rx) = watch::channel(false);
                    let new_start = time::Instant::now();
                    let new_handles: Vec<_> = (0..new_cfg.num_concurrent_tasks)
                        .map(|i| {
                            let wc = WorkerConfig {
                                task_id: i,
                                url: new_cfg.target_url.clone(),
                                request_type: new_cfg.request_type.clone(),
                                send_json: new_cfg.send_json,
                                json_payload: new_cfg.json_payload.clone(),
                                test_duration: new_cfg.test_duration,
                                load_model: new_cfg.load_model.clone(),
                                num_concurrent_tasks: new_cfg.num_concurrent_tasks,
                                percentile_tracking_enabled: new_cfg.percentile_tracking_enabled,
                                percentile_sampling_rate: new_cfg.percentile_sampling_rate,
                                region: region_for_watcher.clone(),
                                stop_rx: new_stop_rx.clone(),
                            };
                            tokio::spawn(run_worker(new_client.clone(), wc, new_start))
                        })
                        .collect();

                    {
                        let mut state = pool_for_watcher.lock().await;
                        state.stop_tx = new_stop_tx;
                        state.handles = new_handles;
                    }

                    WORKERS_CONFIGURED_TOTAL.set(new_cfg.num_concurrent_tasks as f64);
                    info!(
                        workers = new_cfg.num_concurrent_tasks,
                        url = %new_cfg.target_url,
                        "Worker pool reconfigured from Raft config (Issue #79)"
                    );
                }
            });
        }

        // ── Leader config auto-fetch (Issue #76) ──────────────────────────
        if let Some(config_source) = rust_loadtest::config_source::ConfigSource::from_env() {
            let raft_for_fetch = raft_node.clone();
            let client_for_fetch = client.clone();
            tokio::spawn(async move {
                let timeout_secs = std::env::var("CLUSTER_CONFIG_TIMEOUT_SECS")
                    .ok()
                    .and_then(|v| v.parse::<u64>().ok())
                    .unwrap_or(30);
                let mut metrics_rx = raft_for_fetch.raft.metrics();
                let mut was_leader = false;
                loop {
                    if metrics_rx.changed().await.is_err() {
                        break;
                    }
                    let is_leader =
                        metrics_rx.borrow().current_leader == Some(raft_for_fetch.node_id);
                    if is_leader && !was_leader {
                        match tokio::time::timeout(
                            Duration::from_secs(timeout_secs),
                            config_source.fetch(&client_for_fetch),
                        )
                        .await
                        {
                            Ok(Ok(yaml)) => {
                                let secs = std::time::SystemTime::now()
                                    .duration_since(std::time::UNIX_EPOCH)
                                    .unwrap_or_default()
                                    .as_secs();
                                let version = format!("auto-{}", secs);
                                match tokio::time::timeout(
                                    Duration::from_secs(timeout_secs),
                                    raft_for_fetch.set_config(yaml, version),
                                )
                                .await
                                {
                                    Ok(Ok(_)) => {
                                        info!("Leader committed auto-fetched config (Issue #76)")
                                    }
                                    Ok(Err(e)) => {
                                        // Detect ForwardToLeader (stepped down mid-commit) and
                                        // emit a clean message rather than raw openraft internals.
                                        use openraft::error::{ClientWriteError, RaftError};
                                        let msg = if let RaftError::APIError(
                                            ClientWriteError::ForwardToLeader(ref fwd),
                                        ) = e
                                        {
                                            let addr = fwd
                                                .leader_node
                                                .as_ref()
                                                .map(|n| n.addr.as_str())
                                                .unwrap_or("unknown");
                                            format!(
                                                "stepped down before commit completed \
                                                 (new leader: {}) — will retry on next election",
                                                addr
                                            )
                                        } else {
                                            e.to_string()
                                        };
                                        error!(error = %msg, "Failed to commit fetched config");
                                    }
                                    Err(_) => error!(
                                        timeout_secs,
                                        "Raft commit timed out after fetching config — \
                                         cluster may have lost quorum"
                                    ),
                                }
                            }
                            Ok(Err(e)) => {
                                error!(error = %e, "Config fetch from external source failed")
                            }
                            Err(_) => error!(timeout_secs, "Config fetch timed out"),
                        }
                    }
                    was_leader = is_leader;
                }
            });
        }

        // Outbound peer connections (PeerClientPool)
        if !config.cluster.nodes.is_empty() {
            let pool = PeerClientPool::new();
            pool.connect_to_peers(config.cluster.nodes.clone());
            info!(
                peer_count = config.cluster.nodes.len(),
                "Connecting to cluster peers"
            );
        }
    } else {
        // Standalone — gRPC server without Raft (serves health check only)
        let grpc_handle = cluster_handle.clone();
        tokio::spawn(async move {
            start_grpc_server(grpc_handle, None).await;
        });
    }

    // Initialize percentile tracking runtime flag (Issue #72)
    init_percentile_tracking_flag(config.percentile_tracking_enabled);
    if config.percentile_tracking_enabled {
        info!("Percentile tracking initialized and enabled");
    } else {
        info!("Percentile tracking initialized but DISABLED via config");
    }

    // Spawn auto-OOM memory guard (Issue #72)
    if config.percentile_tracking_enabled {
        let memory_guard_config = MemoryGuardConfig {
            warning_threshold_percent: config.memory_warning_threshold_percent,
            critical_threshold_percent: config.memory_critical_threshold_percent,
            auto_disable_on_warning: config.auto_disable_percentiles_on_warning,
            check_interval: Duration::from_secs(5),
        };
        tokio::spawn(async move {
            spawn_memory_guard(memory_guard_config).await;
        });
    } else {
        info!("Memory guard not started - percentile tracking disabled via config");
    }

    // Spawn memory monitoring task (Issue #69).
    // Also calls mi_collect() every 30s to return mimalloc arena pages to the
    // OS — without this, mimalloc retains freed pages as allocator caches which
    // shows up as ever-growing RSS under sustained high-throughput load.
    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(10));
        let mut collect_ticks: u32 = 0;
        loop {
            interval.tick().await;
            if let Err(e) = update_memory_metrics() {
                error!(error = %e, "Failed to update memory metrics");
            }
            collect_ticks += 1;
            if collect_ticks.is_multiple_of(3) {
                // Every 30s: ask mimalloc to return cached pages to the OS.
                // mi_collect(true) collects all arenas, not just the calling thread.
                unsafe { libmimalloc_sys::mi_collect(true) };
            }
        }
    });
    info!("Memory monitoring started (updates every 10s, mi_collect every 30s)");

    // Spawn health-endpoint metrics updater — refreshes per-node RPS, error
    // rate, worker count, memory and CPU once per second so the loadtest-control
    // web app can display live stats without scraping Prometheus.
    {
        use rust_loadtest::errors::ErrorCategory;
        let metrics_handle = cluster_handle.clone();
        let region = config.cluster.region.clone();
        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(1));
            let mut prev_requests: u64 = 0;
            let mut prev_errors: u64 = 0;
            // CPU tracking (Linux only) — tracks utime+stime jiffies
            #[cfg(target_os = "linux")]
            let mut prev_cpu_ticks: Option<u64> = None;

            loop {
                interval.tick().await;

                // ── Request counter (monotonic) ──────────────────────────
                let curr_requests = REQUEST_TOTAL.with_label_values(&[&region]).get();

                // ── Error counter: sum all known categories ───────────────
                let curr_errors: u64 = ErrorCategory::all()
                    .iter()
                    .map(|cat| {
                        REQUEST_ERRORS_BY_CATEGORY
                            .with_label_values(&[cat.label(), &region])
                            .get()
                    })
                    .sum();

                let delta_req = curr_requests.saturating_sub(prev_requests);
                let delta_err = curr_errors.saturating_sub(prev_errors);
                let rps = delta_req as f64;
                let error_rate_pct = if delta_req > 0 {
                    (delta_err as f64 / delta_req as f64) * 100.0
                } else {
                    0.0
                };

                // ── Workers & memory ─────────────────────────────────────
                let workers = WORKERS_CONFIGURED_TOTAL.get() as u32;
                let memory_mb = PROCESS_MEMORY_RSS_BYTES.get() / (1024.0 * 1024.0);

                // ── Total memory limit (cgroup → system fallback) ─────────
                // Mirror the logic in memory_guard::detect_memory_limit so the
                // health endpoint can expose memory_used% without a Prometheus query.
                let total_memory_mb: f64 = {
                    // cgroup v2
                    let v2 = std::fs::read_to_string("/sys/fs/cgroup/memory.max")
                        .ok()
                        .and_then(|s| {
                            let t = s.trim();
                            if t == "max" {
                                None
                            } else {
                                t.parse::<u64>().ok()
                            }
                        });
                    // cgroup v1
                    let v1 = std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes")
                        .ok()
                        .and_then(|s| s.trim().parse::<u64>().ok())
                        .filter(|&b| b < u64::MAX / 2); // ignore sentinel "unlimited" values

                    #[cfg(target_os = "linux")]
                    let system = {
                        use procfs::{Current, Meminfo};
                        Meminfo::current().ok().map(|m| m.mem_total)
                    };
                    #[cfg(not(target_os = "linux"))]
                    let system: Option<u64> = None;

                    v2.or(v1)
                        .or(system)
                        .map(|bytes| bytes as f64 / (1024.0 * 1024.0))
                        .unwrap_or(0.0)
                };

                // ── CPU % (Linux only via procfs) ────────────────────────
                // Reports percentage of one CPU core consumed by this process
                // in the last second (100 = fully saturating one core).
                // Computed from utime+stime delta in jiffies (CLK_TCK=100).
                #[cfg(target_os = "linux")]
                let cpu_pct = {
                    use procfs::process::Process;
                    let mut pct = 0.0_f64;
                    if let Ok(me) = Process::myself() {
                        if let Ok(stat) = me.stat() {
                            let proc_ticks = stat.utime + stat.stime;
                            if let Some(prev) = prev_cpu_ticks {
                                // delta ticks / CLK_TCK (100) * 100 = pct of one core
                                pct = proc_ticks.saturating_sub(prev) as f64;
                            }
                            prev_cpu_ticks = Some(proc_ticks);
                        }
                    }
                    pct
                };
                #[cfg(not(target_os = "linux"))]
                let cpu_pct = 0.0_f64;

                metrics_handle.update_metrics(NodeMetricsSnapshot {
                    rps,
                    error_rate_pct,
                    workers,
                    memory_mb,
                    total_memory_mb,
                    cpu_pct,
                });

                prev_requests = curr_requests;
                prev_errors = curr_errors;
            }
        });
    }

    // Spawn histogram rotation task if enabled (Issue #67)
    if config.histogram_rotation_interval.as_secs() > 0 {
        let rotation_interval = config.histogram_rotation_interval;
        tokio::spawn(async move {
            let mut interval = time::interval(rotation_interval);
            interval.tick().await; // Skip the first immediate tick
            loop {
                interval.tick().await;
                info!(
                    rotation_interval_secs = rotation_interval.as_secs(),
                    "Rotating histograms - clearing percentile data to free memory"
                );
                rotate_all_histograms();
                info!("Histogram rotation complete - memory freed");
            }
        });
        info!(
            rotation_interval_secs = config.histogram_rotation_interval.as_secs(),
            "Histogram rotation enabled - will rotate every {} seconds",
            config.histogram_rotation_interval.as_secs()
        );
    }

    // Initialize connection pool configuration metrics (Issue #36)
    let pool_config = PoolConfig::from_env();
    CONNECTION_POOL_MAX_IDLE.set(pool_config.max_idle_per_host as f64);
    CONNECTION_POOL_IDLE_TIMEOUT_SECONDS.set(pool_config.idle_timeout.as_secs() as f64);
    info!(
        max_idle_per_host = pool_config.max_idle_per_host,
        idle_timeout_secs = pool_config.idle_timeout.as_secs(),
        "Connection pool configuration initialized"
    );

    // Initialize test configuration metrics
    WORKERS_CONFIGURED_TOTAL.set(config.num_concurrent_tasks as f64);
    PERCENTILE_SAMPLING_RATE_PERCENT.set(config.percentile_sampling_rate as f64);

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
            percentile_tracking_enabled: config.percentile_tracking_enabled,
            percentile_sampling_rate: config.percentile_sampling_rate,
            region: config.cluster.region.clone(),
            // Graceful-stop signal (Issue #79). In cluster mode the
            // config-watcher fires this before replacing the worker pool.
            // In standalone mode it is never fired; workers self-terminate
            // via the test-duration check.
            stop_rx: worker_stop_rx.clone(),
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
    info!(
        duration_secs = config.test_duration.as_secs(),
        "Test duration completed, signalling workers to stop"
    );

    // Brief pause to allow in-flight metrics to be updated
    tokio::time::sleep(Duration::from_secs(2)).await;
    info!("Collecting final metrics");

    // Print percentile latency statistics (Issue #33, #66)
    print_percentile_report(
        config.percentile_tracking_enabled,
        config.percentile_sampling_rate,
    );

    // Print per-scenario throughput statistics (Issue #35)
    print_throughput_report();

    // Print connection pool statistics (Issue #36)
    print_pool_report();

    // Gather and print final metrics
    let final_metrics_output = gather_metrics_string(&registry_arc);
    info!("\n--- FINAL METRICS ---\n{}", final_metrics_output);
    info!("--- END OF FINAL METRICS ---");

    info!("Pausing for 2 minutes to allow final Prometheus scrape");
    tokio::time::sleep(Duration::from_secs(120)).await;
    info!("Pause complete, exiting");

    Ok(())
}
