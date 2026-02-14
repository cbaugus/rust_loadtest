# Docker Guide

This guide shows how to build and run rust-loadtest using Docker.

## Quick Start

### Option 1: Using Docker Compose (Recommended)

The easiest way to test the load testing tool with a test API:

```bash
# Start test API and run load test
docker-compose up

# Or run in detached mode
docker-compose up -d

# View logs
docker-compose logs -f loadtest

# Stop services
docker-compose down
```

This will:
1. Start an httpbin test API on port 8080
2. Build the rust-loadtest Docker image
3. Run a test load test against the API

### Option 2: Build and Run Manually

```bash
# Build the Docker image
docker build -t rust-loadtest .

# Run with a config file
docker run --rm \
  -v $(pwd)/examples/configs:/app/configs \
  rust-loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml

# Run with environment variable overrides
docker run --rm \
  -e TARGET_URL=https://api.example.com \
  -e NUM_CONCURRENT_TASKS=50 \
  -v $(pwd)/examples/configs:/app/configs \
  rust-loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml
```

## Docker Compose Setup

The `docker-compose.yml` includes:

### Services

1. **test-api** - HTTPBin test API
   - Port: 8080
   - Health checks enabled
   - Used for testing load generation

2. **loadtest** - Rust LoadTest tool
   - Waits for test-api to be healthy
   - Mounts config and data directories
   - Configurable via environment variables

3. **simple-api** - Nginx alternative
   - Port: 8081
   - Simple static file server

## Testing Against Different APIs

### Test Against Docker Compose API

```yaml
# In your config file
config:
  baseUrl: "http://test-api"
  # or
  baseUrl: "http://simple-api"
```

```bash
docker-compose up
```

### Test Against External API

```bash
# Override base URL
docker-compose run \
  -e TARGET_URL=https://api.example.com \
  loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml
```

### Test Against Host Machine API

```yaml
# Use host.docker.internal (Docker Desktop)
config:
  baseUrl: "http://host.docker.internal:3000"
```

```bash
docker-compose run loadtest \
  rust-loadtest --config /app/configs/your-config.yaml
```

## Available Configurations

All example configs are available in the container at `/app/configs/`:

```bash
# Basic API test
docker-compose run loadtest rust-loadtest --config /app/configs/basic-api-test.yaml

# E-commerce scenario
docker-compose run loadtest rust-loadtest --config /app/configs/ecommerce-scenario.yaml

# Stress test
docker-compose run loadtest rust-loadtest --config /app/configs/stress-test.yaml

# Docker-specific test (uses httpbin)
docker-compose run loadtest rust-loadtest --config /app/configs/docker-test.yaml
```

## Custom Configurations

### Mount Your Own Config

```bash
docker run --rm \
  -v /path/to/your/config.yaml:/app/my-config.yaml \
  rust-loadtest \
  rust-loadtest --config /app/my-config.yaml
```

### Using Docker Compose Override

Create `docker-compose.override.yml`:

```yaml
version: '3.8'

services:
  loadtest:
    volumes:
      - ./my-configs:/app/my-configs
    command: ["rust-loadtest", "--config", "/app/my-configs/my-test.yaml"]
```

## Environment Variables

Override configuration values using environment variables:

| Variable | Description | Example |
|----------|-------------|---------|
| `TARGET_URL` | Base URL to test | `https://api.example.com` |
| `NUM_CONCURRENT_TASKS` | Number of workers | `50` |
| `TEST_DURATION` | Test duration | `10m` |
| `TARGET_RPS` | Target RPS | `100` |

Example:

```bash
docker-compose run \
  -e TARGET_URL=https://staging.api.com \
  -e NUM_CONCURRENT_TASKS=100 \
  -e TEST_DURATION=5m \
  loadtest \
  rust-loadtest --config /app/configs/stress-test.yaml
```

## Interactive Mode

Keep the container running for manual testing:

```bash
# Start container in interactive mode
docker-compose run --rm loadtest bash

# Inside container, run tests manually
rust-loadtest --config /app/configs/basic-api-test.yaml
rust-loadtest --config /app/configs/stress-test.yaml

# Exit when done
exit
```

