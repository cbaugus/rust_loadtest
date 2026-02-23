//! gRPC worker communication protocol (Issue #46).
//!
//! Implements the `LoadTestCoordinator` gRPC service for all inter-node
//! communication:
//!
//! | RPC group        | Status          | Completed in |
//! |------------------|-----------------|--------------|
//! | Raft transport   | Implemented     | Issue #47    |
//! | Test coordination| Stubbed         | Issues #48/#76|
//! | Metrics streaming| Stubbed         | Issue #49    |
//! | Health check     | Implemented     | Issue #46    |
//!
//! All nodes listen on `CLUSTER_BIND_ADDR` (default `0.0.0.0:7000`).
//!
//! ## TLS
//! TLS is optional. GCP internal VPC traffic is encrypted at the network layer,
//! so plaintext gRPC is acceptable for intra-cluster traffic. Set
//! `CLUSTER_TLS=true` (future issue) to enable mutual TLS.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use serde_json;

use tonic::transport::{Channel, Endpoint, Server};
use tonic::{Request, Response, Status, Streaming};
use tracing::{error, info, warn};

use crate::cluster::ClusterHandle;
use crate::raft::{RaftNode, TypeConfig};

// ── Generated protobuf / gRPC code ───────────────────────────────────────────

/// Generated protobuf types and gRPC service stubs for `LoadTestCoordinator`.
///
/// These types are shared across the codebase:
/// - `proto::MetricsBatch` used by the metrics streaming implementation (Issue #49)
/// - `proto::AppendEntriesRequest` / `VoteRequest` used by Raft (Issue #47)
/// - `proto::TestConfig` / `StartRequest` used by test coordination (Issue #48)
pub mod proto {
    tonic::include_proto!("loadtest");
}

use proto::load_test_coordinator_client::LoadTestCoordinatorClient;
use proto::load_test_coordinator_server::{LoadTestCoordinator, LoadTestCoordinatorServer};
use proto::*;

// ── gRPC server implementation ────────────────────────────────────────────────

/// Server-side implementation of the `LoadTestCoordinator` gRPC service.
///
/// - `handle`: node state (for `HealthCheck`)
/// - `raft`: local Raft instance (for Raft transport RPCs, set when cluster enabled)
#[derive(Clone)]
pub struct LoadTestCoordinatorService {
    handle: ClusterHandle,
    raft: Option<Arc<RaftNode>>,
}

impl LoadTestCoordinatorService {
    pub fn new(handle: ClusterHandle) -> Self {
        Self { handle, raft: None }
    }

    pub fn with_raft(handle: ClusterHandle, raft: Arc<RaftNode>) -> Self {
        Self {
            handle,
            raft: Some(raft),
        }
    }
}

#[tonic::async_trait]
impl LoadTestCoordinator for LoadTestCoordinatorService {
    // ── Raft transport (Issue #47) ────────────────────────────────────────
    //
    // Each RPC deserialises the proto `payload` bytes back into the openraft
    // request type and forwards it to the local Raft instance.

    async fn append_entries(
        &self,
        req: Request<AppendEntriesRequest>,
    ) -> Result<Response<AppendEntriesResponse>, Status> {
        let raft = self.raft.as_ref().ok_or_else(|| {
            Status::unavailable("Cluster not enabled — cannot handle Raft AppendEntries")
        })?;

        let payload = req.into_inner().payload;
        let raft_req: openraft::raft::AppendEntriesRequest<TypeConfig> =
            serde_json::from_slice(&payload).map_err(|e| {
                Status::invalid_argument(format!("failed to decode AppendEntriesRequest: {}", e))
            })?;

        let resp = raft
            .raft
            .append_entries(raft_req)
            .await
            .map_err(|e| Status::internal(format!("Raft AppendEntries error: {}", e)))?;

        let payload = serde_json::to_vec(&resp)
            .map_err(|e| Status::internal(format!("failed to encode response: {}", e)))?;

        Ok(Response::new(AppendEntriesResponse {
            success: true,
            payload,
            ..Default::default()
        }))
    }

