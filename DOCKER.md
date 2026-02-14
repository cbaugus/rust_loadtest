# Docker Guide

This guide shows how to build and run rust-loadtest using Docker.

## Important Note

**The CLI currently uses environment variables only.** YAML config file support (`--config` flag) exists in the library but is not yet integrated into the main binary. All examples below use environment variables.

### Current Limitations

- **No CLI argument parsing**: The `--config` flag is not implemented yet
- **Single endpoint testing**: Can only test one URL at a time (no multi-scenario support yet)
- **Basic request types**: Supports simple GET/POST requests with optional JSON payload
- **Environment-based config**: All configuration must be passed via environment variables

### Future Enhancements

- CLI argument parsing with `--config` flag support
- Multi-scenario testing from YAML configuration files
- Advanced features: headers, authentication, data-driven tests
- Interactive CLI mode

## Quick Start

### Option 1: Test Against Your API

```bash
# Build the Docker image
docker build -t rust-loadtest .

# Run against your API (GET request)
docker run --rm \
  -e TARGET_URL=https://api.example.com/endpoint \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=10 \
  -e TEST_DURATION=5m \
  rust-loadtest

# Run against your API (POST with JSON)
docker run --rm \
  -e TARGET_URL=https://api.example.com/endpoint \
  -e REQUEST_TYPE=POST \
  -e SEND_JSON=true \
  -e JSON_PAYLOAD='{"key":"value"}' \
  -e NUM_CONCURRENT_TASKS=10 \
  -e TEST_DURATION=5m \
  rust-loadtest
```

### Option 2: Using Docker Compose with Test API

Test against the included httpbin test API:

```bash
# Start test API
docker-compose up -d test-api

# Run load test against it
docker run --rm --network rust_loadtest_default \
  -e TARGET_URL=http://test-api/status/200 \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=5 \
  -e TEST_DURATION=1m \
  rust-loadtest

# Stop services
docker-compose down
```

## Configuration via Environment Variables

The tool is configured entirely through environment variables. Here are the key variables:

| Variable | Description | Example | Default |
|----------|-------------|---------|---------|
| `TARGET_URL` | Base URL to test (required) | `https://api.example.com` | - |
| `REQUEST_TYPE` | HTTP method | `GET`, `POST`, `PUT`, `DELETE` | `POST` |
| `NUM_CONCURRENT_TASKS` | Number of workers | `50` | `10` |
| `TEST_DURATION` | Test duration | `10m`, `1h`, `2h` | `2h` |
| `SEND_JSON` | Send JSON payload | `true`, `false` | `false` |
| `JSON_PAYLOAD` | JSON body for POST/PUT | `{"key":"value"}` | - |
| `TARGET_RPS` | Target requests per second | `100` | - |
| `LOAD_MODEL_TYPE` | Load model | `Concurrent`, `Rps`, `RampRps` | `Concurrent` |
| `SKIP_TLS_VERIFY` | Skip TLS verification | `true`, `false` | `false` |

**Important:** If your endpoint expects GET requests, you must set `REQUEST_TYPE=GET` (the default is POST).

Example:

```bash
docker run --rm \
  -e TARGET_URL=https://api.example.com/endpoint \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=100 \
  -e TEST_DURATION=5m \
  rust-loadtest
```

## Accessing Metrics

The tool exposes Prometheus metrics on port 9090. Map the port to access them:

```bash
docker run --rm \
  -p 9090:9090 \
  -e TARGET_URL=https://api.example.com \
  -e REQUEST_TYPE=GET \
  rust-loadtest

# In another terminal, access metrics
curl http://localhost:9090/metrics
```

## Saving Results

Redirect output to save test results:

```bash
docker run --rm \
  -e TARGET_URL=https://api.example.com \
  -e REQUEST_TYPE=GET \
  -e TEST_DURATION=5m \
  rust-loadtest > test-results.log 2>&1
```

## Docker Hub

Pull the pre-built image from Docker Hub:

```bash
# Pull latest version
docker pull cbaugus/rust-loadtest:latest

# Run directly
docker run --rm cbaugus/rust-loadtest:latest rust-loadtest --help
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Load Test

on:
  push:
    branches: [ main ]

jobs:
  load-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Build Docker image
        run: docker build -t rust-loadtest .

      - name: Run load test
        run: |
          docker run --rm \
            -e TARGET_URL=${{ secrets.API_URL }} \
            -e REQUEST_TYPE=GET \
            -e NUM_CONCURRENT_TASKS=10 \
            -e TEST_DURATION=5m \
            rust-loadtest
```

