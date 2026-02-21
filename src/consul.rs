//! Consul service registration and health-tag management (Issue #47).
//!
//! When `DISCOVERY_MODE=consul`, each node:
//! 1. Registers itself with the local Consul agent on startup.
//! 2. Updates its service tags on every Raft state change
//!    (`forming` → `follower` → `leader`).
//! 3. Deregisters on clean shutdown.
//!
//! Consul then provides DNS-based peer discovery:
//!
//! | DNS name                                    | Resolves to            |
//! |---------------------------------------------|------------------------|
//! | `loadtest-cluster.service.consul`           | All healthy nodes      |
//! | `leader.loadtest-cluster.service.consul`    | Current leader         |
//! | `follower.loadtest-cluster.service.consul`  | All followers          |
//! | `forming.loadtest-cluster.service.consul`   | Nodes forming quorum   |
//!
//! The Consul agent is assumed to be running locally at `CONSUL_ADDR`
//! (default `http://127.0.0.1:8500`). The same agent is used for both
//! discovery (DNS) and KV config retrieval (Issue #76).

use reqwest::Client;
use serde_json::json;
use tracing::{error, info, warn};

/// Minimal Consul HTTP API client for service registration.
///
/// Uses the already-present `reqwest` dependency — no additional crates needed.
pub struct ConsulClient {
    base_url: String,
    http: Client,
    service_name: String,
    node_id: String,
    grpc_port: u16,
    health_addr: String,
}

impl ConsulClient {
    /// Create a new Consul client.
    ///
    /// - `consul_addr`: e.g. `http://127.0.0.1:8500`
    /// - `service_name`: e.g. `loadtest-cluster`
    /// - `node_id`: stable node identity (e.g. hostname)
    /// - `grpc_port`: the gRPC port for cluster traffic (e.g. 7000)
    /// - `health_addr`: e.g. `0.0.0.0:8080` — resolved to `localhost:8080` for check URL
    pub fn new(
        consul_addr: &str,
        service_name: &str,
        node_id: &str,
        grpc_port: u16,
        health_addr: &str,
    ) -> Self {
        Self {
            base_url: consul_addr.trim_end_matches('/').to_string(),
            http: Client::new(),
            service_name: service_name.to_string(),
            node_id: node_id.to_string(),
            grpc_port,
            health_addr: health_addr.to_string(),
        }
    }

    fn service_id(&self) -> String {
        format!("{}-{}", self.service_name, self.node_id)
    }

    fn health_check_url(&self) -> String {
        // Convert 0.0.0.0:8080 → http://localhost:8080/health/cluster
        let port = self
            .health_addr
            .split(':')
            .last()
            .unwrap_or("8080")
            .parse::<u16>()
            .unwrap_or(8080);
        format!("http://localhost:{}/health/cluster", port)
    }

    /// Register (or re-register) this node with Consul using the given `state` tag.
    ///
    /// Called on startup (with `"forming"`) and on every Raft state change.
    /// Consul uses the tag to populate tagged DNS queries and drive config
    /// retrieval (leader tag triggers GCS/Consul KV fetch in Issue #76).
    pub async fn register(&self, state_tag: &str) {
        let url = format!("{}/v1/agent/service/register", self.base_url);
        let body = json!({
            "ID":   self.service_id(),
            "Name": self.service_name,
            "Tags": [state_tag],
            "Port": self.grpc_port,
            "Meta": {
                "node_id": self.node_id,
                "state":   state_tag,
            },
            "Checks": [{
                "HTTP":     self.health_check_url(),
                "Interval": "5s",
                "Timeout":  "2s",
                "DeregisterCriticalServiceAfter": "30s",
            }],
        });

        match self.http.put(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!(
                    service  = %self.service_id(),
                    state    = %state_tag,
                    "Registered with Consul"
                );
            }
            Ok(resp) => {
                warn!(
                    service = %self.service_id(),
                    status  = %resp.status(),
                    "Consul registration returned non-2xx"
                );
            }
            Err(e) => {
                error!(error = %e, "Failed to register with Consul");
            }
        }
    }

    /// Update the service's Consul tags to reflect the new Raft state.
    ///
    /// Re-registers the service (Consul upserts on repeated registration).
    pub async fn update_tags(&self, new_state_tag: &str) {
        info!(
            service = %self.service_id(),
            state   = %new_state_tag,
            "Updating Consul service tags"
        );
        self.register(new_state_tag).await;
    }

    /// Deregister this node from Consul on clean shutdown.
    pub async fn deregister(&self) {
        let url = format!(
            "{}/v1/agent/service/deregister/{}",
            self.base_url,
            self.service_id()
        );
        match self.http.put(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!(service = %self.service_id(), "Deregistered from Consul");
            }
            Ok(resp) => {
                warn!(
                    service = %self.service_id(),
                    status  = %resp.status(),
                    "Consul deregister returned non-2xx"
                );
            }
            Err(e) => {
                error!(error = %e, "Failed to deregister from Consul");
            }
        }
    }
}

/// Build a `ConsulClient` from a `ClusterConfig` and spawn a background task
/// that updates Consul tags whenever the `ClusterHandle` state changes.
///
/// Returns `None` if discovery mode is not Consul.
pub fn start_consul_tagging(handle: &crate::cluster::ClusterHandle) -> Option<ConsulClient> {
    use crate::cluster::DiscoveryMode;

    let cfg = handle.config();
    if cfg.discovery_mode != DiscoveryMode::Consul {
        return None;
    }

    let port: u16 = cfg
        .bind_addr
        .split(':')
        .last()
        .unwrap_or("7000")
        .parse()
        .unwrap_or(7000);

    let client = ConsulClient::new(
        &cfg.consul_addr,
        &cfg.consul_service_name,
        &cfg.node_id,
        port,
        &cfg.health_addr,
    );

    // Spawn a task that watches ClusterHandle state and calls update_tags.
    // We poll every second; for production a watch::Receiver<NodeState> on
    // ClusterHandle would be cleaner, but this keeps the code simple.
    {
        use crate::cluster::NodeState;
        let client2 = ConsulClient::new(
            &cfg.consul_addr,
            &cfg.consul_service_name,
            &cfg.node_id,
            port,
            &cfg.health_addr,
        );
        let handle_clone = handle.clone();
        tokio::spawn(async move {
            let mut last_state = NodeState::Forming;
            // Initial registration
            client2.register("forming").await;
            loop {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;
                let current = handle_clone.state();
                if current != last_state {
                    client2.update_tags(current.as_str()).await;
                    last_state = current;
                }
            }
        });
    }

    Some(client)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_id_format() {
        let c = ConsulClient::new(
            "http://127.0.0.1:8500",
            "loadtest-cluster",
            "node-1",
            7000,
            "0.0.0.0:8080",
        );
        assert_eq!(c.service_id(), "loadtest-cluster-node-1");
    }

    #[test]
    fn health_check_url_from_bind_addr() {
        let c = ConsulClient::new(
            "http://127.0.0.1:8500",
            "loadtest-cluster",
            "node-1",
            7000,
            "0.0.0.0:8090",
        );
        assert_eq!(c.health_check_url(), "http://localhost:8090/health/cluster");
    }
}
