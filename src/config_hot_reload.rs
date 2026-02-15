//! Configuration hot-reload functionality (Issue #44).
//!
//! This module provides file watching and hot-reload capabilities for YAML
//! configuration files. Changes are detected, validated, and applied without
//! stopping the running test.
//!
//! # Example
//! ```no_run
//! use rust_loadtest::config_hot_reload::{ConfigWatcher, ReloadNotifier};
//! use std::sync::Arc;
//! use std::time::Duration;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let notifier = Arc::new(ReloadNotifier::new());
//! let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;
//!
//! // Start watching in background
//! watcher.start()?;
//!
//! // Check for reload events
//! if let Some(event) = notifier.try_recv() {
//!     println!("Config reloaded: {:?}", event);
//! }
//!
//! // Stop watching
//! watcher.stop()?;
//! # Ok(())
//! # }
//! ```

use crate::yaml_config::{YamlConfig, YamlConfigError};
use notify::{Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};
use tracing::{debug, error, info, warn};

/// Hot-reload configuration.
#[derive(Debug, Clone)]
pub struct HotReloadConfig {
    /// Enable hot-reload functionality.
    pub enabled: bool,

    /// Path to the config file to watch.
    pub file_path: PathBuf,

    /// Debounce duration to avoid multiple reloads for rapid file changes.
    pub debounce_ms: u64,
}

impl HotReloadConfig {
    /// Create a new hot-reload config.
    pub fn new(file_path: impl Into<PathBuf>) -> Self {
        Self {
            enabled: true,
            file_path: file_path.into(),
            debounce_ms: 500, // Wait 500ms after last change
        }
    }

    /// Create a disabled hot-reload config.
    pub fn disabled() -> Self {
        Self {
            enabled: false,
            file_path: PathBuf::new(),
            debounce_ms: 0,
        }
    }

    /// Enable hot-reload.
    pub fn enable(mut self) -> Self {
        self.enabled = true;
        self
    }

    /// Disable hot-reload.
    pub fn disable(mut self) -> Self {
        self.enabled = false;
        self
    }

    /// Set debounce duration in milliseconds.
    pub fn with_debounce_ms(mut self, ms: u64) -> Self {
        self.debounce_ms = ms;
        self
    }
}

/// Reload event containing the new configuration.
#[derive(Debug, Clone)]
pub struct ReloadEvent {
    /// Timestamp of the reload.
    pub timestamp: SystemTime,

    /// Path to the config file.
    pub file_path: PathBuf,

    /// The reloaded configuration.
    pub config: YamlConfig,

    /// Whether validation succeeded.
    pub valid: bool,

    /// Validation error message (if any).
    pub error: Option<String>,
}

impl ReloadEvent {
    /// Check if the reload was successful.
    pub fn is_success(&self) -> bool {
        self.valid && self.error.is_none()
    }
}

/// Reload event notifier.
///
/// Uses a channel to send reload events to consumers.
pub struct ReloadNotifier {
    sender: Sender<ReloadEvent>,
    receiver: Arc<Mutex<Receiver<ReloadEvent>>>,
}

impl ReloadNotifier {
    /// Create a new reload notifier.
    pub fn new() -> Self {
        let (sender, receiver) = channel();
        Self {
            sender,
            receiver: Arc::new(Mutex::new(receiver)),
        }
    }

    /// Send a reload event.
    pub fn notify(&self, event: ReloadEvent) {
        if let Err(e) = self.sender.send(event) {
            error!("Failed to send reload event: {}", e);
        }
    }

    /// Try to receive a reload event (non-blocking).
    pub fn try_recv(&self) -> Option<ReloadEvent> {
        match self.receiver.lock().unwrap().try_recv() {
            Ok(event) => Some(event),
            Err(TryRecvError::Empty) => None,
            Err(TryRecvError::Disconnected) => {
                error!("Reload event channel disconnected");
                None
            }
        }
    }

    /// Receive a reload event (blocking).
    pub fn recv(&self) -> Option<ReloadEvent> {
        match self.receiver.lock().unwrap().recv() {
            Ok(event) => Some(event),
            Err(e) => {
                error!("Failed to receive reload event: {}", e);
                None
            }
        }
    }
}

impl Default for ReloadNotifier {
    fn default() -> Self {
        Self::new()
    }
}

/// Configuration file watcher.
///
/// Watches a YAML config file for changes and triggers reload events.
pub struct ConfigWatcher {
    config: HotReloadConfig,
    notifier: Arc<ReloadNotifier>,
    watcher: Option<RecommendedWatcher>,
    last_reload: Arc<Mutex<Option<SystemTime>>>,
}

impl std::fmt::Debug for ConfigWatcher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ConfigWatcher")
            .field("config", &self.config)
            .field("watcher_active", &self.watcher.is_some())
            .finish()
    }
}

