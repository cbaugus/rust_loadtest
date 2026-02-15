use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::time::{self, Duration};
use tracing::{error, info, warn};

use crate::percentiles::rotate_all_histograms;

/// Global atomic flag for runtime control of percentile tracking.
/// When false, workers should skip percentile recording to save memory.
pub static PERCENTILE_TRACKING_ACTIVE: AtomicBool = AtomicBool::new(true);

/// Memory guard configuration.
#[derive(Debug, Clone)]
pub struct MemoryGuardConfig {
    pub warning_threshold_percent: f64,
    pub critical_threshold_percent: f64,
    pub auto_disable_on_warning: bool,
    pub check_interval: Duration,
}

impl Default for MemoryGuardConfig {
    fn default() -> Self {
        Self {
            warning_threshold_percent: 80.0,
            critical_threshold_percent: 90.0,
            auto_disable_on_warning: true,
            check_interval: Duration::from_secs(5),
        }
    }
}

/// Represents current memory usage and limits.
#[derive(Debug)]
pub struct MemoryStatus {
    pub current_bytes: u64,
    pub limit_bytes: u64,
    pub usage_percent: f64,
}

/// Detects the memory limit for the current process.
///
/// For containerized environments (Docker, Kubernetes), checks cgroup limits.
/// For bare metal, uses system memory as the limit.
///
/// Returns limit in bytes, or None if unable to determine.
#[cfg(target_os = "linux")]
fn detect_memory_limit() -> Option<u64> {
    // Try cgroup v2 first (modern Docker/Kubernetes)
    if let Ok(content) = std::fs::read_to_string("/sys/fs/cgroup/memory.max") {
        if let Ok(limit) = content.trim().parse::<u64>() {
            if limit != u64::MAX {
                info!(limit_mb = limit / 1024 / 1024, "Detected cgroup v2 memory limit");
                return Some(limit);
            }
        }
    }

    // Try cgroup v1 (older Docker/Kubernetes)
    if let Ok(content) = std::fs::read_to_string("/sys/fs/cgroup/memory/memory.limit_in_bytes") {
        if let Ok(limit) = content.trim().parse::<u64>() {
            // cgroup v1 uses a very large number to indicate "no limit"
            if limit < (1u64 << 60) {
                info!(limit_mb = limit / 1024 / 1024, "Detected cgroup v1 memory limit");
                return Some(limit);
            }
        }
    }

    // Fall back to system total memory
    if let Ok(content) = std::fs::read_to_string("/proc/meminfo") {
        for line in content.lines() {
            if line.starts_with("MemTotal:") {
                if let Some(kb_str) = line.split_whitespace().nth(1) {
                    if let Ok(kb) = kb_str.parse::<u64>() {
                        let bytes = kb * 1024;
                        info!(
                            limit_mb = bytes / 1024 / 1024,
                            "Using system total memory as limit (no cgroup limit detected)"
                        );
                        return Some(bytes);
                    }
                }
            }
        }
    }

    None
}

#[cfg(not(target_os = "linux"))]
fn detect_memory_limit() -> Option<u64> {
    // On non-Linux systems, we can't easily detect memory limits
    // Return None and monitoring will be disabled
    warn!("Memory limit detection not supported on this platform - auto-OOM protection disabled");
    None
}

/// Gets current memory usage from /proc/self/status (RSS).
#[cfg(target_os = "linux")]
fn get_current_memory_usage() -> Option<u64> {
    use procfs::process::Process;

    match Process::myself() {
        Ok(me) => {
            if let Ok(stat) = me.stat() {
                // RSS in bytes (Resident Set Size)
                let rss_bytes = stat.rss * 4096; // RSS is in pages, typically 4KB per page
                return Some(rss_bytes);
            }
        }
        Err(e) => {
            tracing::debug!(error = %e, "Failed to read /proc memory stats");
        }
    }
    None
}

#[cfg(not(target_os = "linux"))]
fn get_current_memory_usage() -> Option<u64> {
    None
}

/// Checks current memory status against limits.
pub fn check_memory_status(limit_bytes: u64) -> Option<MemoryStatus> {
    let current_bytes = get_current_memory_usage()?;
    let usage_percent = (current_bytes as f64 / limit_bytes as f64) * 100.0;

    Some(MemoryStatus {
        current_bytes,
        limit_bytes,
        usage_percent,
    })
}

/// State tracking for memory guard to avoid repeated actions.
struct MemoryGuardState {
    warning_triggered: bool,
    critical_triggered: bool,
    percentiles_disabled_at: Option<std::time::Instant>,
}

impl MemoryGuardState {
    fn new() -> Self {
        Self {
            warning_triggered: false,
            critical_triggered: false,
            percentiles_disabled_at: None,
        }
    }
}