    async fn request_vote(
        &self,
        req: Request<VoteRequest>,
    ) -> Result<Response<VoteResponse>, Status> {
        let raft = self.raft.as_ref().ok_or_else(|| {
            Status::unavailable("Cluster not enabled — cannot handle Raft RequestVote")
        })?;

        let payload = req.into_inner().payload;
        let raft_req: openraft::raft::VoteRequest<crate::raft::NodeId> =
            serde_json::from_slice(&payload).map_err(|e| {
                Status::invalid_argument(format!("failed to decode VoteRequest: {}", e))
            })?;

        let resp = raft
            .raft
            .vote(raft_req)
            .await
            .map_err(|e| Status::internal(format!("Raft Vote error: {}", e)))?;

        let payload = serde_json::to_vec(&resp)
            .map_err(|e| Status::internal(format!("failed to encode response: {}", e)))?;

        Ok(Response::new(VoteResponse {
            vote_granted: resp.vote_granted,
            payload,
            ..Default::default()
        }))
    }

    async fn install_snapshot(
        &self,
        req: Request<SnapshotRequest>,
    ) -> Result<Response<SnapshotResponse>, Status> {
        let raft = self.raft.as_ref().ok_or_else(|| {
            Status::unavailable("Cluster not enabled — cannot handle Raft InstallSnapshot")
        })?;

        let payload = req.into_inner().payload;
        let raft_req: openraft::raft::InstallSnapshotRequest<TypeConfig> =
            serde_json::from_slice(&payload).map_err(|e| {
                Status::invalid_argument(format!("failed to decode InstallSnapshotRequest: {}", e))
            })?;

        let resp = raft
            .raft
            .install_snapshot(raft_req)
            .await
            .map_err(|e| Status::internal(format!("Raft InstallSnapshot error: {}", e)))?;

        let payload = serde_json::to_vec(&resp)
            .map_err(|e| Status::internal(format!("failed to encode response: {}", e)))?;

        Ok(Response::new(SnapshotResponse {
            payload,
            ..Default::default()
        }))
    }

    // ── Test coordination (Issue #79) ────────────────────────────────────
    //
    // DistributeConfig is the entry point for committing a new test config.
    // On the leader it calls client_write; openraft returns ForwardToLeader
    // if this node is not the leader, which we surface as an Internal error
    // so the caller (main.rs submission handler or a follower proxy) can
    // re-route to the correct peer.

    async fn distribute_config(&self, req: Request<TestConfig>) -> Result<Response<Ack>, Status> {
        let raft = self.raft.as_ref().ok_or_else(|| {
            Status::unavailable("Cluster not enabled — cannot handle DistributeConfig")
        })?;

        let inner = req.into_inner();
        raft.set_config(inner.yaml_content, inner.config_version)
            .await
            .map(|_| {
                Response::new(Ack {
                    ok: true,
                    message: "config committed to Raft log".to_string(),
                })
            })
            .map_err(|e| Status::internal(format!("Raft error: {}", e)))
    }

    async fn start_test(&self, _req: Request<StartRequest>) -> Result<Response<Ack>, Status> {
        Err(Status::unimplemented(
            "Coordinated start not yet implemented — see Issue #48",
        ))
    }

    async fn stop_test(&self, _req: Request<StopRequest>) -> Result<Response<Ack>, Status> {
        Err(Status::unimplemented(
            "Coordinated stop not yet implemented — see Issue #48",
        ))
    }

    // ── Metrics streaming — stubbed until Issue #49 ───────────────────────

    async fn stream_metrics(
        &self,
        _req: Request<Streaming<MetricsBatch>>,
    ) -> Result<Response<Ack>, Status> {
        Err(Status::unimplemented(
            "Metrics streaming not yet implemented — see Issue #49",
        ))
    }

    // ── Health check — implemented ────────────────────────────────────────

    async fn health_check(
        &self,
        _req: Request<HealthRequest>,
    ) -> Result<Response<HealthResponse>, Status> {
        let state = self.handle.state();
        let config = self.handle.config();
        Ok(Response::new(HealthResponse {
            node_id: config.node_id.clone(),
            state: state.as_str().to_string(),
            region: config.region.clone(),
            cluster_ready: state.cluster_ready(),
            peer_count: config.nodes.len() as u32,
        }))
    }
}

