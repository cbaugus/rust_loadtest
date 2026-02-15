# Environment Variable Overrides

## Overview

rust-loadtest supports environment variable overrides for YAML configuration values, enabling flexible configuration management across different environments (development, CI/CD, production) without modifying config files.

## Precedence Order

Configuration values are resolved in the following order (highest to lowest priority):

1. **Environment Variables** (Highest Priority)
2. **YAML Configuration File**
3. **Default Values** (Lowest Priority)

## Environment Variable Mapping

### Global Configuration

| Environment Variable | YAML Path | Description | Example |
|---------------------|-----------|-------------|---------|
| `TARGET_URL` | `config.baseUrl` | Base URL for requests | `https://api.example.com` |
| `NUM_CONCURRENT_TASKS` | `config.workers` | Number of concurrent workers | `100` |
| `REQUEST_TIMEOUT` | `config.timeout` | Request timeout duration | `60s`, `5m` |
| `TEST_DURATION` | `config.duration` | Total test duration | `30m`, `2h` |
| `SKIP_TLS_VERIFY` | `config.skipTlsVerify` | Skip TLS certificate verification | `true`, `false` |
| `CUSTOM_HEADERS` | `config.customHeaders` | Custom HTTP headers | `Authorization:Bearer token` |

### Load Model Configuration

#### Concurrent Model
No environment variables (model has no parameters).

#### RPS Model
| Environment Variable | YAML Path | Description | Example |
|---------------------|-----------|-------------|---------|
| `TARGET_RPS` | `load.target` | Target requests per second | `500` |

#### Ramp Model
| Environment Variable | YAML Path | Description | Example |
|---------------------|-----------|-------------|---------|
| `MIN_RPS` | `load.min` | Starting RPS | `10` |
| `MAX_RPS` | `load.max` | Maximum RPS | `1000` |
| `RAMP_DURATION` | `load.rampDuration` | Ramp-up duration | `5m`, `30s` |

#### Daily Traffic Model
| Environment Variable | YAML Path | Description | Example |
|---------------------|-----------|-------------|---------|
| `DAILY_MIN_RPS` | `load.min` | Minimum RPS (night) | `10` |
| `DAILY_MID_RPS` | `load.mid` | Midday RPS | `50` |
| `DAILY_MAX_RPS` | `load.max` | Peak RPS | `100` |
| `DAILY_CYCLE_DURATION` | `load.cycleDuration` | Full cycle duration | `1d`, `24h` |

#### Complete Load Model Override
| Environment Variable | Description | Example |
|---------------------|-------------|---------|
| `LOAD_MODEL_TYPE` | Completely override load model | `Concurrent`, `Rps`, `RampRps`, `DailyTraffic` |

When `LOAD_MODEL_TYPE` is set, the entire load model from YAML is replaced with the environment variable configuration.

## Usage Examples

### Example 1: Override Workers and Duration

**YAML Config (test.yaml):**
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  workers: 10
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "API Test"
    steps:
      - request:
          method: "GET"
          path: "/health"
```

**Run with overrides:**
```bash
NUM_CONCURRENT_TASKS=50 TEST_DURATION=30m rust-loadtest --config test.yaml
```

**Result:**
- `workers`: 50 (from ENV, overrides YAML's 10)
- `duration`: 30m (from ENV, overrides YAML's 5m)
- `baseUrl`: https://api.example.com (from YAML)

### Example 2: Override RPS Target

**YAML Config:**
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "10m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "Load Test"
    steps:
      - request:
          method: "POST"
          path: "/api/data"
```

**Run with override:**
```bash
TARGET_RPS=500 rust-loadtest --config loadtest.yaml
```