/// Spawns a background task that monitors memory usage and takes defensive actions.
///
/// Actions taken based on thresholds:
/// - **Warning threshold**: Disable percentile tracking, rotate histograms
/// - **Critical threshold**: Additional aggressive cleanup (future: could add more)
///
/// This task runs for the lifetime of the application.
pub async fn spawn_memory_guard(config: MemoryGuardConfig) {
    let limit_bytes = match detect_memory_limit() {
        Some(limit) => limit,
        None => {
            warn!("Unable to detect memory limit - auto-OOM protection disabled");
            return;
        }
    };

    info!(
        limit_mb = limit_bytes / 1024 / 1024,
        warning_threshold = config.warning_threshold_percent,
        critical_threshold = config.critical_threshold_percent,
        auto_disable = config.auto_disable_on_warning,
        "Memory guard started - monitoring every {} seconds",
        config.check_interval.as_secs()
    );

    let mut interval = time::interval(config.check_interval);
    let mut state = MemoryGuardState::new();

    loop {
        interval.tick().await;

        let status = match check_memory_status(limit_bytes) {
            Some(s) => s,
            None => {
                tracing::debug!("Unable to read current memory usage");
                continue;
            }
        };

        let current_mb = status.current_bytes / 1024 / 1024;
        let limit_mb = status.limit_bytes / 1024 / 1024;

        // Log periodic status at debug level
        tracing::debug!(
            current_mb = current_mb,
            limit_mb = limit_mb,
            usage_percent = format!("{:.1}", status.usage_percent),
            "Memory status check"
        );

        // Critical threshold (90% by default)
        if status.usage_percent >= config.critical_threshold_percent && !state.critical_triggered {
            error!(
                current_mb = current_mb,
                limit_mb = limit_mb,
                usage_percent = format!("{:.1}", status.usage_percent),
                "⚠️  CRITICAL memory threshold exceeded! Process is at {:.1}% of limit",
                status.usage_percent
            );
            state.critical_triggered = true;

            // At critical level, rotate histograms again to free as much memory as possible
            if config.auto_disable_on_warning {
                info!("Critical threshold: Aggressively rotating histograms");
                rotate_all_histograms();
            }
        }

        // Warning threshold (80% by default)
        if status.usage_percent >= config.warning_threshold_percent && !state.warning_triggered {
            warn!(
                current_mb = current_mb,
                limit_mb = limit_mb,
                usage_percent = format!("{:.1}", status.usage_percent),
                "⚠️  Memory warning threshold exceeded! Process is at {:.1}% of limit",
                status.usage_percent
            );
            state.warning_triggered = true;

            if config.auto_disable_on_warning {
                info!("Auto-OOM protection triggered - taking defensive actions:");
                info!("  1. Disabling percentile tracking to prevent further memory growth");
                info!("  2. Rotating all histograms to free existing memory");

                // Disable percentile tracking globally
                PERCENTILE_TRACKING_ACTIVE.store(false, Ordering::SeqCst);
                state.percentiles_disabled_at = Some(std::time::Instant::now());

                // Clear existing histogram data
                rotate_all_histograms();

                info!("Defensive actions complete - percentile tracking disabled");
            } else {
                info!(
                    "Memory warning threshold exceeded, but auto_disable_on_warning=false - no action taken"
                );
            }
        }

        // If memory drops back below warning threshold, consider re-enabling (with hysteresis)
        if status.usage_percent < config.warning_threshold_percent - 10.0 && state.warning_triggered {
            if let Some(disabled_at) = state.percentiles_disabled_at {
                // Only re-enable if it's been at least 60 seconds since we disabled
                let elapsed = disabled_at.elapsed();
                if elapsed.as_secs() >= 60 {
                    info!(
                        usage_percent = format!("{:.1}", status.usage_percent),
                        "Memory usage dropped below warning threshold - considering re-enabling percentiles"
                    );

                    // Don't automatically re-enable for now - too risky
                    // User can restart the test if they want percentiles back
                    info!("Percentiles remain disabled for safety - restart test to re-enable");
                }
            }

            // Reset warning state (but keep percentiles disabled)
            state.warning_triggered = false;
            state.critical_triggered = false;
        }
    }
}

/// Checks if percentile tracking is currently active.
///
/// Workers should call this before recording percentile data.
pub fn is_percentile_tracking_active() -> bool {
    PERCENTILE_TRACKING_ACTIVE.load(Ordering::Relaxed)
}

/// Initialize percentile tracking flag based on config.
///
/// Should be called at startup before spawning workers.
pub fn init_percentile_tracking_flag(enabled: bool) {
    PERCENTILE_TRACKING_ACTIVE.store(enabled, Ordering::SeqCst);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_memory_guard_config_default() {
        let config = MemoryGuardConfig::default();
        assert_eq!(config.warning_threshold_percent, 80.0);
        assert_eq!(config.critical_threshold_percent, 90.0);
        assert!(config.auto_disable_on_warning);
    }

    #[test]
    fn test_percentile_tracking_flag() {
        // Test that we can read and write the flag
        init_percentile_tracking_flag(true);
        assert!(is_percentile_tracking_active());

        init_percentile_tracking_flag(false);
        assert!(!is_percentile_tracking_active());

        // Reset to default for other tests
        init_percentile_tracking_flag(true);
    }

    #[test]
    fn test_memory_status_calculation() {
        // Simulate a memory status
        let status = MemoryStatus {
            current_bytes: 800_000_000,  // 800 MB
            limit_bytes: 1_000_000_000,  // 1 GB
            usage_percent: 80.0,
        };

        assert_eq!(status.usage_percent, 80.0);
        assert!(status.usage_percent < 90.0); // Below critical
    }
}
