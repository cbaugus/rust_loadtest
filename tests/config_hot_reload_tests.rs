//! Integration tests for config hot-reload (Issue #44).
//!
//! These tests validate:
//! - File watching and change detection
//! - Config validation before reload
//! - Reload notification system
//! - Debouncing of rapid changes
//! - Development mode enable/disable

use rust_loadtest::config_hot_reload::{
    ConfigWatcher, ConfigWatcherError, HotReloadConfig, ReloadNotifier,
};
use std::fs;
use std::sync::Arc;
use std::thread;
use std::time::Duration;
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

fn create_updated_config() -> String {
    r#"
version: "1.0"
config:
  baseUrl: "https://updated.com"
  duration: "10m"
  workers: 20
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Updated Test"
    steps:
      - request:
          method: "POST"
          path: "/updated"
"#
    .to_string()
}

fn create_invalid_config() -> String {
    r#"
version: "1.0"
config:
  baseUrl: "not-a-url"
  duration: "invalid"
  workers: -5
load:
  model: "concurrent"
scenarios: []
"#
    .to_string()
}

#[test]
fn test_hot_reload_config_creation() {
    let config = HotReloadConfig::new("test.yaml");
    assert!(config.enabled);
    assert_eq!(config.file_path.to_str().unwrap(), "test.yaml");
    assert_eq!(config.debounce_ms, 500);

    println!("✅ HotReloadConfig creation works");
}

#[test]
fn test_hot_reload_config_disabled() {
    let config = HotReloadConfig::disabled();
    assert!(!config.enabled);
    assert_eq!(config.debounce_ms, 0);

    println!("✅ HotReloadConfig disabled mode works");
}

#[test]
fn test_hot_reload_config_builders() {
    let config = HotReloadConfig::new("test.yaml")
        .disable()
        .with_debounce_ms(1000);

    assert!(!config.enabled);
    assert_eq!(config.debounce_ms, 1000);

    let enabled = HotReloadConfig::new("test.yaml").enable();
    assert!(enabled.enabled);

    println!("✅ HotReloadConfig builder methods work");
}

#[test]
fn test_reload_notifier_basic() {
    let notifier = ReloadNotifier::new();

    // Should be empty initially
    assert!(notifier.try_recv().is_none());

    println!("✅ ReloadNotifier basic functionality works");
}

#[test]
fn test_reload_notifier_send_receive() {
    use rust_loadtest::yaml_config::YamlConfig;
    use std::path::PathBuf;
    use std::time::SystemTime;

    let notifier = ReloadNotifier::new();

    // Send event
    let event = rust_loadtest::config_hot_reload::ReloadEvent {
        timestamp: SystemTime::now(),
        file_path: PathBuf::from("test.yaml"),
        config: YamlConfig::default(),
        valid: true,
        error: None,
    };

    notifier.notify(event.clone());

    // Receive event
    let received = notifier.try_recv();
    assert!(received.is_some());

    let received_event = received.unwrap();
    assert!(received_event.is_success());
    assert!(received_event.valid);
    assert!(received_event.error.is_none());

    // Should be empty again
    assert!(notifier.try_recv().is_none());

    println!("✅ ReloadNotifier send/receive works");
}

#[test]
fn test_reload_notifier_multiple_events() {
    use rust_loadtest::yaml_config::YamlConfig;
    use std::path::PathBuf;
    use std::time::SystemTime;

    let notifier = ReloadNotifier::new();

    // Send multiple events
    for i in 0..3 {
        let event = rust_loadtest::config_hot_reload::ReloadEvent {
            timestamp: SystemTime::now(),
            file_path: PathBuf::from(format!("test{}.yaml", i)),
            config: YamlConfig::default(),
            valid: true,
            error: None,
        };
        notifier.notify(event);
    }

    // Receive all events
    for _ in 0..3 {
        let received = notifier.try_recv();
        assert!(received.is_some());
    }

    // Should be empty
    assert!(notifier.try_recv().is_none());

    println!("✅ ReloadNotifier handles multiple events");
}

