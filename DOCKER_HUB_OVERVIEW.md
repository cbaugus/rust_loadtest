# Rust HTTP Load Testing Tool

A high-performance, Rust-based HTTP load testing tool designed for comprehensive service performance testing. Generate realistic traffic patterns, collect Prometheus metrics, and stress-test your endpoints with various load profiles.

## Quick Start

```bash
# Standard Ubuntu-based image - great for development and debugging
docker run --rm \
  -e TARGET_URL="https://your-api.com/endpoint" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="5m" \
  cbaugus/rust-loadtester:latest

# Chainguard static image - production-ready, minimal CVEs
docker run --rm \
  -e TARGET_URL="https://your-api.com/endpoint" \
  -e LOAD_MODEL_TYPE="Rps" \
  -e TARGET_RPS="200" \
  -e NUM_CONCURRENT_TASKS="100" \
  -e TEST_DURATION="10m" \
  cbaugus/rust-loadtester:latest-Chainguard
```

## Available Image Variants

### Standard Image (Ubuntu-based)
**Tags:** `latest`, `main`, `<branch-name>`

- **Size:** ~80-100 MB
- **Base:** Ubuntu latest
- **Best for:** Development, testing, debugging
- **Features:** Full shell access, standard utilities, easy troubleshooting

### Static Image (Chainguard-based) ⭐ Recommended for Production
**Tags:** `latest-Chainguard`, `main-Chainguard`, `<branch-name>-Chainguard`

- **Size:** ~10-15 MB (75% smaller)
- **Base:** Chainguard static (distroless)
- **Best for:** Production deployments, security-conscious environments
- **Features:** Zero to minimal CVEs, no shell, static binary, maximum security posture

## Key Features

### 🚀 Multiple Load Models
- **Concurrent**: Fixed number of concurrent requests at maximum speed
- **RPS**: Constant target requests per second
- **RampRps**: Gradual ramp-up to peak, sustain, then ramp-down
- **DailyTraffic**: Complex daily traffic patterns with multiple phases

### 📊 Built-in Monitoring
- Prometheus metrics exposed on port 9090
- Real-time request tracking
- Status code distribution
- Concurrent request monitoring

### 🔒 Security Features
- HTTPS support with TLS verification control
- mTLS (mutual TLS) support
- Custom headers including authentication tokens
- Minimal attack surface with Chainguard static image

### ⚙️ Advanced Configuration
- GET and POST request support
- JSON payload support for POST requests
- Custom DNS resolution override
- Flexible duration formats (minutes, hours, days)

## Common Use Cases

### Load Testing a REST API
```bash
docker run --rm \
  -e TARGET_URL="https://api.example.com/users" \
  -e REQUEST_TYPE="GET" \
  -e LOAD_MODEL_TYPE="Rps" \
  -e TARGET_RPS="500" \
  -e NUM_CONCURRENT_TASKS="100" \
  -e TEST_DURATION="15m" \
  cbaugus/rust-loadtester:latest-Chainguard
```

### Testing Login Endpoint with JSON
```bash
docker run --rm \
  -e TARGET_URL="https://api.example.com/login" \
  -e REQUEST_TYPE="POST" \
  -e SEND_JSON="true" \
  -e JSON_PAYLOAD='{"username":"testuser","password":"testpass"}' \
  -e LOAD_MODEL_TYPE="Concurrent" \
  -e NUM_CONCURRENT_TASKS="20" \
  -e TEST_DURATION="10m" \
  cbaugus/rust-loadtester:latest
```

### Simulating Daily Traffic Patterns
```bash
docker run --rm \
  -e TARGET_URL="https://your-service.com/api" \
  -e LOAD_MODEL_TYPE="DailyTraffic" \
  -e DAILY_MIN_RPS="10" \
  -e DAILY_MID_RPS="200" \
  -e DAILY_MAX_RPS="1000" \
  -e DAILY_CYCLE_DURATION="24h" \
  -e NUM_CONCURRENT_TASKS="200" \
  -e TEST_DURATION="48h" \
  cbaugus/rust-loadtester:latest-Chainguard
```

### Using Custom Headers (API Keys, Auth Tokens)
```bash
docker run --rm \
  -e TARGET_URL="https://api.example.com/protected" \
  -e CUSTOM_HEADERS="Authorization: Bearer your_token,X-Api-Key:your_key" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="5m" \
  cbaugus/rust-loadtester:latest
```

### Ramp Testing (Gradual Load Increase)
```bash
docker run --rm \
  -e TARGET_URL="https://api.example.com/endpoint" \
  -e LOAD_MODEL_TYPE="RampRps" \
  -e MIN_RPS="50" \
  -e MAX_RPS="500" \
  -e NUM_CONCURRENT_TASKS="150" \
  -e TEST_DURATION="15m" \
  cbaugus/rust-loadtester:latest-Chainguard
```

## Essential Environment Variables

| Variable | Required | Default | Description |
|----------|----------|---------|-------------|
| `TARGET_URL` | Yes | - | Full URL of the endpoint to test |
| `LOAD_MODEL_TYPE` | Yes | - | Load model: `Concurrent`, `Rps`, `RampRps`, or `DailyTraffic` |
| `NUM_CONCURRENT_TASKS` | No | 10 | Maximum concurrent requests |
| `TEST_DURATION` | No | 2h | Test duration (e.g., `10m`, `1h`, `3d`) |
| `REQUEST_TYPE` | No | GET | HTTP method: `GET` or `POST` |
| `SKIP_TLS_VERIFY` | No | false | Skip TLS certificate verification |
| `CLUSTER_NODE_ID` | No | hostname | Node identifier used in metrics labels |
| `CLUSTER_REGION` | No | default | Region label used in metrics and health output |
| `CLUSTER_HEALTH_ADDR` | No | 0.0.0.0:8080 | Bind address for the live control HTTP API |