impl ConfigWatcher {
    /// Create a new config watcher.
    pub fn new(
        file_path: impl Into<PathBuf>,
        notifier: Arc<ReloadNotifier>,
    ) -> Result<Self, ConfigWatcherError> {
        let file_path = file_path.into();

        if !file_path.exists() {
            return Err(ConfigWatcherError::FileNotFound(file_path));
        }

        Ok(Self {
            config: HotReloadConfig::new(file_path),
            notifier,
            watcher: None,
            last_reload: Arc::new(Mutex::new(None)),
        })
    }

    /// Create a watcher with custom config.
    pub fn with_config(
        config: HotReloadConfig,
        notifier: Arc<ReloadNotifier>,
    ) -> Result<Self, ConfigWatcherError> {
        if config.enabled && !config.file_path.exists() {
            return Err(ConfigWatcherError::FileNotFound(config.file_path.clone()));
        }

        Ok(Self {
            config,
            notifier,
            watcher: None,
            last_reload: Arc::new(Mutex::new(None)),
        })
    }

    /// Start watching the config file.
    pub fn start(&mut self) -> Result<(), ConfigWatcherError> {
        if !self.config.enabled {
            debug!("Hot-reload is disabled, skipping watcher start");
            return Ok(());
        }

        info!("Starting config watcher for: {:?}", self.config.file_path);

        let file_path = self.config.file_path.clone();
        let notifier = self.notifier.clone();
        let debounce_ms = self.config.debounce_ms;
        let last_reload = self.last_reload.clone();

        let mut watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    if should_reload(&event) {
                        debug!("File change detected: {:?}", event);
                        handle_reload(&file_path, &notifier, debounce_ms, &last_reload);
                    }
                }
                Err(e) => {
                    error!("Watch error: {:?}", e);
                }
            }
        })
        .map_err(ConfigWatcherError::WatcherCreation)?;

        watcher
            .watch(&self.config.file_path, RecursiveMode::NonRecursive)
            .map_err(ConfigWatcherError::WatcherStart)?;

        self.watcher = Some(watcher);

        info!("Config watcher started successfully");
        Ok(())
    }

    /// Stop watching the config file.
    pub fn stop(&mut self) -> Result<(), ConfigWatcherError> {
        if let Some(mut watcher) = self.watcher.take() {
            info!("Stopping config watcher");
            watcher
                .unwatch(&self.config.file_path)
                .map_err(ConfigWatcherError::WatcherStop)?;
        }
        Ok(())
    }

    /// Check if watcher is running.
    pub fn is_running(&self) -> bool {
        self.watcher.is_some()
    }

    /// Get the watched file path.
    pub fn file_path(&self) -> &Path {
        &self.config.file_path
    }
}

impl Drop for ConfigWatcher {
    fn drop(&mut self) {
        let _ = self.stop();
    }
}

/// Check if an event should trigger a reload.
fn should_reload(event: &Event) -> bool {
    matches!(
        event.kind,
        EventKind::Modify(_) | EventKind::Create(_) | EventKind::Remove(_)
    )
}

/// Handle a config reload.
fn handle_reload(
    file_path: &Path,
    notifier: &ReloadNotifier,
    debounce_ms: u64,
    last_reload: &Arc<Mutex<Option<SystemTime>>>,
) {
    // Debounce: skip if reload happened recently
    let now = SystemTime::now();
    {
        let mut last = last_reload.lock().unwrap();
        if let Some(last_time) = *last {
            if let Ok(elapsed) = now.duration_since(last_time) {
                if elapsed.as_millis() < debounce_ms as u128 {
                    debug!("Debouncing reload ({}ms since last)", elapsed.as_millis());
                    return;
                }
            }
        }
        *last = Some(now);
    }

    info!("Reloading config from: {:?}", file_path);

    // Load and validate new config
    let result = load_and_validate_config(file_path);

    match result {
        Ok(config) => {
            info!("Config reloaded successfully");
            notifier.notify(ReloadEvent {
                timestamp: now,
                file_path: file_path.to_path_buf(),
                config,
                valid: true,
                error: None,
            });
        }
        Err(e) => {
            warn!("Config reload failed validation: {}", e);
            // Send event with error, but create a placeholder config
            notifier.notify(ReloadEvent {
                timestamp: now,
                file_path: file_path.to_path_buf(),
                config: YamlConfig::default(),
                valid: false,
                error: Some(e),
            });
        }
    }
}

/// Load and validate a config file.
fn load_and_validate_config(file_path: &Path) -> Result<YamlConfig, String> {
    // Load YAML
    let config = YamlConfig::from_file(file_path)
        .map_err(|e| format!("Failed to parse YAML: {}", e))?;

    // Validate
    config
        .validate()
        .map_err(|e| format!("Validation failed: {}", e))?;

    Ok(config)
}

