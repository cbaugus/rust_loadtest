//! Node auto-registration with the web app registry (Issue #89).
//!
//! When `NODE_REGISTRY_URL`, `AUTO_REGISTER_PSK`, and `NODE_BASE_URL` are all
//! set, the node POSTs its identity to the web app **once at startup**.
//!
//! The control plane is expected to poll each node's `GET /health` endpoint
//! on its own schedule to track liveness and runtime metrics (webload-gui#82).
//! Periodic re-registration from the node side is no longer needed and has
//! been removed (Issue #104).
//!
//! If any of the three required env vars is missing, registration is silently
//! skipped — the node operates exactly as before (fully backwards-compatible).
//!
//! `NODE_REGISTRY_INTERVAL` is **deprecated** — if set it is ignored and a
//! warning is logged.

use reqwest::Client;
use tracing::{info, warn, error};

/// Configuration for auto-registration, built from environment variables.
pub struct RegistrationConfig {
    /// Base URL of the web app, e.g. `https://loadtest-control.example.com`
    pub registry_url: String,
    /// Pre-shared key sent as `X-Auto-Register-PSK` header.
    pub psk: String,
    /// This node's reachable URL, e.g. `http://10.0.1.5:8080`
    pub node_base_url: String,
    /// Human-readable node name shown in the registry UI.
    pub node_name: String,
    /// Region label forwarded from `ClusterConfig`.
    pub region: String,
    /// Arbitrary JSON tags, e.g. `{"env":"staging","rack":"A"}`.
    pub tags: serde_json::Value,
}

impl RegistrationConfig {
    /// Build from environment variables.  Returns `None` if any required var
    /// (`NODE_REGISTRY_URL`, `AUTO_REGISTER_PSK`, `NODE_BASE_URL`) is missing.
    pub fn from_env(node_id: &str, region: &str) -> Option<Self> {
        let registry_url = match std::env::var("NODE_REGISTRY_URL") {
            Ok(v) => v,
            Err(_) => return None,
        };
        let psk = match std::env::var("AUTO_REGISTER_PSK") {
            Ok(v) => v,
            Err(_) => {
                warn!("NODE_REGISTRY_URL is set but AUTO_REGISTER_PSK is missing — skipping registration");
                return None;
            }
        };
        let node_base_url = match std::env::var("NODE_BASE_URL") {
            Ok(v) => v,
            Err(_) => {
                warn!(
                    "NODE_REGISTRY_URL is set but NODE_BASE_URL is missing — skipping registration"
                );
                return None;
            }
        };

        // NODE_REGISTRY_INTERVAL is deprecated — the control plane now polls
        // GET /health instead of relying on node-side heartbeats (Issue #104).
        if std::env::var("NODE_REGISTRY_INTERVAL").is_ok() {
            warn!(
                "NODE_REGISTRY_INTERVAL is deprecated and ignored — \
                 the control plane polls GET /health for liveness (webload-gui#82). \
                 Remove this variable from your configuration."
            );
        }

        let node_name = std::env::var("NODE_NAME").unwrap_or_else(|_| node_id.to_string());

        let tags = std::env::var("NODE_TAGS")
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        Some(Self {
            registry_url,
            psk,
            node_base_url,
            node_name,
            region: region.to_string(),
            tags,
        })
    }
}

/// Send a single registration POST.  Returns `true` on success.
/// Errors are logged but never propagated — the node must keep running.
pub async fn register_once(client: &Client, cfg: &RegistrationConfig) -> bool {
    let url = format!("{}/api/v1/nodes/register", cfg.registry_url);
    let body = serde_json::json!({
        "name":     cfg.node_name,
        "base_url": cfg.node_base_url,
        "region":   cfg.region,
        "tags":     cfg.tags,
    });

    match client
        .post(&url)
        .header("X-Auto-Register-PSK", &cfg.psk)
        .header("Content-Type", "application/json")
        .json(&body)
        .send()
        .await
    {
        Ok(resp) if resp.status().is_success() => {
            info!(
                url = %url,
                node = %cfg.node_name,
                base_url = %cfg.node_base_url,
                "Node registered with web app"
            );
            true
        }
        Ok(resp) => {
            warn!(
                url = %url,
                status = %resp.status(),
                node = %cfg.node_name,
                "Node registration rejected by web app"
            );
            false
        }
        Err(e) => {
            error!(
                url = %url,
                error = %e,
                node = %cfg.node_name,
                "Node registration request failed"
            );
            false
        }
    }
}

/// Register the node with the web app once at startup.
/// The control plane polls `GET /health` for ongoing liveness (webload-gui#82).
pub fn spawn_registration_task(client: Client, cfg: RegistrationConfig) {
    tokio::spawn(async move {
        register_once(&client, &cfg).await;
    });
}