**Result:**
- `load.target`: 500 (from ENV, overrides YAML's 100)
- All other values from YAML

### Example 3: Complete Load Model Override

**YAML Config:**
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "10m"
load:
  model: "concurrent"  # Will be completely replaced
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
```

**Run with complete override:**
```bash
LOAD_MODEL_TYPE=Rps TARGET_RPS=200 rust-loadtest --config test.yaml
```

**Result:**
- Load model: RPS with target 200 (from ENV, replaces YAML's concurrent model)

### Example 4: Multiple Overrides in CI/CD

**YAML Config (base.yaml):**
```yaml
version: "1.0"
config:
  baseUrl: "https://staging.example.com"
  workers: 20
  timeout: "30s"
  duration: "5m"
  skipTlsVerify: false
load:
  model: "ramp"
  min: 10
  max: 100
  rampDuration: "2m"
scenarios:
  - name: "Integration Test"
    steps:
      - request:
          method: "GET"
          path: "/api/v1/health"
```

**Production CI/CD run:**
```bash
TARGET_URL=https://prod.example.com \
NUM_CONCURRENT_TASKS=100 \
TEST_DURATION=30m \
MIN_RPS=50 \
MAX_RPS=1000 \
RAMP_DURATION=10m \
rust-loadtest --config base.yaml
```

**Result:**
- `baseUrl`: https://prod.example.com (ENV override)
- `workers`: 100 (ENV override)
- `duration`: 30m (ENV override)
- `load.min`: 50 (ENV override)
- `load.max`: 1000 (ENV override)
- `load.rampDuration`: 10m (ENV override)
- `timeout`: 30s (from YAML)
- `skipTlsVerify`: false (from YAML)

## Best Practices

### 1. Version Control YAML, Override with Environment

**✅ Recommended:**
- Keep base configuration in version-controlled YAML files
- Use environment variables for environment-specific values
- Document required environment variables in README

**❌ Avoid:**
- Hardcoding environment-specific values in YAML
- Creating separate YAML files for each environment

### 2. Use Environment Variables for Secrets

**✅ Recommended:**
```bash
# Keep secrets out of YAML files
CUSTOM_HEADERS="Authorization:Bearer ${API_TOKEN}" \
rust-loadtest --config test.yaml
```

**❌ Avoid:**
```yaml
# Don't hardcode secrets in YAML
config:
  customHeaders: "Authorization:Bearer hardcoded-secret-123"
```

### 3. Document Environment Variables

Include a `.env.example` file in your repository:

```bash
# .env.example
# Load Test Configuration Overrides

# Target URL (overrides config.baseUrl)
TARGET_URL=https://api.example.com

# Workers (overrides config.workers)
NUM_CONCURRENT_TASKS=50

# Test Duration (overrides config.duration)
TEST_DURATION=10m

# Load Model
LOAD_MODEL_TYPE=Rps
TARGET_RPS=200
```

### 4. Use CI/CD Pipeline Variables

**GitHub Actions Example:**
```yaml
name: Load Test

on: [push]

jobs:
  loadtest:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - name: Run Load Test
        env:
          TARGET_URL: ${{ secrets.PROD_API_URL }}
          NUM_CONCURRENT_TASKS: 100
          TEST_DURATION: 30m
          TARGET_RPS: ${{ vars.TARGET_RPS }}
        run: |
          rust-loadtest --config loadtest.yaml
```

### 5. Validate Configuration

Always validate your final configuration before running long tests:

```bash
# Set env vars
export NUM_CONCURRENT_TASKS=100
export TEST_DURATION=2h
export TARGET_RPS=500

# Do a short dry run first
TEST_DURATION=10s rust-loadtest --config test.yaml

# If successful, run full test
rust-loadtest --config test.yaml
```

## Fallback Behavior

### Invalid Environment Variable Values

If an environment variable contains an invalid value, the system falls back to the YAML value or default:

```bash
# Invalid worker count
NUM_CONCURRENT_TASKS=invalid rust-loadtest --config test.yaml
# → Falls back to YAML config.workers value
```

### Empty Environment Variables

Empty environment variables are treated as unset and fall back to YAML:

```bash
# Empty target URL
TARGET_URL="" rust-loadtest --config test.yaml
# → Falls back to YAML config.baseUrl value
```

## Duration Format

Duration values support multiple formats:
- Seconds: `30s`, `120s`
- Minutes: `5m`, `30m`
- Hours: `2h`, `24h`
- Days: `1d`, `7d`
- Raw seconds: `300` (interpreted as seconds)

## Boolean Values

Boolean environment variables are case-insensitive:
- True: `true`, `TRUE`, `True`, `1`
- False: `false`, `FALSE`, `False`, `0`

## Debugging

### Print Effective Configuration

To see which values are being used:

```bash
# Enable debug logging
RUST_LOG=debug NUM_CONCURRENT_TASKS=100 rust-loadtest --config test.yaml
```

### Test Precedence

1. Load YAML config without env vars
2. Add one env var at a time
3. Verify each override takes effect

## Related Documentation

- [Configuration Precedence](/docs/CONFIGURATION_PRECEDENCE.md)
- [YAML Configuration Guide](/docs/YAML_CONFIG.md)
- [Default Values Reference](/docs/DEFAULTS.md)

## Support

If environment variable overrides aren't working as expected:

1. Check environment variable spelling (case-sensitive)
2. Verify YAML path matches the override documentation
3. Enable debug logging: `RUST_LOG=debug`
4. Check for typos in duration formats (e.g., `30m` not `30min`)
