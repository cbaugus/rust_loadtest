# Configuration Hot-Reload

## Overview

Configuration hot-reload allows you to modify YAML configuration files during test execution without stopping or restarting the load test. Changes are automatically detected, validated, and applied in real-time.

## Key Features

✅ **Automatic file watching** - Detects changes to YAML config files
✅ **Validation before reload** - Ensures new config is valid before applying
✅ **Graceful reload** - Updates config without stopping the test
✅ **Reload notifications** - Event-based system to handle config changes
✅ **Debouncing** - Prevents multiple reloads for rapid file changes
✅ **Development mode** - Enable/disable hot-reload as needed

## When to Use Hot-Reload

### Development & Testing

- **Rapid iteration**: Adjust load parameters without restarting tests
- **A/B testing**: Compare different configurations in real-time
- **Debugging**: Fine-tune settings while observing behavior
- **Experimentation**: Try different scenarios on the fly

### Production Monitoring

- **Load adjustment**: Scale workers up/down based on system capacity
- **Scenario updates**: Modify traffic patterns during long-running tests
- **Emergency response**: Quickly reduce load if system shows stress

## Quick Start

### Basic Usage

```rust
use rust_loadtest::config_hot_reload::{ConfigWatcher, ReloadNotifier};
use std::sync::Arc;

// Create notifier
let notifier = Arc::new(ReloadNotifier::new());

// Create watcher
let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;

// Start watching
watcher.start()?;

// Check for reload events
if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        println!("Config reloaded successfully");
        // Apply new config
    } else {
        println!("Config reload failed: {:?}", event.error);
    }
}

// Stop watching when done
watcher.stop()?;
```

### CLI Usage (Development Mode)

```bash
# Enable hot-reload in development mode
rust-loadtest --config loadtest.yaml --dev-mode

# Or with environment variable
DEV_MODE=true rust-loadtest --config loadtest.yaml
```

## Configuration

### HotReloadConfig

Control hot-reload behavior with `HotReloadConfig`:

```rust
use rust_loadtest::config_hot_reload::HotReloadConfig;

// Enable hot-reload with defaults
let config = HotReloadConfig::new("loadtest.yaml");

// Disable hot-reload
let config = HotReloadConfig::disabled();

// Custom debounce duration
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(1000);  // Wait 1 second after changes

// Enable/disable dynamically
let config = HotReloadConfig::new("loadtest.yaml")
    .disable()
    .enable();
```

### Debouncing

Debouncing prevents multiple reloads when files are saved rapidly (e.g., by IDEs):

```rust
// Short debounce (100ms) - more responsive
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(100);

// Default debounce (500ms) - balanced
let config = HotReloadConfig::new("loadtest.yaml");

// Long debounce (2000ms) - reduces reload frequency
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(2000);
```

**Recommendation**: Use default 500ms for most cases. Increase if you experience too many reloads.

## Reload Events

### ReloadEvent Structure

```rust
pub struct ReloadEvent {
    /// Timestamp of the reload
    pub timestamp: SystemTime,

    /// Path to the config file
    pub file_path: PathBuf,

    /// The reloaded configuration
    pub config: YamlConfig,

    /// Whether validation succeeded
    pub valid: bool,

    /// Validation error message (if any)
    pub error: Option<String>,
}
```

### Handling Reload Events

```rust
let notifier = Arc::new(ReloadNotifier::new());
let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;
watcher.start()?;

// Poll for events (non-blocking)
loop {
    if let Some(event) = notifier.try_recv() {
        if event.is_success() {
            println!("✅ Config reloaded at {:?}", event.timestamp);
            println!("   Base URL: {}", event.config.config.base_url);
            println!("   Workers: {}", event.config.config.workers);

            // Apply new configuration
            apply_config(event.config);
        } else {
            eprintln!("❌ Config reload failed:");
            eprintln!("   Error: {}", event.error.unwrap());
            // Keep using old config
        }
    }

    // Continue test execution
    thread::sleep(Duration::from_millis(100));
}
```

### Blocking Event Reception

```rust
// Wait for the next reload event (blocks)
if let Some(event) = notifier.recv() {
    println!("Config changed: {:?}", event);
}
```

## Validation Before Reload

All config changes are validated before being applied:

### Validation Steps

1. **YAML parsing** - Ensure valid YAML syntax
2. **Schema validation** - Check required fields and types
3. **URL validation** - Verify baseUrl format
4. **Duration validation** - Check duration strings (e.g., "5m")
5. **Load model validation** - Validate load model parameters
6. **Scenario validation** - Ensure scenarios are well-formed

### Handling Validation Failures

When validation fails, the old configuration remains active:

```rust
if let Some(event) = notifier.try_recv() {
    if !event.is_success() {
        eprintln!("⚠️  Config reload failed - keeping current config");
        eprintln!("   Reason: {}", event.error.unwrap());

        // Log validation error
        log::warn!("Config validation failed: {:?}", event.error);

        // Continue with existing config
        return;
    }

    // Apply new config only if valid
    apply_config(event.config);
}
```

