# Rust HTTP Load Testing Tool

This repository contains a Rust-based HTTP load testing tool designed to generate various types of load on a target endpoint, collect Prometheus metrics, and provide insights into service performance under stress.

## Features

* **Configurable Load Models**: Supports different load profiles including:
    * **Concurrent**: A fixed number of concurrent requests.
    * **RPS (Requests Per Second)**: A constant target rate of requests per second.
    * **RampRps**: A load profile that ramps up to a peak RPS, sustains, and then ramps down.
    * **DailyTraffic**: A more complex model mimicking daily traffic patterns with multiple ramp/sustain phases.
* **Prometheus Metrics**: Exposes real-time metrics (total requests, status codes, concurrent requests) on port 9090 for monitoring.
* **HTTPS Support**: Can target HTTPS endpoints.
* **TLS Verification Control**: Option to skip TLS certificate verification for testing non-production or self-signed certificate environments.
* **Dockerized**: Easily containerized for consistent execution in various environments.

## Prerequisites

Before you begin, ensure you have the following installed:

* [**Rust**](https://www.rust-lang.org/tools/install): Rust toolchain (version 1.77 or newer recommended).
* [**Docker**](https://docs.docker.com/get-docker/): Docker Engine to build and run the containerized application.

## Available Docker Images

This tool is available in two image variants to suit different deployment scenarios:

### Standard Image (Ubuntu-based)
**Tag:** `cbaugus/rust-loadtester:latest` or `cbaugus/rust-loadtester:<branch>`

- **Base:** Ubuntu latest
- **Size:** ~80-100 MB
- **Use case:** Development, testing, debugging in lab environments
- **Features:**
  - Full shell access for troubleshooting
  - Standard system utilities available
  - Easy to debug and inspect
- **Build:** `Dockerfile`

### Static Image (Chainguard-based)
**Tag:** `cbaugus/rust-loadtester:latest-Chainguard` or `cbaugus/rust-loadtester:<branch>-Chainguard`

- **Base:** Chainguard static (distroless)
- **Size:** ~10-15 MB (75% smaller)
- **Use case:** Production, secure environments, minimal attack surface
- **Features:**
  - Zero to minimal CVEs (typically 0-2)
  - No shell or unnecessary binaries
  - Static binary with all dependencies compiled in
  - Maximum security posture
- **Build:** `Dockerfile.static`

**Recommendation:** Use the **static image** for production deployments in secure environments. Use the **standard image** for development and troubleshooting.

## ⚠️ Memory Configuration

Load testing at high concurrency or RPS can consume significant memory. **Read this before running high-load tests.**

### Quick Memory Limits

| Available RAM | Max Concurrent Tasks | Max RPS | Max Duration |
|---------------|---------------------|---------|--------------|
| 512MB         | 10                  | 500     | 5 minutes    |
| 2GB           | 100                 | 5,000   | 30 minutes   |
| 4GB           | 500                 | 10,000  | 1 hour       |
| 8GB+          | 1,000               | 25,000  | 2+ hours     |

### Memory Optimization (Issues #66, #68)

For high-load tests that may cause OOM errors, use memory optimization settings:

\`\`\`bash
docker run --memory=4g \\
  -e TARGET_URL="https://api.example.com" \\
  -e NUM_CONCURRENT_TASKS=500 \\
  -e TARGET_RPS=10000 \\
  -e PERCENTILE_TRACKING_ENABLED=false \\  # <-- Disables histogram tracking
  -e MAX_HISTOGRAM_LABELS=100 \\           # <-- Limits unique labels (if enabled)
  cbaugus/rust-loadtester:latest
\`\`\`

**PERCENTILE_TRACKING_ENABLED=false:**
- Saves 2-4MB per unique scenario/step label
- Disables P50/P90/P95/P99 percentile calculation
- Allows much higher concurrency and RPS
- Prometheus metrics still work normally

**MAX_HISTOGRAM_LABELS=100 (default):**
- Limits memory to 200-400MB for percentile tracking
- Uses LRU eviction for oldest labels
- Warns at 80% capacity
- Increase if you have >100 unique scenario/step combinations

**When to disable percentile tracking:**
- High concurrency tests (>500 tasks)
- High RPS tests (>10,000 RPS)
- Long duration tests (>2 hours without rotation)
- Limited RAM (2-4GB)

**For long-duration tests (24h+), use histogram rotation:**
```bash
docker run --memory=4g \
  -e TARGET_URL="https://api.example.com" \
  -e NUM_CONCURRENT_TASKS=200 \
  -e TARGET_RPS=5000 \
  -e TEST_DURATION=24h \
  -e HISTOGRAM_ROTATION_INTERVAL=15m \  # <-- Rotate every 15 minutes
  cbaugus/rust-loadtester:latest
```

**What histogram rotation does:**
- Clears percentile data every N minutes to free memory
- Keeps histogram labels (no recreation overhead)
- Enables 24h+ tests without OOM
- Logs rotation events for visibility
- Recommended: 15-30 minute intervals for long tests

**Auto-OOM Protection (Issue #72):**

The load tester includes automatic memory protection to prevent OOM crashes:

```bash
docker run --memory=4g \
  -e TARGET_URL="https://api.example.com" \
  -e NUM_CONCURRENT_TASKS=1000 \
  -e TARGET_RPS=20000 \
  -e MEMORY_WARNING_THRESHOLD_PERCENT=80 \      # <-- Warn at 80% memory
  -e MEMORY_CRITICAL_THRESHOLD_PERCENT=90 \     # <-- Critical at 90% memory
  -e AUTO_DISABLE_PERCENTILES_ON_WARNING=true \ # <-- Auto-disable percentiles
  cbaugus/rust-loadtester:latest
```

**How it works:**
- Monitors memory usage every 5 seconds
- Detects memory limits (Docker cgroup-aware)
- At **warning threshold (80%)**:
  - Automatically disables percentile tracking
  - Rotates existing histograms to free memory
  - Logs defensive actions taken
- At **critical threshold (90%)**:
  - Aggressively rotates histograms again
  - Logs critical memory warning
- Works on both bare metal and containerized environments

**Configuration:**
- `MEMORY_WARNING_THRESHOLD_PERCENT` - Warning threshold (default: 80%)
- `MEMORY_CRITICAL_THRESHOLD_PERCENT` - Critical threshold (default: 90%)
- `AUTO_DISABLE_PERCENTILES_ON_WARNING` - Take automatic defensive actions (default: true)

**When to use:**
- Unknown memory requirements
- Long-duration tests where memory may grow
- Protection against misconfiguration
- Production load tests where stability is critical

Set `AUTO_DISABLE_PERCENTILES_ON_WARNING=false` for monitoring-only mode (logs warnings but doesn't take action).

**Response Body Memory Management (Issue #73):**

At high RPS (50K+), HTTP response bodies are now automatically consumed and discarded to prevent memory accumulation. Previous versions only checked status codes without reading response bodies, which could cause rapid memory growth (~215 MB/second at 50K RPS).

**Fixed behavior:**
- Response bodies are explicitly read and discarded in single-request mode
- Prevents unbuffered response accumulation
- Enables sustained high-RPS testing without memory leaks
- Scenario mode was already handling this correctly

**No configuration needed** - this fix is automatic and transparent. If you previously experienced rapid memory growth at high RPS even with percentile tracking disabled, this fix resolves it.

### Pre-configured Examples

See `docker-compose.loadtest-examples.yml` for ready-to-use configurations:

\`\`\`bash
# Small test (512MB RAM)
docker-compose -f docker-compose.loadtest-examples.yml up loadtest-small

# High load test (4GB RAM)
docker-compose -f docker-compose.loadtest-examples.yml up loadtest-high
\`\`\`

📚 **Full documentation:** See `MEMORY_OPTIMIZATION.md` for detailed analysis, memory breakdown, and optimization strategies.

### Memory Monitoring (Issue #69)

Real-time memory metrics are available via Prometheus on port 9090:

**Available Metrics:**
- `rust_loadtest_process_memory_rss_bytes` - Resident set size (actual RAM used)
- `rust_loadtest_process_memory_virtual_bytes` - Virtual memory size
- `rust_loadtest_histogram_count` - Number of active HDR histograms
- `rust_loadtest_histogram_memory_estimate_bytes` - Estimated histogram memory (3MB per histogram)

**Example queries:**
\`\`\`promql
# Memory usage in MB
rust_loadtest_process_memory_rss_bytes / 1024 / 1024

# Memory usage percentage (if you know container limit)
(rust_loadtest_process_memory_rss_bytes / 4294967296) * 100  # For 4GB limit

# Histogram memory overhead
rust_loadtest_histogram_memory_estimate_bytes / 1024 / 1024
\`\`\`

**Set up alerts:**
\`\`\`yaml
# Prometheus alert when approaching 80% of 4GB limit
- alert: LoadTestHighMemory
  expr: rust_loadtest_process_memory_rss_bytes > 3.4e9
  annotations:
    summary: "Load test using >80% of memory limit"
\`\`\`

## Project Structure

```
.
├── Cargo.toml
├── src
│   └── main.rs
├── Dockerfile              # Standard Ubuntu-based build
└── Dockerfile.static       # Minimal Chainguard static build
```


## Running the Load Test

The load testing tool is configured primarily through environment variables passed to the Docker container. You can select different load models by setting the LOAD_MODEL_TYPE environment variable.

### Common Environment Variables

* TARGET_URL (Required): The full URL of the endpoint you want to load test (e.g., http://example.com/api/data or https://secure-api.com/status).
* REQUEST_TYPE (Optional, default: POST): The HTTP method to use for requests. Supported values are "GET" and "POST".
* NUM_CONCURRENT_TASKS (Optional, default: 10): The maximum number of concurrent HTTP requests (worker tasks) that the load generator will attempt to maintain. This acts as a concurrency limit.
* TEST_DURATION (Optional, default: 2h): The total duration for which the load test will run. Accepts values like 10m (10 minutes), 1h (1 hour), 3d (3 days).
* SKIP_TLS_VERIFY (Optional, default: false): Set to "true" to skip TLS/SSL certificate verification for HTTPS endpoints. Use with caution, primarily for testing environments with self-signed certificates.
* CLIENT_CERT_PATH (Optional): Path to the client's PEM-encoded public certificate file for mTLS.
* CLIENT_KEY_PATH (Optional): Path to the client's PEM-encoded PKCS#8 private key file for mTLS. Both `CLIENT_CERT_PATH` and `CLIENT_KEY_PATH` must be provided to enable mTLS.
* RESOLVE_TARGET_ADDR (Optional): Allows overriding DNS resolution for the `TARGET_URL`. The format is `"hostname:ip_address:port"`. For example, if `TARGET_URL` is `http://example.com/api` and `RESOLVE_TARGET_ADDR` is set to `"example.com:192.168.1.50:8080"`, all requests to `example.com` will be directed to `192.168.1.50` on port `8080`. This is useful for targeting services not in DNS or for specific routing during tests.
* PERCENTILE_TRACKING_ENABLED (Optional, default: true): Set to "false" to disable HDR histogram tracking for percentile latency calculation. Disabling this can save significant memory (2-4MB per unique scenario/step) in high-load tests. When disabled, P50/P90/P95/P99 percentiles won't be available, but Prometheus metrics continue to work. See [Memory Configuration](#️-memory-configuration) for details.
* MAX_HISTOGRAM_LABELS (Optional, default: 100): Maximum number of unique scenario/step labels to track for percentile calculation. Uses LRU eviction when limit is reached. Each label consumes 2-4MB. Increase for tests with many unique scenarios, or decrease to save memory. Warning logged at 80% capacity.
* HISTOGRAM_ROTATION_INTERVAL (Optional, default: disabled): Periodically reset histogram data to prevent unbounded memory growth in long tests. Format: `15m`, `1h`, `2h`. Clears percentile data while keeping labels. Essential for 24h+ tests. Example: `HISTOGRAM_ROTATION_INTERVAL=15m`
* MEMORY_WARNING_THRESHOLD_PERCENT (Optional, default: 80.0): Memory usage percentage that triggers warning and defensive actions. When memory exceeds this threshold, auto-OOM protection can automatically disable percentile tracking to prevent crashes.
* MEMORY_CRITICAL_THRESHOLD_PERCENT (Optional, default: 90.0): Memory usage percentage that triggers critical warnings and aggressive cleanup. At this level, histograms are rotated to free as much memory as possible.
* AUTO_DISABLE_PERCENTILES_ON_WARNING (Optional, default: true): When true, automatically disables percentile tracking and rotates histograms when memory warning threshold is exceeded. Set to false for monitoring-only mode (logs warnings without taking action).

Load Model Specific Environment Variables
The behavior of the load test is determined by LOAD_MODEL_TYPE and its associated variables:

### 1. Concurrent Model

LOAD_MODEL_TYPE="Concurrent"

This model simply maintains NUM_CONCURRENT_TASKS sending requests as fast as the target service can respond, for the TEST_DURATION.

Example docker run command:

```bash
docker run --rm \
  -e TARGET_URL="http://jsonplaceholder.typicode.com/todos/1" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="5m" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  cbaugus/rust-loadtester:latest
```

### 2. RPS (Requests Per Second) Model

LOAD_MODEL_TYPE="Rps"

This model aims to achieve a constant overall requests per second across all tasks.

Additional Environment Variable:

* TARGET_RPS (Required for Rps model): The desired total requests per second (e.g., 200).
Example docker run command:

```bash
docker run --rm \
  -e TARGET_URL="http://jsonplaceholder.typicode.com/todos/1" \
  -e NUM_CONCURRENT_TASKS="100" \
  -e TEST_DURATION="5m" \
  -e LOAD_MODEL_TYPE="Rps" \
  -e TARGET_RPS="200" \
  cbaugus/rust-loadtester:latest
```

### 3. RampRps (Ramping Requests Per Second) Model

LOAD_MODEL_TYPE="RampRps"

This model ramps the RPS up to a peak, sustains it, and then ramps down. The TEST_DURATION is divided into three equal phases: ramp-up, peak sustain, and ramp-down.

Additional Environment Variables:

* MIN_RPS (Required for RampRps model): The starting and ending RPS for the ramp (e.g., 50).
* MAX_RPS (Required for RampRps model): The peak RPS during the test (e.g., 500).
* RAMP_DURATION (Optional, default: TEST_DURATION): The total duration over which the ramp-up/sustain/ramp-down profile should occur. If this is shorter than TEST_DURATION, the load will remain at MIN_RPS after the ramp profile completes until TEST_DURATION is met.
Example docker run command:

```bash
docker run --rm \
  -e TARGET_URL="http://jsonplaceholder.typicode.com/todos/1" \
  -e NUM_CONCURRENT_TASKS="150" \
  -e TEST_DURATION="15m" \
  -e LOAD_MODEL_TYPE="RampRps" \
  -e MIN_RPS="50" \
  -e MAX_RPS="500" \
  cbaugus/rust-loadtester:latest
```

This will run for 15 minutes, with:

Minutes 0-5: RPS ramps from 50 to 500.
Minutes 5-10: RPS holds at 500.
Minutes 10-15: RPS ramps from 500 down to 50.

### 4. DailyTraffic Model

LOAD_MODEL_TYPE="DailyTraffic"

This model allows for complex daily traffic patterns with multiple ramp and sustain phases (e.g., night, morning ramp, peak, mid-day decline, mid-day sustain, evening decline).

Additional Environment Variables:

* DAILY_MIN_RPS: Base load (e.g., night-time traffic).
* DAILY_MID_RPS: Mid-level load (e.g., afternoon traffic).
* DAILY_MAX_RPS: Peak load (e.g., morning rush).
* DAILY_CYCLE_DURATION: Duration of one full daily cycle (e.g., 24h).
* MORNING_RAMP_RATIO (Optional, default: 0.125): Ratio of DAILY_CYCLE_DURATION for ramp from MIN_RPS to MAX_RPS.
* PEAK_SUSTAIN_RATIO (Optional, default: 0.167): Ratio for holding MAX_RPS.
* MID_DECLINE_RATIO (Optional, default: 0.125): Ratio for ramp from MAX_RPS to MID_RPS.
* MID_SUSTAIN_RATIO (Optional, default: 0.167): Ratio for holding MID_RPS.
* EVENING_DECLINE_RATIO (Optional, default: 0.167): Ratio for ramp from MID_RPS to MIN_RPS.
Note: The sum of *_RATIO variables should ideally be 1.0 or less. Any remaining ratio will be MIN_RPS sustain.
Example docker run command:

```bash
docker run --rm \
  -e TARGET_URL="https://your-service.com/daily-endpoint" \
  -e NUM_CONCURRENT_TASKS="200" \
  -e TEST_DURATION="48h" \
  -e LOAD_MODEL_TYPE="DailyTraffic" \
  -e DAILY_MIN_RPS="10" \
  -e DAILY_MID_RPS="200" \
  -e DAILY_MAX_RPS="1000" \
  -e DAILY_CYCLE_DURATION="24h" \
  -e MORNING_RAMP_RATIO="0.10" \
  -e PEAK_SUSTAIN_RATIO="0.15" \
  -e MID_DECLINE_RATIO="0.05" \
  -e MID_SUSTAIN_RATIO="0.20" \
  -e EVENING_DECLINE_RATIO="0.10" \
  cbaugus/rust-loadtester:latest
```

### Choosing Request Type (GET vs POST)

You can configure the tool to send either GET or POST requests using the `REQUEST_TYPE` environment variable:

* `REQUEST_TYPE` (Optional, default: POST): Set to `"GET"` for GET requests or `"POST"` for POST requests.

**Example with GET requests:**

```bash
docker run --rm \
  -e TARGET_URL="https://jsonplaceholder.typicode.com/posts/1" \
  -e REQUEST_TYPE="GET" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="5m" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  cbaugus/rust-loadtester:latest
```

**Example with POST requests (default):**

```bash
docker run --rm \
  -e TARGET_URL="https://your-service.com/api/data" \
  -e REQUEST_TYPE="POST" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="5m" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  cbaugus/rust-loadtester:latest
```

### Sending a JSON Payload (for POST requests)

You can configure the tool to send a JSON body with each POST request (for example, to test login endpoints that expect a JSON payload). This is controlled by two environment variables:

* `SEND_JSON` (Optional, default: false): Set to `"true"` to enable sending a JSON payload in the body of each POST request.
* `JSON_PAYLOAD` (Required if `SEND_JSON=true`): The JSON string to send as the request body.

If `SEND_JSON` is not set or is not `"true"`, POST requests will be sent without a body. Note that JSON payloads are only sent with POST requests, not GET requests.

**Example:**

```bash
docker run --rm \
  -e TARGET_URL="https://your-service.com/login" \
  -e REQUEST_TYPE="POST" \
  -e SEND_JSON="true" \
  -e JSON_PAYLOAD='{"username":"testuser","password":"testpass"}' \
  -e NUM_CONCURRENT_TASKS="20" \
  -e TEST_DURATION="10m" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  cbaugus/rust-loadtester:latest
```

### Using mTLS (Mutual TLS)

To enable mTLS, you need to provide both a client certificate and a client private key. The private key **must be in PKCS#8 format**.

1.  **Mount your certificate and key**: When running in Docker, ensure your certificate and key files are mounted into the container (e.g., using `-v /path/on/host/cert.pem:/path/in/container/cert.pem`).
2.  **Set Environment Variables**:
    *   `CLIENT_CERT_PATH`: Set this to the path *inside the container* where your client certificate PEM file is located.
    *   `CLIENT_KEY_PATH`: Set this to the path *inside the container* where your client private key (PKCS#8 PEM) file is located.

Example `docker run` command with mTLS:

```bash
docker run --rm \
  -v /local/path/to/client.crt:/etc/ssl/certs/client.crt \
  -v /local/path/to/client_pkcs8.key:/etc/ssl/private/client_pkcs8.key \
  -e TARGET_URL="https://your-secure-service.com/api/data" \
  -e CLIENT_CERT_PATH="/etc/ssl/certs/client.crt" \
  -e CLIENT_KEY_PATH="/etc/ssl/private/client_pkcs8.key" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="10m" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  cbaugus/rust-loadtester:latest
```

### Using custom headers

To use custom headers: Set the CUSTOM_HEADERS environment variable when running your application (e.g., in your docker run command):

```bash
docker run --rm \\
-e TARGET_URL="http://your-target.com/api" \\
-e CUSTOM_HEADERS="Authorization: Bearer your_token,X-Api-Key:your_api_key" \\
# ... other environment variables ...
cbaugus/rust-loadtester:latest
```

This will send the specified Authorization and X-Api-Key headers with every request made by the load tester.

#### Escaping commas in header values

If your header values contain commas (e.g., Keep-Alive headers), you need to escape them with a backslash (`\,`). The escaped comma will be included in the header value as a literal comma.

**Example with Keep-Alive headers:**

```bash
docker run --rm \\
-e TARGET_URL="http://your-target.com/api" \\
-e CUSTOM_HEADERS="Connection:keep-alive,Keep-Alive:timeout=5\\,max=200" \\
# ... other environment variables ...
cbaugus/rust-loadtester:latest
```

This will send:
- `Connection: keep-alive`
- `Keep-Alive: timeout=5,max=200`

**Other examples of escaped commas:**

```bash
# Multiple comma-separated values in Accept header
-e CUSTOM_HEADERS="Accept:text/html\\,application/xml\\,application/json"

# Complex Keep-Alive with multiple parameters
-e CUSTOM_HEADERS="Keep-Alive:timeout=5\\,max=1000\\,custom=value"

# Multiple headers with some containing commas
-e CUSTOM_HEADERS="Accept:text/html\\,application/json,User-Agent:MyApp/1.0,Cache-Control:no-cache\\,no-store"
```

**Note:** Only commas that are part of header values need to be escaped. Commas that separate different headers should not be escaped.


**Important Note on Private Key Format:**
If your private key is not in PKCS#8 format (e.g., it's a traditional PKCS#1 RSA key), you'll need to convert it. You can do this using OpenSSL:
```bash
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt -in your_original_private_key.pem -out your_private_key_pkcs8.pem
```

## Monitoring Metrics

The tool exposes Prometheus metrics on port 9090.

To access these metrics:

Ensure port 9090 is accessible: If running locally, it's usually fine. If running in a cloud VM or Kubernetes, ensure the port is opened in the firewall/security groups.

Access the metrics endpoint: Open your browser or use curl to access http://<CONTAINER_IP_OR_HOST_IP>:9090/metrics.
Example curl command (from the host machine running Docker):

```bash
curl http://localhost:9090/metrics
```
