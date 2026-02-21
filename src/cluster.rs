//! Cluster mode infrastructure for multi-region distributed load testing (Issue #45).
//!
//! This module provides:
//! - Cluster configuration parsed from environment variables
//! - Node state tracking (Standalone, Forming, Follower, Leader)
//! - Health check HTTP endpoint consumed by Consul for service discovery
//!
//! Cluster mode is opt-in via `CLUSTER_ENABLED=true`. When disabled (the default),
//! the binary runs in standalone mode and no cluster infrastructure is started.
//!
//! ## Discovery modes
//!
//! **Static** (default, GCP): peers listed in `CLUSTER_NODES=ip1:7000,ip2:7000,...`
//!
//! **Consul** (local/dev): peers discovered via `loadtest-cluster.service.consul`.
//! Each node registers with Consul and exposes `/health/cluster`. Consul tags are
//! updated automatically as Raft state changes: `forming → follower → leader`.
//! The untagged DNS name resolves to all healthy nodes; tagged names
//! (`leader.loadtest-cluster.service.consul`) resolve to the specific role.
//!
//! ## Health check states
//!
//! | State      | Meaning                                         |
//! |------------|-------------------------------------------------|
//! | standalone | Cluster disabled — normal single-node operation |
//! | forming    | Cluster enabled, waiting to reach quorum        |
//! | follower   | In cluster, running as a Raft follower          |
//! | leader     | Elected Raft leader / test coordinator          |

use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use hyper::service::{make_service_fn, service_fn};
use hyper::{Body, Request, Response, Server};
use tracing::{error, info};

// ── Discovery mode ────────────────────────────────────────────────────────────

/// How this node discovers its cluster peers.
#[derive(Debug, Clone, PartialEq)]
pub enum DiscoveryMode {
    /// Peers specified as a static comma-separated list in `CLUSTER_NODES`.
    Static,
    /// Peers discovered via HashiCorp Consul DNS / HTTP API.
    Consul,
}

impl DiscoveryMode {
    fn from_env() -> Self {
        match std::env::var("DISCOVERY_MODE")
            .unwrap_or_else(|_| "static".to_string())
            .to_lowercase()
            .as_str()
        {
            "consul" => DiscoveryMode::Consul,
            _ => DiscoveryMode::Static,
        }
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            DiscoveryMode::Static => "static",
            DiscoveryMode::Consul => "consul",
        }
    }
}

// ── Cluster configuration ─────────────────────────────────────────────────────

/// Cluster configuration parsed from environment variables.
///
/// All fields have defaults so the struct is always constructable even when
/// `CLUSTER_ENABLED=false` (the default).
#[derive(Debug, Clone)]
pub struct ClusterConfig {
    /// Whether cluster mode is enabled. Default: `false`.
    pub enabled: bool,

    /// Stable node identity used in Raft and metric labels.
    /// Defaults to `HOSTNAME` env var, then `"unknown-node"`.
    pub node_id: String,

    /// Geographic region tag attached to all emitted metrics.
    /// Defaults to `"local"` in standalone mode, `"unknown"` in cluster mode
    /// unless `CLUSTER_REGION` is set.
    pub region: String,

    /// Address for the Raft + gRPC listener (used in Issue #46/#47).
    pub bind_addr: String,

    /// Address for the HTTP health check endpoint polled by Consul.
    pub health_addr: String,

    /// How peers are discovered.
    pub discovery_mode: DiscoveryMode,

    /// Peer addresses for static discovery (parsed from `CLUSTER_NODES`).
    pub nodes: Vec<String>,

    /// Consul agent HTTP address for Consul-based discovery.
    pub consul_addr: String,

    /// Consul service name this node registers as.
    pub consul_service_name: String,
}

impl ClusterConfig {
    /// Parse cluster configuration from environment variables.
    pub fn from_env() -> Self {
        let enabled = std::env::var("CLUSTER_ENABLED")
            .unwrap_or_else(|_| "false".to_string())
            .to_lowercase()
            == "true";

        let node_id = std::env::var("CLUSTER_NODE_ID").unwrap_or_else(|_| {
            std::env::var("HOSTNAME").unwrap_or_else(|_| "unknown-node".to_string())
        });

        let region = std::env::var("CLUSTER_REGION").unwrap_or_else(|_| {
            if enabled {
                "unknown".to_string()
            } else {
                "local".to_string()
            }
        });

        let bind_addr =
            std::env::var("CLUSTER_BIND_ADDR").unwrap_or_else(|_| "0.0.0.0:7000".to_string());

        let health_addr =
            std::env::var("CLUSTER_HEALTH_ADDR").unwrap_or_else(|_| "0.0.0.0:8080".to_string());

        let discovery_mode = DiscoveryMode::from_env();

        let nodes = std::env::var("CLUSTER_NODES")
            .unwrap_or_default()
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();

        let consul_addr =
            std::env::var("CONSUL_ADDR").unwrap_or_else(|_| "http://127.0.0.1:8500".to_string());

        let consul_service_name =
            std::env::var("CONSUL_SERVICE_NAME").unwrap_or_else(|_| "loadtest-cluster".to_string());

        Self {
            enabled,
            node_id,
            region,
            bind_addr,
            health_addr,
            discovery_mode,
            nodes,
            consul_addr,
            consul_service_name,
        }
    }