### Load Model Specific Variables

**RPS Model:**
- `TARGET_RPS`: Target requests per second

**RampRps Model:**
- `MIN_RPS`: Starting/ending RPS
- `MAX_RPS`: Peak RPS
- `RAMP_DURATION`: Total ramp profile duration

**DailyTraffic Model:**
- `DAILY_MIN_RPS`: Base load (night-time)
- `DAILY_MID_RPS`: Mid-level load (afternoon)
- `DAILY_MAX_RPS`: Peak load (morning rush)
- `DAILY_CYCLE_DURATION`: Duration of one full cycle

## Monitoring with Prometheus

Access metrics at `http://<container-host>:9090/metrics`

```bash
# View metrics
curl http://localhost:9090/metrics

# Example metrics available:
# - loadtest_requests_total
# - loadtest_status_codes
# - loadtest_concurrent_requests
```

## mTLS Support

```bash
docker run --rm \
  -v /local/path/to/client.crt:/etc/ssl/certs/client.crt \
  -v /local/path/to/client.key:/etc/ssl/private/client.key \
  -e TARGET_URL="https://secure-api.com/endpoint" \
  -e CLIENT_CERT_PATH="/etc/ssl/certs/client.crt" \
  -e CLIENT_KEY_PATH="/etc/ssl/private/client.key" \
  -e LOAD_MODEL_TYPE="Concurrent" \
  -e NUM_CONCURRENT_TASKS="50" \
  -e TEST_DURATION="10m" \
  cbaugus/rust-loadtester:latest-Chainguard
```

**Note:** Private keys must be in PKCS#8 format. Convert if needed:
```bash
openssl pkcs8 -topk8 -inform PEM -outform PEM -nocrypt \
  -in original.key -out pkcs8.key
```

## Advanced Features

### Custom DNS Resolution
Override DNS for specific testing scenarios:
```bash
-e RESOLVE_TARGET_ADDR="example.com:192.168.1.50:8080"
```

### Header Value Escaping
Escape commas in header values with backslash:
```bash
-e CUSTOM_HEADERS="Keep-Alive:timeout=5\,max=200"
```

### JSON Payloads
Send JSON data with POST requests:
```bash
-e SEND_JSON="true" \
-e JSON_PAYLOAD='{"key":"value","nested":{"data":"here"}}'
```

## Live Control API (port 8080)

Every node exposes a lightweight HTTP API for real-time inspection and reconfiguration.

### GET /health

Returns live node status as JSON:

```json
{
  "node_id": "node-1",
  "region": "us-east",
  "node_state": "running",
  "rps": 1423.7,
  "error_rate_pct": 0.12,
  "workers": 25,
  "memory_mb": 142.3,
  "total_memory_mb": 16384.0,
  "cpu_pct": 14.5,
  "time_remaining_secs": 3542,
  "test_started_at_unix": 1706000000,
  "test_duration_secs": 7200,
  "test_percent_complete": 50.8,
  "current_yaml": "version: \"1.0\"\n..."
}
```

`node_state` values: `"running"` (active test), `"standby"` (test complete, keeping connections warm), `"idle"` (no config).

### POST /config

Submit a YAML config to start or reconfigure the test immediately — no restart required:

```bash
curl -X POST http://<node>:8080/config \
  -H "Content-Type: application/x-yaml" \
  --data-binary @config.yaml
```

**Example YAML config with standby:**

```yaml
version: "1.0"
config:
  baseUrl: "https://your-service.com"
  workers: 50
  duration: "1h"
  timeout: "30s"
load:
  model: "rps"
  target: 500
scenarios:
  - name: "Health check"
    weight: 100
    steps:
      - name: "GET /"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: statusCode
            expected: 200
standby:
  workers: 2
  rps: 0
```

When the test duration expires, nodes automatically transition to `"standby"` state using the `standby:` block (or startup env-var defaults if no `standby:` block is present). Standby workers keep connections warm at low (or zero) RPS until a new `POST /config` arrives.

## Why Choose This Tool?

✅ **High Performance**: Built in Rust for maximum throughput and minimal overhead
✅ **Flexible Load Models**: From simple concurrent loads to complex daily traffic patterns
✅ **Live Reconfiguration**: Update test parameters on the fly via `POST /config` — no restart needed
✅ **Auto Standby**: Nodes automatically switch to warm-standby mode after test completion
✅ **Production Ready**: Chainguard static images with minimal CVEs
✅ **Easy Monitoring**: Built-in Prometheus metrics (port 9090) + live health API (port 8080)
✅ **Secure**: Support for HTTPS, mTLS, and custom authentication
✅ **Container Native**: Optimized for Docker/Kubernetes/Nomad deployments
✅ **Actively Maintained**: Regular updates and security patches

## Support & Documentation

- **GitHub Repository**: [cbaugus/rust_loadtest](https://github.com/cbaugus/rust_loadtest)
- **Issues & Features**: [GitHub Issues](https://github.com/cbaugus/rust_loadtest/issues)
- **Full Documentation**: See the [README.md](https://github.com/cbaugus/rust_loadtest/blob/main/README.md) in the repository

## License

This project is open source. See the repository for license details.

---

**Built with ❤️ in Rust** | Optimized for modern cloud-native deployments