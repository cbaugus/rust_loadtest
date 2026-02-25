//! Raft consensus implementation (Issue #47).
//!
//! Provides leader election and distributed coordination for cluster mode via
//! the `openraft` crate (version 0.9). Each node runs an embedded Raft state
//! machine — no external consensus service required.
//!
//! ## Storage
//!
//! Uses openraft's `Adaptor` to bridge an in-memory combined `RaftStorage`
//! implementation (v1 API) into the v2 `RaftLogStorage` + `RaftStateMachine`
//! split interface required by `Raft::new`.
//!
//! ## Transport
//!
//! openraft's `AppendEntries`, `Vote`, and `InstallSnapshot` requests are
//! JSON-serialized and sent over the `LoadTestCoordinator` gRPC service
//! defined in `proto/loadtest.proto` (Issue #46). The proto `payload: bytes`
//! field carries the serialized openraft payload.
//!
//! ## Implementation note on async traits
//!
//! openraft uses the `#[add_async_trait]` macro (RPITIT — return-position impl
//! Trait in Trait) rather than `async_trait::async_trait`. Implementations of
//! these traits must use plain `async fn` — NOT `#[async_trait]` — to match
//! the expected signature.

use std::collections::BTreeMap;
use std::io::Cursor;
use std::ops::RangeBounds;
use std::sync::Arc;

use openraft::error::{InstallSnapshotError, RPCError, RaftError, Unreachable};
use openraft::network::{RPCOption, RaftNetwork, RaftNetworkFactory};
use openraft::raft::{
    AppendEntriesRequest, AppendEntriesResponse, InstallSnapshotRequest, InstallSnapshotResponse,
    VoteRequest, VoteResponse,
};
use openraft::storage::{Adaptor, RaftLogReader, RaftSnapshotBuilder, RaftStorage};
use openraft::{
    AnyError, BasicNode, Entry, LogId, LogState, RaftLogId, Snapshot, SnapshotMeta, StorageError,
    StoredMembership, TokioRuntime, Vote,
};
use serde::{Deserialize, Serialize};
use std::time::Duration;
use tokio::sync::watch;
use tonic::transport::{Channel, Endpoint};
use tracing::info;

use crate::cluster::{ClusterHandle, NodeState};
use crate::grpc::proto::{
    load_test_coordinator_client::LoadTestCoordinatorClient, AppendEntriesRequest as ProtoAER,
    SnapshotRequest as ProtoSR, VoteRequest as ProtoVR,
};

// ── Type configuration ─────────────────────────────────────────────────────────

/// Application log entry — a test config change or a no-op.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LoadTestRequest {
    SetConfig { yaml: String, version: String },
    Noop,
}

/// State machine response after applying a log entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadTestResponse {
    pub ok: bool,
    pub message: String,
}

// The `declare_raft_types!` macro generates the TypeConfig struct and all
// required trait impls (including `Responder` via `OneshotResponder`).
openraft::declare_raft_types!(
    pub TypeConfig:
        D            = LoadTestRequest,
        R            = LoadTestResponse,
        NodeId       = u64,
        Node         = BasicNode,
        Entry        = Entry<TypeConfig>,
        SnapshotData = Cursor<Vec<u8>>,
        AsyncRuntime = TokioRuntime,
);

pub type NodeId = u64;
pub type RaftInstance = openraft::Raft<TypeConfig>;

// ── Utility ───────────────────────────────────────────────────────────────────

/// Derive a stable u64 node ID from a human-readable string identifier.
pub fn node_id_from_str(s: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    s.hash(&mut h);
    h.finish()
}

// ── In-memory combined storage (openraft v1 RaftStorage) ─────────────────────

/// Combined in-memory Raft storage: log entries + state machine in one struct.
///
/// Wrapped by `openraft::storage::Adaptor::new(store)` to produce the v2
/// `(RaftLogStorage, RaftStateMachine)` split required by `Raft::new`.
pub struct MemStorage {
    vote: Option<Vote<NodeId>>,
    log: BTreeMap<u64, Entry<TypeConfig>>,
    committed: Option<LogId<NodeId>>,
    last_purged: Option<LogId<NodeId>>,
    last_applied: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, BasicNode>,
    pub current_config: Option<String>,
    snapshot: Option<Snapshot<TypeConfig>>,
    /// Notifies `RaftNode` (and `main.rs`) whenever a `SetConfig` entry is
    /// applied or a snapshot is installed.  The `Receiver` half is held by
    /// `RaftNode::config_rx` so reads always reflect the live state machine.
    config_tx: watch::Sender<Option<String>>,
}