    /// Create a cluster config for testing purposes (cluster disabled).
    #[cfg(test)]
    pub fn for_testing() -> Self {
        Self {
            enabled: false,
            node_id: "test-node".to_string(),
            region: "local".to_string(),
            bind_addr: "0.0.0.0:7000".to_string(),
            health_addr: "0.0.0.0:8080".to_string(),
            discovery_mode: DiscoveryMode::Static,
            nodes: vec![],
            consul_addr: "http://127.0.0.1:8500".to_string(),
            consul_service_name: "loadtest-cluster".to_string(),
        }
    }
}

// ── Node state ────────────────────────────────────────────────────────────────

/// The current Raft state of this node.
///
/// State transitions when cluster is enabled:
/// ```text
/// Forming → Follower   (quorum reached, this node is a follower)
/// Forming → Leader     (quorum reached, this node won election)
/// Leader  → Follower   (leader loses election after partition/restart)
/// ```
///
/// In standalone mode the state is permanently `Standalone`.
#[derive(Debug, Clone, PartialEq)]
pub enum NodeState {
    /// Cluster mode disabled — node operates standalone.
    Standalone,
    /// Cluster enabled; waiting to reach quorum with peers.
    Forming,
    /// In cluster as a Raft follower.
    Follower,
    /// Elected Raft leader / test coordinator.
    Leader,
}

impl NodeState {
    /// Returns the lowercase string used in health responses and Consul tags.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeState::Standalone => "standalone",
            NodeState::Forming => "forming",
            NodeState::Follower => "follower",
            NodeState::Leader => "leader",
        }
    }

    /// Returns true once the node has joined the cluster and is ready to run load.
    pub fn cluster_ready(&self) -> bool {
        matches!(self, NodeState::Follower | NodeState::Leader)
    }
}

// ── Cluster handle ────────────────────────────────────────────────────────────

/// Shared cluster state handle — cheap to clone, safe to share across tasks.
///
/// Wraps an `Arc<Mutex<NodeState>>` so the health server, Raft state machine
/// (Issue #47), and the main test loop can all observe and update the node's
/// current state without coordination overhead.
#[derive(Clone)]
pub struct ClusterHandle {
    state: Arc<Mutex<NodeState>>,
    config: ClusterConfig,
}

impl ClusterHandle {
    /// Create a new handle. Initial state is `Forming` if cluster is enabled,
    /// `Standalone` otherwise.
    pub fn new(config: ClusterConfig) -> Self {
        let initial_state = if config.enabled {
            NodeState::Forming
        } else {
            NodeState::Standalone
        };
        Self {
            state: Arc::new(Mutex::new(initial_state)),
            config,
        }
    }

    /// Returns the current node state.
    pub fn state(&self) -> NodeState {
        self.state.lock().unwrap().clone()
    }

    /// Transitions to a new state.
    ///
    /// Called by the Raft implementation (Issue #47) when quorum is reached,
    /// leadership changes, or the cluster is shut down.
    pub fn set_state(&self, new_state: NodeState) {
        let old = {
            let mut guard = self.state.lock().unwrap();
            let old = guard.clone();
            *guard = new_state.clone();
            old
        };
        info!(
            node_id = %self.config.node_id,
            region = %self.config.region,
            old_state = old.as_str(),
            new_state = new_state.as_str(),
            "Cluster node state changed"
        );
    }

    /// The cluster configuration for this node.
    pub fn config(&self) -> &ClusterConfig {
        &self.config
    }

    /// The region label to attach to metrics.
    pub fn region(&self) -> &str {
        &self.config.region
    }
}

// ── Health server ─────────────────────────────────────────────────────────────

/// JSON body returned by `GET /health/cluster`.
///
/// Consul polls this endpoint every 5 seconds and updates the node's Consul
/// service tags based on the `state` field. The three states (`forming`,
/// `follower`, `leader`) map directly to Consul DNS tags:
///
/// - `loadtest-cluster.service.consul` — all healthy nodes
/// - `forming.loadtest-cluster.service.consul` — nodes still building quorum
/// - `follower.loadtest-cluster.service.consul` — active followers
/// - `leader.loadtest-cluster.service.consul` — the elected coordinator
#[derive(Debug, serde::Serialize)]
struct HealthResponse {
    state: String,
    node_id: String,
    region: String,
    cluster_enabled: bool,
    /// True once this node has joined the cluster and is ready to take load.
    cluster_ready: bool,
    /// Number of known peers (populated by Raft in Issue #47; 0 until then).
    peers: usize,
}