### GitLab CI

```yaml
load-test:
  stage: test
  image: docker:latest
  services:
    - docker:dind
  script:
    - docker build -t rust-loadtest .
    - docker run --rm
        -e TARGET_URL=${API_URL}
        -e REQUEST_TYPE=GET
        -e NUM_CONCURRENT_TASKS=10
        -e TEST_DURATION=5m
        rust-loadtest
```

### Jenkins Pipeline

```groovy
pipeline {
    agent any
    stages {
        stage('Build') {
            steps {
                sh 'docker build -t rust-loadtest .'
            }
        }
        stage('Load Test') {
            steps {
                sh '''
                    docker run --rm \
                      -e TARGET_URL=${API_URL} \
                      -e REQUEST_TYPE=GET \
                      -e NUM_CONCURRENT_TASKS=50 \
                      -e TEST_DURATION=10m \
                      rust-loadtest
                '''
            }
        }
    }
}
```

## Networking

### Testing Against Docker Compose Services

```bash
# Start your services with docker-compose
docker-compose up -d

# Run load test on the same network
docker run --rm --network rust_loadtest_default \
  -e TARGET_URL=http://your-service:8080/api \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=10 \
  rust-loadtest
```

### Custom Docker Network

Create a custom network for testing multiple services:

```bash
# Create network
docker network create loadtest-net

# Start test API
docker run -d --name test-api --network loadtest-net kennethreitz/httpbin

# Run load test
docker run --rm --network loadtest-net \
  -e TARGET_URL=http://test-api/status/200 \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=5 \
  rust-loadtest
```

## Troubleshooting

### Getting 405 Method Not Allowed Errors

If you see `status_code="405"` in the metrics but can curl your endpoint successfully:

**Problem:** The default REQUEST_TYPE is POST, but your endpoint expects GET.

**Solution:** Add `-e REQUEST_TYPE=GET` to your docker run command:

```bash
docker run --rm \
  -e TARGET_URL=http://192.168.2.22:8081/health \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=10 \
  rust-loadtest
```

### Missing TARGET_URL Error

If you see "Missing required environment variable: TARGET_URL":

**Solution:** Make sure you're setting the TARGET_URL environment variable:

```bash
docker run --rm \
  -e TARGET_URL=https://your-api.com \
  -e REQUEST_TYPE=GET \
  rust-loadtest
```

### Can't Connect to API on Host Machine

**For Docker Desktop (Mac/Windows):**
```bash
# Use host.docker.internal to reach host machine
docker run --rm \
  -e TARGET_URL=http://host.docker.internal:3000 \
  -e REQUEST_TYPE=GET \
  rust-loadtest
```

**For Linux:**
```bash
# Use --network host
docker run --rm --network host \
  -e TARGET_URL=http://localhost:3000 \
  -e REQUEST_TYPE=GET \
  rust-loadtest
```

### View Container Internals

```bash
# Shell into container
docker run --rm -it rust-loadtest bash

# Check binary
which rust-loadtest
rust-loadtest # Shows help/error with env var requirements
```

## Examples

### Basic GET Request Test

```bash
docker run --rm \
  -e TARGET_URL=https://api.example.com/users \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=10 \
  -e TEST_DURATION=5m \
  rust-loadtest
```

### POST Request with JSON

```bash
docker run --rm \
  -e TARGET_URL=https://api.example.com/users \
  -e REQUEST_TYPE=POST \
  -e SEND_JSON=true \
  -e JSON_PAYLOAD='{"name":"test","email":"test@example.com"}' \
  -e NUM_CONCURRENT_TASKS=10 \
  -e TEST_DURATION=5m \
  rust-loadtest
```

### High-Concurrency Stress Test

```bash
docker run --rm \
  -e TARGET_URL=https://staging.api.com \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=200 \
  -e TEST_DURATION=10m \
  -e LOAD_MODEL_TYPE=Rps \
  -e TARGET_RPS=1000 \
  rust-loadtest
```

### Test Against Local API (Docker Desktop)

```bash
# Start your API on localhost:3000, then:
docker run --rm \
  -e TARGET_URL=http://host.docker.internal:3000/api/health \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=5 \
  -e TEST_DURATION=2m \
  rust-loadtest
```

### Test Against Local API (Linux)