// ── gRPC server startup ───────────────────────────────────────────────────────

/// Starts the gRPC server bound to `CLUSTER_BIND_ADDR` (default `0.0.0.0:7000`).
///
/// When `raft` is `Some`, the Raft transport RPCs (`AppendEntries`, `RequestVote`,
/// `InstallSnapshot`) are handled by the local Raft instance. Without it they
/// return `Unavailable`.
///
/// Runs indefinitely; the caller should spawn this in a background task.
pub async fn start_grpc_server(handle: ClusterHandle, raft: Option<Arc<RaftNode>>) {
    let bind_addr = handle.config().bind_addr.clone();
    let addr: SocketAddr = bind_addr
        .parse()
        .unwrap_or_else(|_| ([0, 0, 0, 0], 7000).into());

    let service = match raft {
        Some(r) => LoadTestCoordinatorService::with_raft(handle, r),
        None => LoadTestCoordinatorService::new(handle),
    };

    info!(addr = %addr, "gRPC server starting (Issue #46/#47)");

    if let Err(e) = Server::builder()
        .add_service(LoadTestCoordinatorServer::new(service))
        .serve(addr)
        .await
    {
        error!(error = %e, "gRPC server error");
    }
}

// ── Peer client pool ──────────────────────────────────────────────────────────

/// Pool of `LoadTestCoordinatorClient` connections to cluster peers.
///
/// Connections are established lazily via [`connect_to_peers`] and retried
/// with exponential backoff (200 ms → 30 s cap) when a peer is unreachable.
///
/// The pool is `Clone`-able (backed by `Arc<Mutex<...>>`). The Raft state
/// machine (Issue #47) holds a clone to dispatch RPCs to followers.
///
/// [`connect_to_peers`]: PeerClientPool::connect_to_peers
#[derive(Clone, Default)]
pub struct PeerClientPool {
    clients: Arc<Mutex<HashMap<String, LoadTestCoordinatorClient<Channel>>>>,
}

impl PeerClientPool {
    pub fn new() -> Self {
        Self::default()
    }

    /// Spawn a background task per peer that connects with exponential backoff.
    ///
    /// Each task runs until the connection is established. Reconnection on
    /// later drops is handled by the Raft transport layer (Issue #47).
    pub fn connect_to_peers(&self, peers: Vec<String>) {
        for peer_addr in peers {
            let clients = self.clients.clone();
            tokio::spawn(async move {
                connect_with_backoff(peer_addr, clients).await;
            });
        }
    }

    /// Returns a cloned client handle for `peer_addr` if a connection exists.
    pub fn get(&self, peer_addr: &str) -> Option<LoadTestCoordinatorClient<Channel>> {
        self.clients.lock().unwrap().get(peer_addr).cloned()
    }

    /// Number of peers with an active gRPC connection.
    pub fn connected_count(&self) -> usize {
        self.clients.lock().unwrap().len()
    }
}

