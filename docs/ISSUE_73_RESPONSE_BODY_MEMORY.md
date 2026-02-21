# Issue #73: Fix Memory Leak from Unconsumed Response Bodies

## Problem

At high RPS (50K+), the simple worker (`run_worker`) was accumulating memory rapidly because HTTP response bodies were never consumed. The code only checked the status code but didn't read the response body, causing it to buffer in memory indefinitely.

### Symptoms
- Memory usage growing from 0 to 14GB in ~65 seconds
- Rate: ~215 MB/second at 50K RPS
- ~4.3 KB per request being accumulated
- Auto-OOM protection triggered but memory continued growing
- Process eventually hitting critical threshold (92%+)

### Root Cause

In `src/worker.rs` around line 77-97:
```rust
match req.send().await {
    Ok(response) => {
        let status = response.status().as_u16();
        // ... metrics recording ...
        // ⚠️ Response dropped without consuming body!
    }
}
```

Even though the response object is dropped, the underlying HTTP connection may buffer the response body in memory, especially with HTTP/1.1 keep-alive connections.

## Solution

Explicitly consume the response body to prevent memory accumulation:

```rust
// Explicitly consume and discard response body to prevent memory accumulation (Issue #73)
// At high RPS, unbuffered response bodies can accumulate and cause OOM
let _ = response.bytes().await;
```

This ensures:
1. Response body is fully read from the network
2. Memory is released immediately after reading
3. Connection can be properly reused
4. No buffering accumulation at high RPS

## Impact

### Before Fix
- **Memory growth**: ~215 MB/second at 50K RPS
- **Stability**: Process OOM after 60-90 seconds
- **Critical threshold**: Reached 92.9% in 65 seconds

### After Fix (Expected)
- **Memory growth**: Stable, only from active connections
- **Stability**: Can sustain 50K RPS indefinitely
- **Memory usage**: Should stabilize around 2-4GB for 5000 concurrent tasks

## Testing

### Recommended Test
```bash
# High RPS test for memory stability
export TARGET_URL="http://your-test-server"
export REQUEST_TYPE="GET"
export NUM_CONCURRENT_TASKS=5000
export TEST_DURATION_SECS=300  # 5 minutes
export LOAD_MODEL="rps"
export TARGET_RPS=50000

# Monitor memory during test
watch -n 1 'docker stats'
```

### Expected Metrics
- Memory should stabilize after initial ramp-up (30-60 seconds)
- No continuous memory growth trend
- Auto-OOM protection should not trigger under normal conditions

## Related Issues

- **Issue #66**: PERCENTILE_TRACKING_ENABLED flag
- **Issue #67**: Periodic histogram rotation
- **Issue #68**: Histogram label limits
- **Issue #69**: Memory usage metrics
- **Issue #72**: Auto-OOM protection

This issue completes the Phase 2.5 memory optimization work by fixing the primary memory leak that was overwhelming all other memory management strategies.

## Note on Scenario Worker

The scenario worker (`run_scenario_worker`) was NOT affected by this issue because the scenario executor properly consumes response bodies at line 301 of `src/executor.rs`:

```rust
let body_result = response.text().await;
```

This issue only affected the simple single-request worker mode.