#[test]
fn test_config_watcher_creation_file_not_found() {
    let notifier = Arc::new(ReloadNotifier::new());
    let result = ConfigWatcher::new("nonexistent.yaml", notifier);

    assert!(result.is_err());
    match result.unwrap_err() {
        ConfigWatcherError::FileNotFound(path) => {
            assert_eq!(path.to_str().unwrap(), "nonexistent.yaml");
        }
        _ => panic!("Expected FileNotFound error"),
    }

    println!("✅ ConfigWatcher rejects nonexistent files");
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

    println!("✅ ConfigWatcher creation succeeds with valid file");
}

#[test]
fn test_config_watcher_with_config() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let hot_reload_config = HotReloadConfig::new(&config_path).with_debounce_ms(1000);
    let notifier = Arc::new(ReloadNotifier::new());

    let watcher = ConfigWatcher::with_config(hot_reload_config, notifier);
    assert!(watcher.is_ok());

    println!("✅ ConfigWatcher with custom config works");
}

#[test]
fn test_config_watcher_disabled() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let hot_reload_config = HotReloadConfig::new(&config_path).disable();
    let notifier = Arc::new(ReloadNotifier::new());

    let mut watcher = ConfigWatcher::with_config(hot_reload_config, notifier).unwrap();

    // Start should succeed but not actually watch
    let result = watcher.start();
    assert!(result.is_ok());
    assert!(!watcher.is_running()); // Not running because disabled

    println!("✅ ConfigWatcher respects disabled flag");
}

#[test]
fn test_config_watcher_start_stop() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let notifier = Arc::new(ReloadNotifier::new());
    let mut watcher = ConfigWatcher::new(&config_path, notifier).unwrap();

    // Start watcher
    let result = watcher.start();
    assert!(result.is_ok());
    assert!(watcher.is_running());

    // Stop watcher
    let result = watcher.stop();
    assert!(result.is_ok());
    assert!(!watcher.is_running());

    println!("✅ ConfigWatcher start/stop works");
}

#[test]
fn test_config_watcher_file_change_detection() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let notifier = Arc::new(ReloadNotifier::new());
    let notifier_clone = notifier.clone();
    let mut watcher = ConfigWatcher::new(&config_path, notifier).unwrap();

    // Start watcher
    watcher.start().unwrap();

    // Give watcher time to initialize
    thread::sleep(Duration::from_millis(100));

    // Modify file
    fs::write(&config_path, create_updated_config()).unwrap();

    // Wait for change detection
    thread::sleep(Duration::from_millis(1000));

    // Check for reload event
    let event = notifier_clone.try_recv();
    assert!(event.is_some(), "Should receive reload event");

    let event = event.unwrap();
    assert!(event.is_success(), "Reload should succeed");
    assert_eq!(event.config.config.base_url, "https://updated.com");
    assert_eq!(event.config.config.workers, 20);

    println!("✅ ConfigWatcher detects file changes");
}

#[test]
fn test_config_watcher_invalid_config_handling() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let notifier = Arc::new(ReloadNotifier::new());
    let notifier_clone = notifier.clone();
    let mut watcher = ConfigWatcher::new(&config_path, notifier).unwrap();

    // Start watcher
    watcher.start().unwrap();
    thread::sleep(Duration::from_millis(100));

    // Write invalid config
    fs::write(&config_path, create_invalid_config()).unwrap();

    // Wait for change detection
    thread::sleep(Duration::from_millis(1000));

    // Check for reload event
    let event = notifier_clone.try_recv();
    assert!(event.is_some(), "Should receive reload event even for invalid config");

    let event = event.unwrap();
    assert!(!event.is_success(), "Reload should fail for invalid config");
    assert!(event.error.is_some());

    println!("✅ ConfigWatcher handles invalid config gracefully");
}

