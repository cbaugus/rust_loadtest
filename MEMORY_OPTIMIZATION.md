# Memory Optimization Guide

## OOM Issue Analysis

### Root Causes

Your load test is hitting OOM with 4GB RAM due to several memory-intensive operations:

#### 1. **HDR Histograms (Primary Issue)**
- **Location**: `src/percentiles.rs:88-106`
- **Problem**: Each histogram tracks 1μs to 60s with 3 significant digits
- **Memory per histogram**: ~2-4MB each
- **Unbounded growth**: `MultiLabelPercentileTracker` creates a NEW histogram for:
  - Every unique scenario name
  - Every unique step name (format: `scenario:step`)
  - No upper limit on number of histograms
- **With your config**: Even with just a few scenarios, you're creating dozens of histograms

#### 2. **5000 Concurrent Tasks**
- **Location**: `src/main.rs:243`
- **Problem**: Spawning 5000 tokio tasks
- **Memory**: Each task has stack overhead (~2-8KB) + async state
- **Total overhead**: ~10-40MB just for task structures
- **Compounded by**: Each task loop allocates request builders, responses, etc.

#### 3. **Prometheus Metrics**
- **Location**: `src/metrics.rs`
- **Problem**: Metrics with labels create separate time series
- **Growth**: `HistogramVec` and `IntCounterVec` grow with unique label combinations
- **24h accumulation**: No data reset/rotation over time

#### 4. **Connection Pool Stats**
- **Location**: Tracking connection reuse patterns
- **Accumulates**: Request timing data over entire test duration

### Memory Breakdown Estimate

With your config (`NUM_CONCURRENT_TASKS=5000`, `TARGET_RPS=50000`, `24h`):

```
Component                          Estimated Memory
─────────────────────────────────────────────────────
5000 tokio tasks                   ~40 MB
HDR Histograms (50 scenarios)      ~150 MB
Prometheus time series (24h)       ~500 MB
Connection pool stats              ~100 MB
Request/response buffers in flight ~1-2 GB (at 50k RPS)
Tokio runtime overhead             ~200 MB
─────────────────────────────────────────────────────
TOTAL                              ~2-3 GB minimum
```

**At peak with 50k RPS**, you'd need **6-8GB minimum**.

## Immediate Solutions

### Solution 1: Reduce Concurrent Tasks (RECOMMENDED)

```bash
# Start with reasonable concurrency
NUM_CONCURRENT_TASKS=100    # Down from 5000
TARGET_RPS=5000             # Down from 50000
TEST_DURATION=1h            # Down from 24h
```

**Why**: Memory usage scales roughly linearly with concurrent tasks. Going from 5000→100 saves ~1.5GB.

### Solution 2: Use Realistic Load Patterns

```bash
# Ramp up gradually to find your limit
LOAD_MODEL_TYPE=RampRps
MIN_RPS=100
MAX_RPS=5000
RAMP_DURATION=30m
TEST_DURATION=1h
NUM_CONCURRENT_TASKS=200
```

### Solution 3: Shorter Test Duration

```bash
# Validate first, then scale up
TEST_DURATION=5m     # Quick validation
# Then: TEST_DURATION=30m
# Then: TEST_DURATION=2h
# Finally: TEST_DURATION=24h (if needed)
```

### Solution 4: Disable Percentile Tracking (Future Enhancement)

Currently not configurable, but percentile tracking is the biggest memory consumer.

## Recommended Test Configurations

### 🟢 Small Load Test (Fits in 512MB)
```bash
NUM_CONCURRENT_TASKS=10
TARGET_RPS=500
TEST_DURATION=5m
LOAD_MODEL_TYPE=Rps
```

### 🟡 Medium Load Test (Fits in 2GB)
```bash
NUM_CONCURRENT_TASKS=100
TARGET_RPS=5000
TEST_DURATION=30m
LOAD_MODEL_TYPE=RampRps
MIN_RPS=500
MAX_RPS=5000
RAMP_DURATION=15m
```

### 🟠 High Load Test (Needs 4GB)
```bash
NUM_CONCURRENT_TASKS=500
TARGET_RPS=10000
TEST_DURATION=1h
LOAD_MODEL_TYPE=Rps
```

### 🔴 Maximum Load Test (Needs 8GB+)
```bash
NUM_CONCURRENT_TASKS=1000
TARGET_RPS=25000
TEST_DURATION=2h
LOAD_MODEL_TYPE=RampRps
MIN_RPS=5000
MAX_RPS=25000
RAMP_DURATION=30m
```

## Understanding the Math

### RPS vs Concurrent Tasks

The relationship is: `Concurrent Tasks × (1000ms / Avg Latency) = Sustainable RPS`

Examples:
- 100 tasks × (1000 / 20ms) = **5,000 RPS** (if avg latency is 20ms)
- 500 tasks × (1000 / 20ms) = **25,000 RPS**
- 5000 tasks × (1000 / 20ms) = **250,000 RPS** (unrealistic for single instance)

**Your config attempted**: 5000 tasks targeting 50k RPS
- This implies expected latency: `5000 × 1000 / 50000 = 100ms`
- But at 50k RPS, you'd saturate the target or network first
- Memory would balloon from all the in-flight requests

### Memory per RPS

Rough estimate: **~1MB per 100 sustained RPS over 1 hour**

- 5,000 RPS × 1h = ~50 MB
- 25,000 RPS × 1h = ~250 MB
- 50,000 RPS × 24h = **~12 GB** (not sustainable in 4GB)

## Future Code Improvements

These would require code changes (future issues):

1. **Add `PERCENTILE_TRACKING_ENABLED` flag** - Disable histogram tracking for high-load tests
2. **Add histogram reset interval** - Clear percentile data every N minutes
3. **Limit max histogram labels** - Cap at 100 unique scenarios/steps
4. **Use sampling** - Only track percentiles for 10% of requests at high RPS
5. **Add memory profiling** - Instrument with memory metrics

## Troubleshooting

### Check Current Memory Usage

```bash
# Inside container
docker stats --no-stream

# Check Prometheus metrics
curl localhost:9090/metrics | grep process_resident_memory
```

### Signs of Memory Pressure

- OOM Killer message in docker logs
- Increasing latency as test progresses
- "Cannot allocate memory" errors
- Container restart/exit code 137

### Docker Memory Limit

If running locally, increase Docker memory:

```bash
# docker-compose.yml
services:
  loadtest:
    mem_limit: 8g
    memswap_limit: 8g
```

Or docker run:
```bash
docker run --memory=8g --memory-swap=8g ...
```

## Summary

**Your config needs 8-12GB RAM minimum. With 4GB, start with:**

```bash
NUM_CONCURRENT_TASKS=200
TARGET_RPS=5000
TEST_DURATION=1h
LOAD_MODEL_TYPE=Rps
```

Then scale up gradually while monitoring `docker stats`.