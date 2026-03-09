//! Node auto-registration with the web app registry (Issue #89).
//!
//! When `NODE_REGISTRY_URL`, `AUTO_REGISTER_PSK`, and `NODE_BASE_URL` are all
//! set, the node POSTs its identity to the web app at startup and re-registers
//! at a configurable interval (heartbeat) to keep its record alive.
//!
//! If any of the three required env vars is missing, registration is silently
//! skipped — the node operates exactly as before (fully backwards-compatible).

use reqwest::Client;
use std::time::Duration;
use tracing::{error, info, warn};

use crate::utils::parse_duration_string;

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
    /// How often to re-register (heartbeat). Default: 30 s.
    pub interval: Duration,
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

        let node_name = std::env::var("NODE_NAME").unwrap_or_else(|_| node_id.to_string());

        let tags = std::env::var("NODE_TAGS")
            .ok()
            .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
            .unwrap_or_else(|| serde_json::json!({}));

        let interval = std::env::var("NODE_REGISTRY_INTERVAL")
            .ok()
            .and_then(|s| parse_duration_string(&s).ok())
            .unwrap_or(Duration::from_secs(30));

        Some(Self {
            registry_url,
            psk,
            node_base_url,
            node_name,
            region: region.to_string(),
            tags,
            interval,
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

/// Spawn a background task that registers immediately then re-registers at
/// `cfg.interval`.  All failures are logged; the task never crashes the node.
pub fn spawn_registration_task(client: Client, cfg: RegistrationConfig) {
    tokio::spawn(async move {
        // Register immediately on startup.
        register_once(&client, &cfg).await;

        // Heartbeat loop — keeps the node alive in the registry.
        let mut ticker = tokio::time::interval(cfg.interval);
        ticker.tick().await; // first tick fires immediately; skip it
        loop {
            ticker.tick().await;
            register_once(&client, &cfg).await;
        }
    });
}
