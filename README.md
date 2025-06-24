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

## Project Structure

```
.
├── Cargo.toml
├── src
│   └── main.rs
└── Dockerfile
```


## Running the Load Test

The load testing tool is configured primarily through environment variables passed to the Docker container. You can select different load models by setting the LOAD_MODEL_TYPE environment variable.

### Common Environment Variables

* TARGET_URL (Required): The full URL of the endpoint you want to load test (e.g., http://example.com/api/data or https://secure-api.com/status).
* NUM_CONCURRENT_TASKS (Optional, default: 10): The maximum number of concurrent HTTP requests (worker tasks) that the load generator will attempt to maintain. This acts as a concurrency limit.
* TEST_DURATION (Optional, default: 2h): The total duration for which the load test will run. Accepts values like 10m (10 minutes), 1h (1 hour), 3d (3 days).
* SKIP_TLS_VERIFY (Optional, default: false): Set to "true" to skip TLS/SSL certificate verification for HTTPS endpoints. Use with caution, primarily for testing environments with self-signed certificates.
* CLIENT_CERT_PATH (Optional): Path to the client's PEM-encoded public certificate file for mTLS.
* CLIENT_KEY_PATH (Optional): Path to the client's PEM-encoded PKCS#8 private key file for mTLS. Both `CLIENT_CERT_PATH` and `CLIENT_KEY_PATH` must be provided to enable mTLS.
* RESOLVE_TARGET_ADDR (Optional): Allows overriding DNS resolution for the `TARGET_URL`. The format is `"hostname:ip_address:port"`. For example, if `TARGET_URL` is `http://example.com/api` and `RESOLVE_TARGET_ADDR` is set to `"example.com:192.168.1.50:8080"`, all requests to `example.com` will be directed to `192.168.1.50` on port `8080`. This is useful for targeting services not in DNS or for specific routing during tests.

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
