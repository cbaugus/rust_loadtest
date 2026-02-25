//! External config source retrieval for cluster leader auto-fetch (Issue #76).
//!
//! When `CLUSTER_CONFIG_SOURCE` is set and this node is elected Raft leader,
//! the leader fetches the test configuration from GCS or Consul KV and commits
//! it to the Raft log, triggering worker-pool reconfiguration on all nodes.
//!
//! ## Supported sources
//!
//! | `CLUSTER_CONFIG_SOURCE` | Env vars required |
//! |-------------------------|-------------------|
//! | `gcs`                   | `GCS_CONFIG_BUCKET`, `GCS_CONFIG_OBJECT` |
//! | `consul-kv`             | `CONSUL_ADDR` (already used by discovery) |
//!
//! Set `CONSUL_CONFIG_KEY` to override the default Consul KV path
//! (`loadtest/config`).
//!
//! Set `CLUSTER_CONFIG_TIMEOUT_SECS` to override the per-fetch timeout
//! (default 30 s).  This env var is read in `main.rs` at call time.

use base64::Engine as _;
use reqwest::Client;
use tracing::{debug, instrument};

// ── ConfigSource ──────────────────────────────────────────────────────────────

/// Where to fetch the test configuration YAML from when a leader is elected.
#[derive(Debug, Clone)]
pub enum ConfigSource {
    /// Google Cloud Storage — uses the GCE metadata service for ADC tokens.
    Gcs { bucket: String, object: String },
    /// HashiCorp Consul KV — plain HTTP, no auth required in dev setups.
    ConsulKv { consul_addr: String, key: String },
}

impl ConfigSource {
    /// Construct a `ConfigSource` from environment variables.
    ///
    /// Returns `None` when `CLUSTER_CONFIG_SOURCE` is unset or empty, meaning
    /// auto-fetch is disabled and clusters rely on `POST /cluster/config`.
    pub fn from_env() -> Option<Self> {
        let source = std::env::var("CLUSTER_CONFIG_SOURCE").ok()?;
        match source.to_lowercase().as_str() {
            "gcs" => {
                let bucket = std::env::var("GCS_CONFIG_BUCKET")
                    .expect("GCS_CONFIG_BUCKET must be set when CLUSTER_CONFIG_SOURCE=gcs");
                let object = std::env::var("GCS_CONFIG_OBJECT")
                    .expect("GCS_CONFIG_OBJECT must be set when CLUSTER_CONFIG_SOURCE=gcs");
                Some(ConfigSource::Gcs { bucket, object })
            }
            "consul-kv" => {
                let consul_addr = std::env::var("CONSUL_ADDR")
                    .unwrap_or_else(|_| "http://127.0.0.1:8500".to_string());
                let key = std::env::var("CONSUL_CONFIG_KEY")
                    .unwrap_or_else(|_| "loadtest/config".to_string());
                Some(ConfigSource::ConsulKv { consul_addr, key })
            }
            other => {
                tracing::warn!(
                    source = other,
                    "Unknown CLUSTER_CONFIG_SOURCE value; auto-fetch disabled"
                );
                None
            }
        }
    }

    /// Fetch the configuration YAML string from the external store.
    ///
    /// Returns the raw YAML text on success. The caller is responsible for
    /// committing it to the Raft log via `RaftNode::set_config`.
    #[instrument(skip(client), fields(source = self.source_name()))]
    pub async fn fetch(&self, client: &Client) -> Result<String, FetchError> {
        match self {
            ConfigSource::Gcs { bucket, object } => fetch_gcs(client, bucket, object).await,
            ConfigSource::ConsulKv { consul_addr, key } => {
                fetch_consul_kv(client, consul_addr, key).await
            }
        }
    }

    fn source_name(&self) -> &'static str {
        match self {
            ConfigSource::Gcs { .. } => "gcs",
            ConfigSource::ConsulKv { .. } => "consul-kv",
        }
    }
}

// ── Error type ────────────────────────────────────────────────────────────────