## Real-World Examples

### Example 1: Dynamic Worker Scaling

```rust
use rust_loadtest::config_hot_reload::{ConfigWatcher, ReloadNotifier};
use std::sync::{Arc, RwLock};

// Shared config
let current_config = Arc::new(RwLock::new(initial_config));

// Start watcher
let notifier = Arc::new(ReloadNotifier::new());
let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;
watcher.start()?;

// Background thread to handle reloads
let config_clone = current_config.clone();
thread::spawn(move || {
    loop {
        if let Some(event) = notifier.try_recv() {
            if event.is_success() {
                let new_workers = event.config.config.workers;

                // Update shared config
                let mut config = config_clone.write().unwrap();
                *config = event.config;

                println!("🔄 Workers updated: {} -> {}",
                    config.config.workers, new_workers);
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
});

// Main test continues, reading from shared config
```

### Example 2: Scenario Hot-Swapping

```yaml
# Before: Testing checkout flow
scenarios:
  - name: "Checkout Flow"
    weight: 100
    steps:
      - request:
          method: "POST"
          path: "/checkout"

# After: Switch to browsing flow (save file to trigger reload)
scenarios:
  - name: "Browse Products"
    weight: 100
    steps:
      - request:
          method: "GET"
          path: "/products"
```

The test automatically picks up the new scenario without restarting.

### Example 3: Load Pattern Adjustment

```yaml
# Initial: Gentle load
load:
  model: "rps"
  target: 50

# Update: Ramp up to stress test (save to reload)
load:
  model: "rps"
  target: 500
```

### Example 4: Emergency Load Reduction

```yaml
# High load causing system stress
config:
  workers: 100
load:
  model: "rps"
  target: 1000

# Reduce immediately (save to reload)
config:
  workers: 10
load:
  model: "rps"
  target: 50
```

## Integration with Main Test Loop

### Pattern 1: Separate Reload Thread

```rust
// Main test loop
let notifier = Arc::new(ReloadNotifier::new());
let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;
watcher.start()?;

// Spawn reload handler
let config_ref = Arc::new(RwLock::new(config));
let config_clone = config_ref.clone();
thread::spawn(move || {
    loop {
        if let Some(event) = notifier.try_recv() {
            if event.is_success() {
                let mut cfg = config_clone.write().unwrap();
                *cfg = event.config;
                println!("Config reloaded");
            }
        }
        thread::sleep(Duration::from_millis(100));
    }
});

// Continue test with config_ref
```

### Pattern 2: Periodic Polling

```rust
let notifier = Arc::new(ReloadNotifier::new());
let mut watcher = ConfigWatcher::new("loadtest.yaml", notifier.clone())?;
watcher.start()?;

loop {
    // Check for reload
    if let Some(event) = notifier.try_recv() {
        if event.is_success() {
            config = event.config;
        }
    }

    // Execute test iteration
    execute_iteration(&config);

    thread::sleep(Duration::from_millis(100));
}
```

## Best Practices

### 1. Always Validate Before Applying

```rust
if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        // ✅ Only apply validated config
        apply_config(event.config);
    } else {
        // ❌ Don't apply invalid config
        log::error!("Validation failed: {:?}", event.error);
    }
}
```

### 2. Use Appropriate Debounce

```rust
// Development: Short debounce for quick iteration
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(100);

// Production: Longer debounce to avoid accidental reloads
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(2000);
```

### 3. Log Reload Events

```rust
if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        info!("Config reloaded from {:?}", event.file_path);
        info!("New workers: {}", event.config.config.workers);
        info!("New RPS: {:?}", event.config.load);
    } else {
        error!("Reload failed: {}", event.error.unwrap());
    }
}
```

### 4. Handle Graceful Transitions

```rust
if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        let old_workers = config.config.workers;
        let new_workers = event.config.config.workers;

        if new_workers > old_workers {
            println!("Scaling up: {} -> {}", old_workers, new_workers);
            // Gradually add workers
        } else if new_workers < old_workers {
            println!("Scaling down: {} -> {}", old_workers, new_workers);
            // Gradually remove workers
        }

        config = event.config;
    }
}
```

### 5. Disable in Production (If Needed)

```rust
let config = if is_production() {
    HotReloadConfig::disabled()
} else {
    HotReloadConfig::new("loadtest.yaml")
};
```

## Troubleshooting

### Config Not Reloading

**Problem**: File changes but no reload event.

**Solutions**:
```rust
// 1. Check if watcher is running
assert!(watcher.is_running());

// 2. Check if hot-reload is enabled
let config = HotReloadConfig::new("loadtest.yaml").enable();

// 3. Verify file path is correct
println!("Watching: {:?}", watcher.file_path());

// 4. Check for events
if let Some(event) = notifier.try_recv() {
    println!("Got event: {:?}", event);
}
```