impl MemStorage {
    /// Create a new empty storage, returning the storage and a `Receiver` that
    /// fires on every committed config change.
    pub fn new() -> (Self, watch::Receiver<Option<String>>) {
        let (config_tx, config_rx) = watch::channel(None);
        let storage = Self {
            vote: None,
            log: BTreeMap::new(),
            committed: None,
            last_purged: None,
            last_applied: None,
            last_membership: StoredMembership::default(),
            current_config: None,
            snapshot: None,
            config_tx,
        };
        (storage, config_rx)
    }
}

// RaftStorage v1 requires MemStorage to implement RaftLogReader directly
// (because RaftStorage: RaftLogReader). Use plain async fn (no #[async_trait])
// because openraft traits use RPITIT, not boxed futures.
impl RaftLogReader<TypeConfig> for MemStorage {
    async fn try_get_log_entries<RB>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>>
    where
        RB: RangeBounds<u64> + Clone + std::fmt::Debug + Send,
    {
        Ok(self.log.range(range).map(|(_, e)| e.clone()).collect())
    }
}

/// Log reader backed by a snapshot of the log at a point in time.
pub struct MemLogReader {
    log: BTreeMap<u64, Entry<TypeConfig>>,
}

impl RaftLogReader<TypeConfig> for MemLogReader {
    async fn try_get_log_entries<RB>(
        &mut self,
        range: RB,
    ) -> Result<Vec<Entry<TypeConfig>>, StorageError<NodeId>>
    where
        RB: RangeBounds<u64> + Clone + std::fmt::Debug + Send,
    {
        Ok(self.log.range(range).map(|(_, e)| e.clone()).collect())
    }
}

/// Snapshot builder — serialises current state to JSON.
pub struct MemSnapshotBuilder {
    last_applied: Option<LogId<NodeId>>,
    last_membership: StoredMembership<NodeId, BasicNode>,
    current_config: Option<String>,
}

impl RaftSnapshotBuilder<TypeConfig> for MemSnapshotBuilder {
    async fn build_snapshot(&mut self) -> Result<Snapshot<TypeConfig>, StorageError<NodeId>> {
        #[derive(Serialize)]
        struct SnapData<'a> {
            current_config: Option<&'a str>,
        }
        let data = serde_json::to_vec(&SnapData {
            current_config: self.current_config.as_deref(),
        })
        .unwrap_or_default();

        let snap_id = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        Ok(Snapshot {
            meta: SnapshotMeta {
                last_log_id: self.last_applied,
                last_membership: self.last_membership.clone(),
                snapshot_id: snap_id,
            },
            snapshot: Box::new(Cursor::new(data)),
        })
    }
}

/// openraft v1 `RaftStorage` implementation for `MemStorage`.
///
/// Wrapped by `Adaptor::new(store)` to produce v2 split traits.
/// Method names follow the v1 API:
///   - `append_to_log` (NOT `append`)
///   - `delete_conflict_logs_since` (NOT `truncate`)
///   - `purge_logs_upto` (NOT `purge`)
///   - `apply_to_state_machine` (NOT `apply`)
impl RaftStorage<TypeConfig> for MemStorage {
    type LogReader = MemLogReader;
    type SnapshotBuilder = MemSnapshotBuilder;

    // ── Vote ──────────────────────────────────────────────────────────────────

    async fn save_vote(&mut self, vote: &Vote<NodeId>) -> Result<(), StorageError<NodeId>> {
        self.vote = Some(*vote);
        Ok(())
    }

    async fn read_vote(&mut self) -> Result<Option<Vote<NodeId>>, StorageError<NodeId>> {
        Ok(self.vote)
    }

    // ── Log ───────────────────────────────────────────────────────────────────

    async fn get_log_state(&mut self) -> Result<LogState<TypeConfig>, StorageError<NodeId>> {
        let last = self.log.values().next_back().map(|e| *e.get_log_id());
        Ok(LogState {
            last_purged_log_id: self.last_purged,
            last_log_id: last,
        })
    }

    async fn save_committed(
        &mut self,
        committed: Option<LogId<NodeId>>,
    ) -> Result<(), StorageError<NodeId>> {
        self.committed = committed;
        Ok(())
    }