async fn health_handler(
    req: Request<Body>,
    handle: ClusterHandle,
) -> Result<Response<Body>, hyper::Error> {
    if req.uri().path() != "/health/cluster" {
        return Ok(Response::builder()
            .status(404)
            .body(Body::from("not found"))
            .unwrap());
    }

    let state = handle.state();
    let response = HealthResponse {
        state: state.as_str().to_string(),
        node_id: handle.config().node_id.clone(),
        region: handle.region().to_string(),
        cluster_enabled: handle.config().enabled,
        cluster_ready: state.cluster_ready(),
        peers: 0, // will be populated in Issue #47
    };

    let body = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
    Ok(Response::builder()
        .status(200)
        .header("Content-Type", "application/json")
        .body(Body::from(body))
        .unwrap())
}

/// Starts the cluster health check HTTP server.
///
/// Serves `GET /health/cluster` returning JSON with the node's current Raft
/// state. Consul polls this endpoint to classify the node and update its
/// DNS tags (`forming`, `follower`, `leader`).
///
/// All other paths return 404.
pub async fn start_health_server(handle: ClusterHandle) {
    let addr: SocketAddr = handle
        .config()
        .health_addr
        .parse()
        .unwrap_or_else(|_| ([0, 0, 0, 0], 8080).into());

    let make_svc = make_service_fn(move |_conn| {
        let handle_clone = handle.clone();
        async move {
            Ok::<_, hyper::Error>(service_fn(move |req| {
                let handle_inner = handle_clone.clone();
                async move { health_handler(req, handle_inner).await }
            }))
        }
    });

    let server = Server::bind(&addr).serve(make_svc);
    info!(
        addr = %addr,
        "Cluster health endpoint started — GET /health/cluster"
    );

    if let Err(e) = server.await {
        error!(error = %e, "Cluster health server error");
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standalone_mode_by_default() {
        let config = ClusterConfig::for_testing();
        let handle = ClusterHandle::new(config);
        assert_eq!(handle.state(), NodeState::Standalone);
        assert!(!handle.state().cluster_ready());
    }

    #[test]
    fn cluster_enabled_starts_forming() {
        let mut config = ClusterConfig::for_testing();
        config.enabled = true;
        config.region = "us-central1".to_string();
        let handle = ClusterHandle::new(config);
        assert_eq!(handle.state(), NodeState::Forming);
        assert!(!handle.state().cluster_ready());
    }

    #[test]
    fn state_transitions() {
        let mut config = ClusterConfig::for_testing();
        config.enabled = true;
        let handle = ClusterHandle::new(config);

        handle.set_state(NodeState::Follower);
        assert_eq!(handle.state(), NodeState::Follower);
        assert!(handle.state().cluster_ready());

        handle.set_state(NodeState::Leader);
        assert_eq!(handle.state(), NodeState::Leader);
        assert!(handle.state().cluster_ready());

        handle.set_state(NodeState::Follower);
        assert_eq!(handle.state(), NodeState::Follower);
    }

    #[test]
    fn node_state_strings() {
        assert_eq!(NodeState::Standalone.as_str(), "standalone");
        assert_eq!(NodeState::Forming.as_str(), "forming");
        assert_eq!(NodeState::Follower.as_str(), "follower");
        assert_eq!(NodeState::Leader.as_str(), "leader");
    }

    #[test]
    fn region_defaults_to_local_in_standalone() {
        let config = ClusterConfig::for_testing();
        assert_eq!(config.region, "local");
    }

    #[test]
    fn discovery_mode_defaults_to_static() {
        let config = ClusterConfig::for_testing();
        assert_eq!(config.discovery_mode, DiscoveryMode::Static);
    }

    #[test]
    fn static_nodes_parsed_from_string() {
        let nodes_str = "10.1.0.1:7000, 10.2.0.1:7000, 10.3.0.1:7000";
        let nodes: Vec<String> = nodes_str
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(String::from)
            .collect();
        assert_eq!(nodes.len(), 3);
        assert_eq!(nodes[0], "10.1.0.1:7000");
        assert_eq!(nodes[2], "10.3.0.1:7000");
    }

    #[test]
    fn handle_clone_shares_state() {
        let config = ClusterConfig::for_testing();
        let handle1 = ClusterHandle::new(config);
        let handle2 = handle1.clone();

        // Mutate via handle1, observe via handle2
        handle1.set_state(NodeState::Follower);
        assert_eq!(handle2.state(), NodeState::Follower);
    }
}