```bash
docker run --rm --network host \
  -e TARGET_URL=http://localhost:3000/api/health \
  -e REQUEST_TYPE=GET \
  -e NUM_CONCURRENT_TASKS=5 \
  -e TEST_DURATION=2m \
  rust-loadtest
```

### Ramp Load Test

```bash
docker run --rm \
  -e TARGET_URL=https://api.example.com \
  -e REQUEST_TYPE=GET \
  -e LOAD_MODEL_TYPE=RampRps \
  -e MIN_RPS=10 \
  -e MAX_RPS=1000 \
  -e RAMP_DURATION=10m \
  -e NUM_CONCURRENT_TASKS=100 \
  rust-loadtest
```

## Performance Tips

1. **Use host network** (Linux only) for better performance:
   ```bash
   docker run --rm --network host \
     -e TARGET_URL=http://localhost:3000 \
     -e REQUEST_TYPE=GET \
     rust-loadtest
   ```

2. **Increase resources** with docker run:
   ```bash
   docker run --rm \
     --cpus="4" \
     --memory="4g" \
     -e TARGET_URL=https://api.example.com \
     -e REQUEST_TYPE=GET \
     -e NUM_CONCURRENT_TASKS=200 \
     rust-loadtest
   ```

3. **Reduce log verbosity** for high-load tests:
   ```bash
   docker run --rm \
     -e RUST_LOG=error \
     -e TARGET_URL=https://api.example.com \
     -e REQUEST_TYPE=GET \
     -e NUM_CONCURRENT_TASKS=500 \
     rust-loadtest
   ```

4. **Monitor metrics** during the test:
   ```bash
   # Terminal 1: Run test with metrics exposed
   docker run --rm -p 9090:9090 \
     -e TARGET_URL=https://api.example.com \
     -e REQUEST_TYPE=GET \
     rust-loadtest

   # Terminal 2: Watch metrics
   watch -n 1 'curl -s http://localhost:9090/metrics | grep rust_loadtest_requests_total'
   ```

## Security

### Running as Non-Root

Update Dockerfile:

```dockerfile
# Add user
RUN useradd -m -u 1000 loadtest

# Change ownership
RUN chown -R loadtest:loadtest /app

# Switch to user
USER loadtest
```

### Scanning for Vulnerabilities

```bash
# Scan image
docker scan rust-loadtest

# Or use trivy
trivy image rust-loadtest
```

## Maintenance

### Update Dependencies

```bash
# Rebuild with latest dependencies
docker build --no-cache -t rust-loadtest .
```

### Cleanup

```bash
# Remove old images
docker image prune -a

# Remove all related containers
docker-compose down -v --remove-orphans
```

## Quick Reference

### Common Commands

```bash
# Basic GET test
docker run --rm -e TARGET_URL=<url> -e REQUEST_TYPE=GET rust-loadtest

# POST with JSON
docker run --rm -e TARGET_URL=<url> -e REQUEST_TYPE=POST -e SEND_JSON=true -e JSON_PAYLOAD='<json>' rust-loadtest

# With metrics exposed
docker run --rm -p 9090:9090 -e TARGET_URL=<url> -e REQUEST_TYPE=GET rust-loadtest

# High concurrency
docker run --rm -e TARGET_URL=<url> -e REQUEST_TYPE=GET -e NUM_CONCURRENT_TASKS=100 rust-loadtest

# Custom duration
docker run --rm -e TARGET_URL=<url> -e REQUEST_TYPE=GET -e TEST_DURATION=10m rust-loadtest

# Against localhost (Docker Desktop)
docker run --rm -e TARGET_URL=http://host.docker.internal:3000 -e REQUEST_TYPE=GET rust-loadtest

# Against localhost (Linux)
docker run --rm --network host -e TARGET_URL=http://localhost:3000 -e REQUEST_TYPE=GET rust-loadtest
```

### Available Load Models

- **Concurrent**: Constant concurrent requests (default)
- **Rps**: Target specific requests per second
  - Requires: `LOAD_MODEL_TYPE=Rps`, `TARGET_RPS=<number>`
- **RampRps**: Gradually increase RPS
  - Requires: `LOAD_MODEL_TYPE=RampRps`, `MIN_RPS=<number>`, `MAX_RPS=<number>`, `RAMP_DURATION=<duration>`

## Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [HTTPBin API Documentation](https://httpbin.org/)
- [Prometheus Metrics](https://prometheus.io/docs/introduction/overview/)