    async fn read_committed(&mut self) -> Result<Option<LogId<NodeId>>, StorageError<NodeId>> {
        Ok(self.committed)
    }

    async fn get_log_reader(&mut self) -> Self::LogReader {
        MemLogReader {
            log: self.log.clone(),
        }
    }

    /// Append log entries (v1 method name).
    async fn append_to_log<I>(&mut self, entries: I) -> Result<(), StorageError<NodeId>>
    where
        I: IntoIterator<Item = Entry<TypeConfig>> + Send,
    {
        for entry in entries {
            self.log.insert(entry.get_log_id().index, entry);
        }
        Ok(())
    }

    /// Delete conflict log entries since `log_id` inclusive (v1 method name).
    async fn delete_conflict_logs_since(
        &mut self,
        log_id: LogId<NodeId>,
    ) -> Result<(), StorageError<NodeId>> {
        self.log.retain(|&idx, _| idx < log_id.index);
        Ok(())
    }

    /// Delete applied log entries up to `log_id` inclusive (v1 method name).
    async fn purge_logs_upto(&mut self, log_id: LogId<NodeId>) -> Result<(), StorageError<NodeId>> {
        self.log.retain(|&idx, _| idx > log_id.index);
        self.last_purged = Some(log_id);
        Ok(())
    }

    // ── State machine ─────────────────────────────────────────────────────────

    async fn last_applied_state(
        &mut self,
    ) -> Result<(Option<LogId<NodeId>>, StoredMembership<NodeId, BasicNode>), StorageError<NodeId>>
    {
        Ok((self.last_applied, self.last_membership.clone()))
    }

    /// Apply committed entries to state machine (v1 method name, takes a slice).
    async fn apply_to_state_machine(
        &mut self,
        entries: &[Entry<TypeConfig>],
    ) -> Result<Vec<LoadTestResponse>, StorageError<NodeId>> {
        let mut responses = Vec::new();

        for entry in entries {
            self.last_applied = Some(*entry.get_log_id());

            match &entry.payload {
                openraft::EntryPayload::Blank => {
                    responses.push(LoadTestResponse {
                        ok: true,
                        message: "noop".to_string(),
                    });
                }
                openraft::EntryPayload::Normal(req) => match req {
                    LoadTestRequest::SetConfig { yaml, version } => {
                        self.current_config = Some(yaml.clone());
                        let _ = self.config_tx.send(Some(yaml.clone()));
                        info!(version = %version, "Applied test config from Raft log");
                        responses.push(LoadTestResponse {
                            ok: true,
                            message: format!("config applied (version: {})", version),
                        });
                    }
                    LoadTestRequest::Noop => {
                        responses.push(LoadTestResponse {
                            ok: true,
                            message: "noop".to_string(),
                        });
                    }
                },
                openraft::EntryPayload::Membership(m) => {
                    self.last_membership =
                        StoredMembership::new(Some(*entry.get_log_id()), m.clone());
                    responses.push(LoadTestResponse {
                        ok: true,
                        message: "membership change applied".to_string(),
                    });
                }
            }
        }

        Ok(responses)
    }

    async fn get_snapshot_builder(&mut self) -> Self::SnapshotBuilder {
        MemSnapshotBuilder {
            last_applied: self.last_applied,
            last_membership: self.last_membership.clone(),
            current_config: self.current_config.clone(),
        }
    }

    async fn begin_receiving_snapshot(
        &mut self,
    ) -> Result<Box<Cursor<Vec<u8>>>, StorageError<NodeId>> {
        Ok(Box::new(Cursor::new(Vec::new())))
    }

    async fn install_snapshot(
        &mut self,
        meta: &SnapshotMeta<NodeId, BasicNode>,
        snapshot: Box<Cursor<Vec<u8>>>,
    ) -> Result<(), StorageError<NodeId>> {
        #[derive(Deserialize)]
        struct SnapData {
            current_config: Option<String>,
        }
        let data: SnapData = serde_json::from_slice(snapshot.get_ref()).unwrap_or(SnapData {
            current_config: None,
        });

        self.last_applied = meta.last_log_id;
        self.last_membership = meta.last_membership.clone();
        self.current_config = data.current_config;
        let _ = self.config_tx.send(self.current_config.clone());
        Ok(())
    }