#[test]
fn test_config_watcher_debouncing() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    // Short debounce for testing
    let hot_reload_config = HotReloadConfig::new(&config_path).with_debounce_ms(300);
    let notifier = Arc::new(ReloadNotifier::new());
    let notifier_clone = notifier.clone();
    let mut watcher = ConfigWatcher::with_config(hot_reload_config, notifier).unwrap();

    watcher.start().unwrap();
    thread::sleep(Duration::from_millis(100));

    // Make rapid changes
    for i in 0..3 {
        let config = format!(
            r#"
version: "1.0"
config:
  baseUrl: "https://test{}.com"
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
"#,
            i
        );
        fs::write(&config_path, config).unwrap();
        thread::sleep(Duration::from_millis(50)); // Rapid changes
    }

    // Wait for debounce + processing
    thread::sleep(Duration::from_millis(800));

    // Should only get one or two events (debounced)
    let mut event_count = 0;
    while notifier_clone.try_recv().is_some() {
        event_count += 1;
    }

    // Due to debouncing, should be fewer than 3 events
    assert!(
        event_count < 3,
        "Expected fewer than 3 events due to debouncing, got {}",
        event_count
    );

    println!("✅ ConfigWatcher debounces rapid changes (got {} events)", event_count);
}

#[test]
fn test_config_watcher_multiple_changes() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let notifier = Arc::new(ReloadNotifier::new());
    let notifier_clone = notifier.clone();
    let mut watcher = ConfigWatcher::new(&config_path, notifier).unwrap();

    watcher.start().unwrap();
    thread::sleep(Duration::from_millis(100));

    // First change
    fs::write(&config_path, create_updated_config()).unwrap();
    thread::sleep(Duration::from_millis(700));

    // Second change (after debounce)
    fs::write(&config_path, create_test_config()).unwrap();
    thread::sleep(Duration::from_millis(700));

    // Should get two events
    let event1 = notifier_clone.try_recv();
    assert!(event1.is_some());
    assert_eq!(
        event1.unwrap().config.config.base_url,
        "https://updated.com"
    );

    let event2 = notifier_clone.try_recv();
    assert!(event2.is_some());
    assert_eq!(event2.unwrap().config.config.base_url, "https://test.com");

    println!("✅ ConfigWatcher handles multiple distinct changes");
}

#[test]
fn test_reload_event_is_success() {
    use rust_loadtest::config_hot_reload::ReloadEvent;
    use rust_loadtest::yaml_config::YamlConfig;
    use std::path::PathBuf;
    use std::time::SystemTime;

    let success = ReloadEvent {
        timestamp: SystemTime::now(),
        file_path: PathBuf::from("test.yaml"),
        config: YamlConfig::default(),
        valid: true,
        error: None,
    };
    assert!(success.is_success());

    let failed_validation = ReloadEvent {
        timestamp: SystemTime::now(),
        file_path: PathBuf::from("test.yaml"),
        config: YamlConfig::default(),
        valid: false,
        error: Some("Validation failed".to_string()),
    };
    assert!(!failed_validation.is_success());

    let with_error = ReloadEvent {
        timestamp: SystemTime::now(),
        file_path: PathBuf::from("test.yaml"),
        config: YamlConfig::default(),
        valid: true,
        error: Some("Some error".to_string()),
    };
    assert!(!with_error.is_success());

    println!("✅ ReloadEvent.is_success() works correctly");
}

#[test]
fn test_config_watcher_drop_stops_watching() {
    let temp_dir = TempDir::new().unwrap();
    let config_path = temp_dir.path().join("test.yaml");
    fs::write(&config_path, create_test_config()).unwrap();

    let notifier = Arc::new(ReloadNotifier::new());
    {
        let mut watcher = ConfigWatcher::new(&config_path, notifier.clone()).unwrap();
        watcher.start().unwrap();
        assert!(watcher.is_running());
        // Watcher dropped here
    }

    // Change file after drop
    thread::sleep(Duration::from_millis(100));
    fs::write(&config_path, create_updated_config()).unwrap();
    thread::sleep(Duration::from_millis(700));

    // Should not receive event (watcher was dropped)
    let event = notifier.try_recv();
    assert!(event.is_none(), "Should not receive event after drop");

    println!("✅ ConfigWatcher stops watching when dropped");
}

#[test]
fn test_yaml_config_default() {
    use rust_loadtest::yaml_config::YamlConfig;

    let config = YamlConfig::default();
    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://example.com");
    assert_eq!(config.config.workers, 10);
    assert_eq!(config.scenarios.len(), 0);

    println!("✅ YamlConfig::default() works");
}