/// Errors that can occur while fetching the external configuration.
#[derive(Debug, thiserror::Error)]
pub enum FetchError {
    #[error("HTTP request failed: {0}")]
    Http(#[from] reqwest::Error),

    #[error("Metadata token response missing 'access_token' field")]
    MissingToken,

    #[error("Consul KV response array was empty")]
    EmptyKvResponse,

    #[error("Consul KV 'Value' field missing or null")]
    MissingKvValue,

    #[error("base64 decode failed: {0}")]
    Base64(#[from] base64::DecodeError),

    #[error("UTF-8 decode failed: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

// ── GCS fetch ─────────────────────────────────────────────────────────────────

/// Fetch a GCS object using Application Default Credentials from the GCE
/// metadata server.
///
/// 1. GET metadata token endpoint → `access_token`
/// 2. GET GCS storage API with `Authorization: Bearer <token>` → object body
async fn fetch_gcs(client: &Client, bucket: &str, object: &str) -> Result<String, FetchError> {
    // Step 1 — obtain an ADC token from the instance metadata service.
    let token_url =
        "http://metadata.google.internal/computeMetadata/v1/instance/service-accounts/default/token";

    debug!(url = token_url, "Fetching GCE metadata token");
    let token_resp: serde_json::Value = client
        .get(token_url)
        .header("Metadata-Flavor", "Google")
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let access_token = token_resp
        .get("access_token")
        .and_then(|v| v.as_str())
        .ok_or(FetchError::MissingToken)?;

    // Step 2 — fetch the object from the GCS JSON API.
    let encoded_object = percent_encode(object);
    let object_url = format!(
        "https://storage.googleapis.com/storage/v1/b/{}/o/{}?alt=media",
        bucket, encoded_object
    );

    debug!(url = %object_url, "Fetching GCS object");
    let yaml = client
        .get(&object_url)
        .header("Authorization", format!("Bearer {}", access_token))
        .send()
        .await?
        .error_for_status()?
        .text()
        .await?;

    Ok(yaml)
}

// ── Consul KV fetch ───────────────────────────────────────────────────────────

/// Response shape returned by `GET /v1/kv/<key>`.
#[derive(serde::Deserialize)]
struct ConsulKvEntry {
    #[serde(rename = "Value")]
    value: Option<String>,
}

/// Fetch a key from Consul KV.
///
/// 1. GET `{consul_addr}/v1/kv/{key}` → JSON array `[{Value: "<base64>"}]`
/// 2. base64-decode `Value` → UTF-8 YAML string
async fn fetch_consul_kv(
    client: &Client,
    consul_addr: &str,
    key: &str,
) -> Result<String, FetchError> {
    let url = format!("{}/v1/kv/{}", consul_addr.trim_end_matches('/'), key);
    debug!(url = %url, "Fetching Consul KV entry");

    let entries: Vec<ConsulKvEntry> = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let entry = entries
        .into_iter()
        .next()
        .ok_or(FetchError::EmptyKvResponse)?;
    let b64 = entry.value.ok_or(FetchError::MissingKvValue)?;

    let bytes = base64::engine::general_purpose::STANDARD.decode(b64.trim())?;
    let yaml = String::from_utf8(bytes)?;

    Ok(yaml)
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Percent-encode a GCS object path for use in a URL.
///
/// Only encodes characters that are not safe in a URL path segment.
/// Slashes are encoded as `%2F` so the object path is treated as a single
/// path component by the GCS API.
pub fn percent_encode(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    for byte in input.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char);
            }
            b => {
                out.push('%');
                out.push(
                    char::from_digit((b >> 4) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
                out.push(
                    char::from_digit((b & 0xf) as u32, 16)
                        .unwrap()
                        .to_ascii_uppercase(),
                );
            }
        }
    }
    out
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialise all tests that mutate environment variables.
    // Rust runs tests in parallel by default; concurrent set_var / remove_var
    // calls on the same keys cause data races between from_env_* tests.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    // ── percent_encode ────────────────────────────────────────────────────────

    #[test]
    fn percent_encode_plain() {
        assert_eq!(percent_encode("configs/prod.yaml"), "configs%2Fprod.yaml");
    }

    #[test]
    fn percent_encode_unreserved_unchanged() {
        assert_eq!(percent_encode("abc-DEF_1.2~"), "abc-DEF_1.2~");
    }

    #[test]
    fn percent_encode_spaces_and_special() {
        assert_eq!(
            percent_encode("my config/v1 test.yaml"),
            "my%20config%2Fv1%20test.yaml"
        );
    }

    // ── from_env ──────────────────────────────────────────────────────────────

    #[test]
    fn from_env_unset_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::remove_var("CLUSTER_CONFIG_SOURCE");
        assert!(ConfigSource::from_env().is_none());
    }

    #[test]
    fn from_env_consul_kv_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("CLUSTER_CONFIG_SOURCE", "consul-kv");
        std::env::remove_var("CONSUL_ADDR");
        std::env::remove_var("CONSUL_CONFIG_KEY");

        let src = ConfigSource::from_env().expect("should be Some");
        match src {
            ConfigSource::ConsulKv { consul_addr, key } => {
                assert_eq!(consul_addr, "http://127.0.0.1:8500");
                assert_eq!(key, "loadtest/config");
            }
            _ => panic!("expected ConsulKv variant"),
        }

        std::env::remove_var("CLUSTER_CONFIG_SOURCE");
    }

    #[test]
    fn from_env_consul_kv_custom_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("CLUSTER_CONFIG_SOURCE", "consul-kv");
        std::env::set_var("CONSUL_CONFIG_KEY", "my/custom/key");
        std::env::remove_var("CONSUL_ADDR");

        let src = ConfigSource::from_env().expect("should be Some");
        match src {
            ConfigSource::ConsulKv { key, .. } => assert_eq!(key, "my/custom/key"),
            _ => panic!("expected ConsulKv variant"),
        }

        std::env::remove_var("CLUSTER_CONFIG_SOURCE");
        std::env::remove_var("CONSUL_CONFIG_KEY");
    }

    #[test]
    fn from_env_unknown_source_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        std::env::set_var("CLUSTER_CONFIG_SOURCE", "s3");
        assert!(ConfigSource::from_env().is_none());
        std::env::remove_var("CLUSTER_CONFIG_SOURCE");
    }

    // ── Consul KV base64 decode path ──────────────────────────────────────────

    #[test]
    fn consul_kv_value_decodes_correctly() {
        // Simulate what Consul returns for a stored YAML string.
        let original = "num_concurrent_tasks: 10\ntarget_url: http://example.com\n";
        let encoded = base64::engine::general_purpose::STANDARD.encode(original);

        let bytes = base64::engine::general_purpose::STANDARD
            .decode(encoded.trim())
            .unwrap();
        let decoded = String::from_utf8(bytes).unwrap();
        assert_eq!(decoded, original);
    }
}