    async fn get_current_snapshot(
        &mut self,
    ) -> Result<Option<Snapshot<TypeConfig>>, StorageError<NodeId>> {
        Ok(self.snapshot.clone())
    }
}

// ── gRPC network transport ─────────────────────────────────────────────────────

/// Per-peer gRPC channel implementing openraft's `RaftNetwork`.
///
/// Serialises openraft request types as JSON and transports them via the proto
/// `payload: bytes` field of the `LoadTestCoordinator` service (Issue #46).
pub struct GrpcNetwork {
    target_addr: String,
    client: Option<LoadTestCoordinatorClient<Channel>>,
}

impl GrpcNetwork {
    fn get_client(&mut self) -> Result<&mut LoadTestCoordinatorClient<Channel>, String> {
        if self.client.is_none() {
            let uri = if self.target_addr.starts_with("http") {
                self.target_addr.clone()
            } else {
                format!("http://{}", self.target_addr)
            };
            // connect_lazy() returns immediately without a blocking TCP handshake.
            // Tonic dials on the first RPC and reconnects automatically on failure.
            // connect_timeout limits the TCP handshake; timeout limits each RPC call,
            // ensuring heartbeats fail fast rather than hanging until a follower's
            // election timer fires and causes an unnecessary leader re-election.
            let ch = Endpoint::from_shared(uri)
                .map_err(|e| e.to_string())?
                .connect_timeout(Duration::from_secs(3))
                .timeout(Duration::from_secs(4))
                .connect_lazy();
            self.client = Some(LoadTestCoordinatorClient::new(ch));
        }
        Ok(self.client.as_mut().unwrap())
    }
}

fn unreachable(msg: impl std::fmt::Display) -> Unreachable {
    Unreachable::new(&AnyError::error(msg.to_string()))
}

impl RaftNetwork<TypeConfig> for GrpcNetwork {
    // Return types use NodeId (not TypeConfig) for response generics per the trait signature.

    async fn append_entries(
        &mut self,
        rpc: AppendEntriesRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<AppendEntriesResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let payload =
            serde_json::to_vec(&rpc).map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let term = rpc.vote.leader_id().term;
        let leader = rpc.vote.leader_id().node_id.to_string();

        let client = self
            .get_client()
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let proto_resp = client
            .append_entries(ProtoAER {
                term,
                leader_id: leader,
                payload,
                ..Default::default()
            })
            .await
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        serde_json::from_slice(&proto_resp.into_inner().payload)
            .map_err(|e| RPCError::Unreachable(unreachable(e)))
    }

    async fn vote(
        &mut self,
        rpc: VoteRequest<NodeId>,
        _option: RPCOption,
    ) -> Result<VoteResponse<NodeId>, RPCError<NodeId, BasicNode, RaftError<NodeId>>> {
        let payload =
            serde_json::to_vec(&rpc).map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let term = rpc.vote.leader_id().term;
        let candidate = rpc.vote.leader_id().node_id.to_string();

        let client = self
            .get_client()
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let proto_resp = client
            .request_vote(ProtoVR {
                term,
                candidate_id: candidate,
                payload,
                ..Default::default()
            })
            .await
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        serde_json::from_slice(&proto_resp.into_inner().payload)
            .map_err(|e| RPCError::Unreachable(unreachable(e)))
    }

    async fn install_snapshot(
        &mut self,
        rpc: InstallSnapshotRequest<TypeConfig>,
        _option: RPCOption,
    ) -> Result<
        InstallSnapshotResponse<NodeId>,
        RPCError<NodeId, BasicNode, RaftError<NodeId, InstallSnapshotError>>,
    > {
        let payload =
            serde_json::to_vec(&rpc).map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let term = rpc.vote.leader_id().term;

        let client = self
            .get_client()
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        let proto_resp = client
            .install_snapshot(ProtoSR {
                term,
                payload,
                ..Default::default()
            })
            .await
            .map_err(|e| RPCError::Unreachable(unreachable(e)))?;

        serde_json::from_slice(&proto_resp.into_inner().payload)
            .map_err(|e| RPCError::Unreachable(unreachable(e)))
    }
}

/// Creates `GrpcNetwork` instances per target peer node.
pub struct GrpcNetworkFactory;

impl RaftNetworkFactory<TypeConfig> for GrpcNetworkFactory {
    type Network = GrpcNetwork;