/// Connects to `peer_addr` with exponential backoff, inserting a ready client
/// into `clients` on success.
async fn connect_with_backoff(
    peer_addr: String,
    clients: Arc<Mutex<HashMap<String, LoadTestCoordinatorClient<Channel>>>>,
) {
    // Normalise to a full URI that tonic's Endpoint understands.
    let uri = if peer_addr.starts_with("http://") || peer_addr.starts_with("https://") {
        peer_addr.clone()
    } else {
        format!("http://{}", peer_addr)
    };

    let endpoint = match Endpoint::from_shared(uri) {
        Ok(ep) => ep,
        Err(e) => {
            error!(peer = %peer_addr, error = %e, "Invalid peer address — aborting reconnect");
            return;
        }
    };

    let mut backoff = Duration::from_millis(200);
    const MAX_BACKOFF: Duration = Duration::from_secs(30);

    loop {
        match endpoint.connect().await {
            Ok(channel) => {
                let client = LoadTestCoordinatorClient::new(channel);
                clients.lock().unwrap().insert(peer_addr.clone(), client);
                info!(peer = %peer_addr, "Connected to cluster peer");
                return;
            }
            Err(e) => {
                warn!(
                    peer = %peer_addr,
                    backoff_ms = backoff.as_millis(),
                    error = %e,
                    "Failed to connect to cluster peer, retrying"
                );
            }
        }

        tokio::time::sleep(backoff).await;
        backoff = (backoff * 2).min(MAX_BACKOFF);
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cluster::{ClusterConfig, ClusterHandle, NodeState};

    fn standalone_handle() -> ClusterHandle {
        ClusterHandle::new(ClusterConfig::for_testing())
    }

    fn cluster_handle(region: &str) -> ClusterHandle {
        let mut cfg = ClusterConfig::for_testing();
        cfg.enabled = true;
        cfg.region = region.to_string();
        ClusterHandle::new(cfg)
    }

    // ── Health check RPC ─────────────────────────────────────────────────

    #[tokio::test]
    async fn health_check_standalone() {
        let svc = LoadTestCoordinatorService::new(standalone_handle());
        let resp = svc
            .health_check(Request::new(HealthRequest {
                node_id: "test".to_string(),
            }))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.state, "standalone");
        assert_eq!(resp.region, "local");
        assert!(!resp.cluster_ready);
    }

    #[tokio::test]
    async fn health_check_forming() {
        let svc = LoadTestCoordinatorService::new(cluster_handle("us-central1"));
        let resp = svc
            .health_check(Request::new(HealthRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.state, "forming");
        assert_eq!(resp.region, "us-central1");
        assert!(!resp.cluster_ready);
    }

    #[tokio::test]
    async fn health_check_leader() {
        let handle = cluster_handle("europe-west1");
        handle.set_state(NodeState::Leader);
        let svc = LoadTestCoordinatorService::new(handle);
        let resp = svc
            .health_check(Request::new(HealthRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.state, "leader");
        assert!(resp.cluster_ready);
    }

    #[tokio::test]
    async fn health_check_follower() {
        let handle = cluster_handle("us-east1");
        handle.set_state(NodeState::Follower);
        let svc = LoadTestCoordinatorService::new(handle);
        let resp = svc
            .health_check(Request::new(HealthRequest::default()))
            .await
            .unwrap()
            .into_inner();

        assert_eq!(resp.state, "follower");
        assert!(resp.cluster_ready);
    }

    // ── RPCs return Unavailable without a raft node ───────────────────────

    #[tokio::test]
    async fn raft_append_entries_unavailable_without_raft() {
        let svc = LoadTestCoordinatorService::new(standalone_handle());
        let err = svc
            .append_entries(Request::new(AppendEntriesRequest::default()))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn raft_request_vote_unavailable_without_raft() {
        let svc = LoadTestCoordinatorService::new(standalone_handle());
        let err = svc
            .request_vote(Request::new(VoteRequest::default()))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn distribute_config_unavailable_without_raft() {
        // distribute_config is now implemented (Issue #79) and requires a
        // Raft node — without one it returns Unavailable, not Unimplemented.
        let svc = LoadTestCoordinatorService::new(standalone_handle());
        let err = svc
            .distribute_config(Request::new(TestConfig::default()))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unavailable);
    }

    #[tokio::test]
    async fn coordination_stubs_are_unimplemented() {
        let svc = LoadTestCoordinatorService::new(standalone_handle());

        let err = svc
            .start_test(Request::new(StartRequest::default()))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unimplemented);

        let err = svc
            .stop_test(Request::new(StopRequest::default()))
            .await
            .unwrap_err();
        assert_eq!(err.code(), tonic::Code::Unimplemented);
    }

    // ── PeerClientPool ────────────────────────────────────────────────────

    #[test]
    fn peer_pool_starts_empty() {
        let pool = PeerClientPool::new();
        assert_eq!(pool.connected_count(), 0);
        assert!(pool.get("10.0.0.1:7000").is_none());
    }

    #[test]
    fn peer_pool_clone_shares_state() {
        let pool1 = PeerClientPool::new();
        let pool2 = pool1.clone();
        // Both views are the same underlying map.
        assert_eq!(pool1.connected_count(), pool2.connected_count());
    }
}