### Too Many Reload Events

**Problem**: File saves trigger multiple reloads.

**Solution**: Increase debounce duration:
```rust
let config = HotReloadConfig::new("loadtest.yaml")
    .with_debounce_ms(1000);  // Wait 1 second
```

### Validation Failing

**Problem**: Config changes but validation fails.

**Solution**: Check validation error:
```rust
if let Some(event) = notifier.try_recv() {
    if !event.is_success() {
        eprintln!("Validation failed: {}", event.error.unwrap());
        // Fix config file based on error message
    }
}
```

### Watcher Stops After Error

**Problem**: Watcher stops working after file error.

**Solution**: The watcher continues even after validation errors. Check:
```rust
// Verify watcher is still running
if !watcher.is_running() {
    watcher.start()?;
}
```

## Performance Considerations

### CPU Impact

- **File watching**: Minimal CPU overhead (<0.1%)
- **Validation**: ~10ms per reload (one-time cost)
- **Event handling**: Negligible impact

### Memory Impact

- **Watcher**: ~100KB
- **Event queue**: Minimal (bounded by channel)
- **Config copies**: One copy per reload event

### Debounce Tuning

| Debounce | Use Case | Pros | Cons |
|----------|----------|------|------|
| 100ms | Development | Very responsive | May reload unnecessarily |
| 500ms (default) | General use | Balanced | Slight delay |
| 1000ms+ | Production | Fewer reloads | Less responsive |

## Security Considerations

### File Permissions

Ensure config files have appropriate permissions:

```bash
# Recommended: Read-only for load test user
chmod 444 loadtest.yaml

# Development: Read-write for editing
chmod 644 loadtest.yaml
```

### Validation

Hot-reload **always** validates new configs before applying. Invalid configs are rejected:

```rust
// Invalid URL
config:
  baseUrl: "not-a-valid-url"  // ❌ Rejected

// Invalid duration
config:
  duration: "invalid"  // ❌ Rejected

// Negative workers
config:
  workers: -10  // ❌ Rejected
```

### Audit Logging

Log all reload events for security auditing:

```rust
if let Some(event) = notifier.try_recv() {
    audit_log!(
        "Config reload: path={:?}, valid={}, user={}, timestamp={:?}",
        event.file_path,
        event.valid,
        get_current_user(),
        event.timestamp
    );
}
```

## Advanced Usage

### Custom Validation Rules

Add application-specific validation:

```rust
if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        // Custom validation
        if event.config.config.workers > max_workers {
            eprintln!("Workers exceed limit: {}", event.config.config.workers);
            return;
        }

        // Apply config
        apply_config(event.config);
    }
}
```

### Metrics on Reload

Track reload metrics:

```rust
let reload_counter = AtomicU64::new(0);
let failed_reload_counter = AtomicU64::new(0);

if let Some(event) = notifier.try_recv() {
    if event.is_success() {
        reload_counter.fetch_add(1, Ordering::Relaxed);
    } else {
        failed_reload_counter.fetch_add(1, Ordering::Relaxed);
    }
}
```

### Multiple Config Files

Watch multiple config files:

```rust
let notifier = Arc::new(ReloadNotifier::new());

let mut watcher1 = ConfigWatcher::new("main.yaml", notifier.clone())?;
let mut watcher2 = ConfigWatcher::new("scenarios.yaml", notifier.clone())?;

watcher1.start()?;
watcher2.start()?;

// Handle events from both watchers
if let Some(event) = notifier.try_recv() {
    println!("Config changed: {:?}", event.file_path);
}
```

## Related Documentation

- [YAML Configuration](/docs/YAML_CONFIG.md)
- [Configuration Validation](/docs/CONFIG_VALIDATION.md)
- [Configuration Versioning](/docs/CONFIG_VERSIONING.md)
- [Development Mode](/docs/DEVELOPMENT_MODE.md)

## FAQ

### Can I reload during a test run?

Yes, that's the main purpose. Configs reload without stopping the test.

### What happens if the new config is invalid?

The old config remains active. You'll receive an event with `valid: false` and an error message.

### How quickly does reload happen?

Typically within 100-1000ms after file save (depending on debounce setting).

### Can I disable hot-reload in production?

Yes, use `HotReloadConfig::disabled()` or check an environment variable.

### Does it work with version control?

Yes, pulling changes from git will trigger reload if the config file changes.

### What file systems are supported?

Works on all major file systems: ext4, NTFS, APFS, etc.

### Can I reload scenarios without changing workers?

Yes, modify only the scenarios section in your YAML and save. Workers remain unchanged.

## Examples Repository

See `/examples/hot_reload/` for complete working examples:

- `basic_reload.rs` - Simple hot-reload setup
- `dynamic_scaling.rs` - Scale workers based on config changes
- `scenario_switching.rs` - Switch scenarios during test
- `production_safety.rs` - Production-safe reload with validation