/// Config watcher errors.
#[derive(Debug, thiserror::Error)]
pub enum ConfigWatcherError {
    #[error("Config file not found: {0:?}")]
    FileNotFound(PathBuf),

    #[error("Failed to create file watcher: {0}")]
    WatcherCreation(notify::Error),

    #[error("Failed to start watching: {0}")]
    WatcherStart(notify::Error),

    #[error("Failed to stop watching: {0}")]
    WatcherStop(notify::Error),

    #[error("Config error: {0}")]
    Config(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_config() -> String {
        r#"
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
  workers: 10
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/test"
"#
        .to_string()
    }

    #[test]
    fn test_hot_reload_config_creation() {
        let config = HotReloadConfig::new("test.yaml");
        assert!(config.enabled);
        assert_eq!(config.file_path, PathBuf::from("test.yaml"));
        assert_eq!(config.debounce_ms, 500);

        let disabled = HotReloadConfig::disabled();
        assert!(!disabled.enabled);
    }

    #[test]
    fn test_hot_reload_config_builders() {
        let config = HotReloadConfig::new("test.yaml")
            .disable()
            .with_debounce_ms(1000);

        assert!(!config.enabled);
        assert_eq!(config.debounce_ms, 1000);
    }

    #[test]
    fn test_reload_event() {
        let event = ReloadEvent {
            timestamp: SystemTime::now(),
            file_path: PathBuf::from("test.yaml"),
            config: YamlConfig::default(),
            valid: true,
            error: None,
        };

        assert!(event.is_success());

        let failed = ReloadEvent {
            timestamp: SystemTime::now(),
            file_path: PathBuf::from("test.yaml"),
            config: YamlConfig::default(),
            valid: false,
            error: Some("error".to_string()),
        };

        assert!(!failed.is_success());
    }

    #[test]
    fn test_reload_notifier() {
        let notifier = ReloadNotifier::new();

        // Should be empty initially
        assert!(notifier.try_recv().is_none());

        // Send event
        let event = ReloadEvent {
            timestamp: SystemTime::now(),
            file_path: PathBuf::from("test.yaml"),
            config: YamlConfig::default(),
            valid: true,
            error: None,
        };

        notifier.notify(event.clone());

        // Should receive event
        let received = notifier.try_recv();
        assert!(received.is_some());
        assert!(received.unwrap().is_success());

        // Should be empty again
        assert!(notifier.try_recv().is_none());
    }

    #[test]
    fn test_config_watcher_creation_file_not_found() {
        let notifier = Arc::new(ReloadNotifier::new());
        let result = ConfigWatcher::new("nonexistent.yaml", notifier);

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ConfigWatcherError::FileNotFound(_)
        ));
    }

    #[test]
    fn test_config_watcher_creation_success() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.yaml");
        fs::write(&config_path, create_test_config()).unwrap();

        let notifier = Arc::new(ReloadNotifier::new());
        let watcher = ConfigWatcher::new(&config_path, notifier);

        assert!(watcher.is_ok());
        let watcher = watcher.unwrap();
        assert_eq!(watcher.file_path(), config_path.as_path());
        assert!(!watcher.is_running());
    }

    #[test]
    fn test_load_and_validate_config_success() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("test.yaml");
        fs::write(&config_path, create_test_config()).unwrap();

        let result = load_and_validate_config(&config_path);
        assert!(result.is_ok());

        let config = result.unwrap();
        assert_eq!(config.config.base_url, "https://test.com");
    }

    #[test]
    fn test_load_and_validate_config_invalid_yaml() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.yaml");
        fs::write(&config_path, "invalid: yaml: content:").unwrap();

        let result = load_and_validate_config(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Failed to parse YAML"));
    }

    #[test]
    fn test_load_and_validate_config_invalid_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("invalid.yaml");
        fs::write(
            &config_path,
            r#"
version: "1.0"
config:
  baseUrl: "not-a-url"
  duration: "invalid"
load:
  model: "concurrent"
scenarios: []
"#,
        )
        .unwrap();

        let result = load_and_validate_config(&config_path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Validation failed"));
    }

    #[test]
    fn test_should_reload() {
        let modify_event = Event {
            kind: EventKind::Modify(notify::event::ModifyKind::Any),
            paths: vec![],
            attrs: Default::default(),
        };
        assert!(should_reload(&modify_event));

        let create_event = Event {
            kind: EventKind::Create(notify::event::CreateKind::Any),
            paths: vec![],
            attrs: Default::default(),
        };
        assert!(should_reload(&create_event));

        let access_event = Event {
            kind: EventKind::Access(notify::event::AccessKind::Any),
            paths: vec![],
            attrs: Default::default(),
        };
        assert!(!should_reload(&access_event));
    }
}