## Saving Results

Mount a volume to save test results:

```bash
docker run --rm \
  -v $(pwd)/results:/app/results \
  -v $(pwd)/examples/configs:/app/configs \
  rust-loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml > /app/results/test-results.log
```

Or with docker-compose:

```yaml
services:
  loadtest:
    volumes:
      - ./results:/app/results
```

## Docker Hub

Pull the pre-built image from Docker Hub:

```bash
# Pull latest version
docker pull cbaugus/rust-loadtest:latest

# Run directly
docker run --rm cbaugus/rust-loadtest:latest rust-loadtest --help
```

## Building for Production

### Optimized Build

```bash
# Build with release optimizations
docker build -t rust-loadtest:prod \
  --build-arg RUST_FLAGS="-C target-cpu=native" \
  .
```

### Multi-Architecture Build

```bash
# Build for multiple platforms
docker buildx build \
  --platform linux/amd64,linux/arm64 \
  -t rust-loadtest:multi-arch \
  .
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
            rust-loadtest \
            rust-loadtest --config /app/configs/basic-api-test.yaml
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
    - docker run --rm rust-loadtest rust-loadtest --config /app/configs/basic-api-test.yaml
```

## Networking

### Docker Network

Create a custom network for testing multiple services:

```bash
# Create network
docker network create loadtest-net

# Start test API
docker run -d --name test-api --network loadtest-net kennethreitz/httpbin

# Run load test
docker run --rm --network loadtest-net \
  -e TARGET_URL=http://test-api \
  rust-loadtest \
  rust-loadtest --config /app/configs/docker-test.yaml
```

## Troubleshooting

### Container Won't Start

```bash
# Check logs
docker-compose logs loadtest

# Check if test-api is healthy
docker-compose ps
```

### Can't Connect to API

```bash
# Test connectivity from loadtest container
docker-compose run loadtest curl http://test-api/status/200

# Check network
docker-compose run loadtest ping test-api
```

### Permission Issues

```bash
# Run as current user
docker-compose run --user $(id -u):$(id -g) loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml
```

### View Container Internals

```bash
# Shell into container
docker-compose run --rm loadtest bash

# Check available configs
ls -la /app/configs/

# Check binary
which rust-loadtest
rust-loadtest --help
```

## Examples

### Test Localhost API

```bash
# Start your API on localhost:3000

# Run load test (Docker Desktop)
docker run --rm \
  -e TARGET_URL=http://host.docker.internal:3000 \
  rust-loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml

# Or on Linux
docker run --rm --network host \
  -e TARGET_URL=http://localhost:3000 \
  rust-loadtest \
  rust-loadtest --config /app/configs/basic-api-test.yaml
```

### Stress Test

```bash
# Run stress test with docker-compose
docker-compose run \
  -e TARGET_URL=https://staging.api.com \
  loadtest \
  rust-loadtest --config /app/configs/stress-test.yaml
```

### Data-Driven Test

```bash
# With custom data files
docker run --rm \
  -v $(pwd)/examples/configs:/app/configs \
  -v $(pwd)/examples/data:/app/data \
  -v $(pwd)/my-data:/app/my-data \
  rust-loadtest \
  rust-loadtest --config /app/configs/data-driven-test.yaml
```

## Performance Tips

1. **Use host network** (Linux only) for better performance:
   ```bash
   docker run --rm --network host rust-loadtest ...
   ```

2. **Increase resources**:
   ```yaml
   services:
     loadtest:
       deploy:
         resources:
           limits:
             cpus: '4'
             memory: 4G
   ```

3. **Disable logging** for high-load tests:
   ```bash
   docker run --rm rust-loadtest ... > /dev/null 2>&1
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

## Additional Resources

- [Docker Documentation](https://docs.docker.com/)
- [Docker Compose Reference](https://docs.docker.com/compose/)
- [HTTPBin API Documentation](https://httpbin.org/)
- [Configuration Examples](./examples/configs/README.md)