    async fn new_client(&mut self, _target: NodeId, node: &BasicNode) -> Self::Network {
        GrpcNetwork {
            target_addr: node.addr.clone(),
            client: None,
        }
    }
}

// ── RaftNode public API ────────────────────────────────────────────────────────

/// A running Raft node.
///
/// Wraps `openraft::Raft<TypeConfig>` plus a `watch::Receiver` that is
/// driven by the real openraft state machine; callers read `current_config`
/// (or subscribe via `config_receiver`) to observe committed configs.
#[derive(Clone)]
pub struct RaftNode {
    pub raft: Arc<RaftInstance>,
    /// Watch receiver driven by the real openraft state machine.
    /// Always reflects the last committed `SetConfig` entry.
    config_rx: watch::Receiver<Option<String>>,
    pub node_id: NodeId,
}

impl RaftNode {
    /// Returns true if this node is the current Raft leader.
    pub fn is_leader(&self) -> bool {
        self.raft.metrics().borrow().current_leader == Some(self.node_id)
    }

    /// Write a test configuration to the Raft log (leader only).
    pub async fn set_config(
        &self,
        yaml: String,
        version: String,
    ) -> Result<
        (),
        openraft::error::RaftError<NodeId, openraft::error::ClientWriteError<NodeId, BasicNode>>,
    > {
        self.raft
            .client_write(LoadTestRequest::SetConfig { yaml, version })
            .await
            .map(|_| ())
    }

    /// Returns a cloned `Receiver` that fires whenever a new config is
    /// committed.  Use this in `main.rs` to drive worker reconfiguration.
    pub fn config_receiver(&self) -> watch::Receiver<Option<String>> {
        self.config_rx.clone()
    }

    /// Returns the currently committed test config YAML, if any.
    pub async fn current_config(&self) -> Option<String> {
        self.config_rx.borrow().clone()
    }
}

// ── Startup ───────────────────────────────────────────────────────────────────

/// Initialises and starts a Raft node, returning a shared `RaftNode` handle.
///
/// `peers` is `(node_id, grpc_addr)` for **every** node including this one,
/// parsed from `CLUSTER_NODES` in static discovery mode.
///
/// Spawns a background task watching `Raft::metrics()` to keep `ClusterHandle`
/// in sync with Raft state (`Forming → Follower → Leader`).
pub async fn start_raft_node(handle: ClusterHandle, peers: Vec<(NodeId, String)>) -> Arc<RaftNode> {
    // Derive this node's ID from CLUSTER_SELF_ADDR when set (Issue #80).
    // CLUSTER_SELF_ADDR must be the address string that appears in the peer list
    // (either CLUSTER_NODES for static, or the Consul-resolved address for Consul mode)
    // so that `this_node_id` matches one of the peer IDs and Raft can initialize.
    // Falls back to CLUSTER_NODE_ID / HOSTNAME, but that causes a mismatch when
    // peers are identified by IP:port strings.
    let this_node_id = handle
        .config()
        .self_addr
        .as_deref()
        .map(node_id_from_str)
        .unwrap_or_else(|| node_id_from_str(&handle.config().node_id));

    let config = Arc::new(
        openraft::Config {
            cluster_name: handle.config().consul_service_name.clone(),
            // Generous timeouts so Raft survives CPU/memory pressure from the
            // load test workers sharing the same Tokio runtime.
            heartbeat_interval: 500,
            election_timeout_min: 5_000,
            election_timeout_max: 10_000,
            ..Default::default()
        }
        .validate()
        .expect("valid openraft config"),
    );

    // Create the single MemStorage instance.  The watch Receiver is the only
    // channel between openraft's state machine and RaftNode::current_config().
    let (storage, config_rx) = MemStorage::new();
    let (log_store, state_machine) = Adaptor::new(storage);

    let raft = Arc::new(
        openraft::Raft::new(
            this_node_id,
            config,
            GrpcNetworkFactory,
            log_store,
            state_machine,
        )
        .await
        .expect("failed to create Raft instance"),
    );

    // Initialise cluster with the full peer set.
    if !peers.is_empty() {
        let members: BTreeMap<NodeId, BasicNode> = peers
            .iter()
            .map(|(id, addr)| (*id, BasicNode { addr: addr.clone() }))
            .collect();

        let min_id = peers
            .iter()
            .map(|(id, _)| *id)
            .min()
            .unwrap_or(this_node_id);
        if this_node_id == min_id {
            if let Err(e) = raft.initialize(members).await {
                info!(error = %e, "Raft already initialised (ignoring on restart)");
            }
        }
    }

    let node = Arc::new(RaftNode {
        raft: raft.clone(),
        config_rx,
        node_id: this_node_id,
    });

    // Watch Raft state → update ClusterHandle
    {
        let mut rx = raft.metrics();
        let h = handle.clone();
        tokio::spawn(async move {
            loop {
                if rx.changed().await.is_err() {
                    break;
                }
                let m = rx.borrow().clone();
                let new_state = match m.state {
                    openraft::ServerState::Leader => NodeState::Leader,
                    openraft::ServerState::Follower | openraft::ServerState::Candidate => {
                        NodeState::Follower
                    }
                    _ => NodeState::Forming,
                };
                if h.state() != new_state {
                    h.set_state(new_state.clone());
                    info!(
                        node_id = this_node_id,
                        state   = new_state.as_str(),
                        term    = m.current_term,
                        leader  = ?m.current_leader,
                        "Raft state changed"
                    );
                }
            }
        });
    }

    info!(
        node_id = this_node_id,
        peers = peers.len(),
        "Raft node started"
    );

    node
}

