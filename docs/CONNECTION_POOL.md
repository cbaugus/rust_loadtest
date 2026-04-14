# Connection Pool Configuration

This document explains how `rust_loadtest` manages HTTP connections and how to
configure pooling behavior for different test scenarios.

## How Connection Pooling Works

Each load test builds a single `reqwest::Client` that maintains a connection
pool per target host. When a request completes, the underlying TCP connection
(including its TLS session) is returned to the pool. Subsequent requests grab
an existing connection from the pool instead of performing a new TCP handshake
and TLS negotiation.

This is the **default behavior** — no special configuration is needed to reuse
connections.

### When connections are reused

- Workers fire requests continuously (e.g., RPS >= 1)
- Idle connections haven't exceeded the idle timeout
- The pool hasn't reached the max idle limit

### When new connections are created

- First request from each worker (no pooled connection exists yet)
- Idle timeout expired — the pooled connection was closed
- `maxIdlePerHost` is set to 0 — pooling is effectively disabled
- The server closed the connection (e.g., server-side idle timeout)

## Configuration

Pool settings can be configured via **environment variables** (applied at
startup) or via the **YAML config** (applied per-test on `POST /config`).
YAML values override environment variables when present.

### Environment Variables

| Variable                 | Default | Description                                      |
|--------------------------|---------|--------------------------------------------------|
| `POOL_MAX_IDLE_PER_HOST` | `32`    | Maximum idle connections kept per host            |
| `POOL_IDLE_TIMEOUT_SECS` | `30`    | Seconds an idle connection stays in the pool      |
| `TCP_NODELAY`            | `true`  | Disable Nagle's algorithm for lower latency       |
| `REQUEST_TIMEOUT_SECS`   | `30`    | Per-request timeout                               |

### YAML Config

Add an optional `pool` section under `config`:

```yaml
config:
  baseUrl: https://example.com
  pool:
    maxIdlePerHost: 32
    idleTimeoutSecs: 30
```

| Field            | Default | Description                                                          |
|------------------|---------|----------------------------------------------------------------------|
| `maxIdlePerHost` | `32`    | Max idle connections per host. Set to `0` to disable pooling.        |
| `idleTimeoutSecs`| `30`    | Seconds before idle connections are closed. Set to `0` to close immediately. |

## Use Case: Force New Connection Per Request

Use this when you need every request to perform a full TCP + TLS handshake.
Useful for testing:

- TLS handshake latency and overhead
- Server-side connection establishment handling under load
- Certificate validation performance
- Load balancer connection distribution

```yaml
version: "1.0"
config:
  baseUrl: https://api.example.com
  workers: 10
  duration: 5m
  timeout: 30s
  pool:
    maxIdlePerHost: 0
    idleTimeoutSecs: 0
load:
  model: rps
  target: 100
scenarios:
  - name: new-connection-test
    weight: 100
    steps:
      - name: request
        request:
          method: GET
          path: /health
        assertions:
          - type: statusCode
            expected: 200
```

With environment variables:

```bash
POOL_MAX_IDLE_PER_HOST=0 POOL_IDLE_TIMEOUT_SECS=0
```

## Use Case: Reuse Connections (Default)

Use this for standard load testing where you want realistic connection behavior.
Connections are established once and reused across requests, which is how most
production clients behave.

```yaml
version: "1.0"
config:
  baseUrl: https://api.example.com
  workers: 25
  duration: 10m
  timeout: 30s
  # No pool section needed — defaults reuse connections
load:
  model: rps
  target: 1000
scenarios:
  - name: reuse-connection-test
    weight: 100
    steps:
      - name: request
        request:
          method: GET
          path: /health
        assertions:
          - type: statusCode
            expected: 200
```

## Use Case: Long-Lived Connection Reuse with Infrequent Requests

Use this when requests are spaced far apart (e.g., every 5 minutes) but you
want to keep the same TCP/TLS session alive between them. Increase the idle
timeout to prevent the pool from closing connections during gaps.

```yaml
version: "1.0"
config:
  baseUrl: https://api.example.com
  workers: 1
  duration: 1h
  timeout: 30s
  pool:
    maxIdlePerHost: 1
    idleTimeoutSecs: 600
load:
  model: rps
  target: 1
scenarios:
  - name: keepalive-test
    weight: 100
    steps:
      - name: request
        request:
          method: POST
          path: /oauth2/v1/token
          body: "grant_type=client_credentials&client_id=my_id&client_secret=my_secret"
          headers:
            Content-Type: application/x-www-form-urlencoded
        assertions:
          - type: statusCode
            expected: 200
        thinkTime:
          min: 4m
          max: 5m
standby:
  workers: 1
  rps: 1.0
```

**Note:** Even with a high idle timeout, the remote server may close the
connection on its side (common server idle timeouts are 60-120s). The pool
will transparently open a new connection when this happens.

## Monitoring Connection Reuse

Prometheus metrics are available on port 9090. Connection tracking uses
**local TCP port comparison** — each response's local socket address is
checked. A new local port means a new TCP connection was established.
Same port means the connection was reused from the pool. This is
deterministic and accurate at any RPS.

| Metric                              | Type    | Description                              |
|-------------------------------------|---------|------------------------------------------|
| `connection_pool_new_total`         | Counter | Requests that used a new TCP connection  |
| `connection_pool_reused_total`      | Counter | Requests that reused a pooled connection |
| `connection_pool_reuse_rate_percent`| Gauge   | Current reuse percentage                 |
| `connection_pool_requests_total`    | Counter | Total requests tracked                   |
| `connection_pool_max_idle_per_host` | Gauge   | Configured max idle setting              |
| `connection_pool_idle_timeout_seconds`| Gauge | Configured idle timeout setting          |

### Grafana Queries

**New vs reused connections over time (time series panel):**

| Query                                      | Legend | Color |
|--------------------------------------------|--------|-------|
| `rate(connection_pool_reused_total[1m])`   | Reused | Green |
| `rate(connection_pool_new_total[1m])`      | New    | Red   |

**Reuse rate (single stat panel):**

```promql
connection_pool_reuse_rate_percent
```

**Percentage of new connections (single stat panel):**

```promql
connection_pool_new_total / connection_pool_requests_total * 100
```