// ── Tests ──────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn node_id_is_deterministic() {
        assert_eq!(
            node_id_from_str("node-us-central1"),
            node_id_from_str("node-us-central1")
        );
    }

    #[test]
    fn different_node_ids_differ() {
        assert_ne!(
            node_id_from_str("node-us-central1"),
            node_id_from_str("node-europe-west1")
        );
    }

    #[test]
    fn request_serialises_roundtrip() {
        let req = LoadTestRequest::SetConfig {
            yaml: "workers: 10".into(),
            version: "v1".into(),
        };
        let j = serde_json::to_string(&req).unwrap();
        assert!(matches!(
            serde_json::from_str::<LoadTestRequest>(&j).unwrap(),
            LoadTestRequest::SetConfig { .. }
        ));
    }

    #[tokio::test]
    async fn mem_storage_vote_roundtrip() {
        let (mut s, _rx) = MemStorage::new();
        let vote = Vote::new(1, 42);
        s.save_vote(&vote).await.unwrap();
        assert_eq!(s.read_vote().await.unwrap(), Some(vote));
    }

    #[tokio::test]
    async fn mem_storage_initial_log_state() {
        let (mut s, _rx) = MemStorage::new();
        let state = s.get_log_state().await.unwrap();
        assert!(state.last_log_id.is_none());
        assert!(state.last_purged_log_id.is_none());
    }

    #[tokio::test]
    async fn mem_storage_apply_set_config() {
        let (mut s, mut rx) = MemStorage::new();
        assert!(s.current_config.is_none());

        // Build a synthetic Entry with a Normal payload.
        use openraft::{CommittedLeaderId, Entry, EntryPayload, LogId};
        let log_id = LogId::new(CommittedLeaderId::new(1, 1), 1);
        let entry = Entry::<TypeConfig> {
            log_id,
            payload: EntryPayload::Normal(LoadTestRequest::SetConfig {
                yaml: "workers: 5".into(),
                version: "abc".into(),
            }),
        };
        let resps = s.apply_to_state_machine(&[entry]).await.unwrap();
        assert!(resps[0].ok);
        assert_eq!(s.current_config.as_deref(), Some("workers: 5"));
        // The watch channel must have been notified with the new config.
        assert!(rx.has_changed().unwrap());
        assert_eq!(rx.borrow_and_update().as_deref(), Some("workers: 5"));
    }

    #[tokio::test]
    async fn snapshot_roundtrip() {
        let (mut s, _rx) = MemStorage::new();
        s.current_config = Some("workers: 10\n".into());
        let mut builder = s.get_snapshot_builder().await;
        let snap = builder.build_snapshot().await.unwrap();

        let (mut s2, mut rx2) = MemStorage::new();
        s2.install_snapshot(&snap.meta, snap.snapshot)
            .await
            .unwrap();
        assert_eq!(s2.current_config.as_deref(), Some("workers: 10\n"));
        // install_snapshot must also notify the watch channel.
        assert!(rx2.has_changed().unwrap());
        assert_eq!(rx2.borrow_and_update().as_deref(), Some("workers: 10\n"));
    }
}
