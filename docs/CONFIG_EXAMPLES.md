# Configuration Examples and Templates

## Overview

The `examples/configs/` directory contains production-ready YAML configuration templates for common load testing scenarios. Each template is fully documented, validated, and ready to use.

## Quick Start

```bash
# 1. Browse available templates
ls examples/configs/*.yaml

# 2. Copy a template
cp examples/configs/basic-api-test.yaml my-test.yaml

# 3. Customize for your API
vim my-test.yaml

# 4. Run the test
rust-loadtest --config my-test.yaml
```

## Available Templates

### Template Overview

| Template | Complexity | Workers | Scenarios | Best For |
|----------|-----------|---------|-----------|----------|
| [Basic API](#1-basic-api-test) | ⭐ | 10 | 1 | Simple endpoint testing |
| [E-Commerce](#2-e-commerce-scenario) | ⭐⭐⭐ | 50 | 4 | Multi-step user flows |
| [Stress Test](#3-stress-test) | ⭐⭐⭐⭐ | 200 | 3 | Finding system limits |
| [Data-Driven](#4-data-driven-test) | ⭐⭐ | 20 | 2 | Testing with real data |
| [Authenticated](#5-authenticated-api) | ⭐⭐⭐ | 25 | 3 | Auth flows, tokens |
| [Microservices](#6-microservices-test) | ⭐⭐⭐⭐ | 40 | 4 | Distributed systems |
| [GraphQL](#7-graphql-api) | ⭐⭐⭐ | 30 | 4 | GraphQL APIs |
| [Spike Test](#8-spike-test) | ⭐⭐⭐⭐ | 150 | 3 | Sudden traffic spikes |

## Template Details

### 1. Basic API Test

**File**: `basic-api-test.yaml`

**Purpose**: Simple load test for a single API endpoint.

**Configuration**:
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  workers: 10
  duration: "5m"
load:
  model: "rps"
  target: 100
scenarios:
  - name: "API Health Check"
    steps:
      - request:
          method: "GET"
          path: "/health"
```

**Use Cases**:
- API health monitoring
- Smoke testing
- CI/CD integration
- Getting started with load testing

**Customization**:
```bash
# Change URL
sed -i 's|api.example.com|your-api.com|' basic-api-test.yaml

# Adjust RPS
sed -i 's/target: 100/target: 200/' basic-api-test.yaml

# Quick test with env override
TARGET_URL=https://staging.api.com rust-loadtest --config basic-api-test.yaml
```

---

### 2. E-Commerce Scenario

**File**: `ecommerce-scenario.yaml`

**Purpose**: Realistic e-commerce load test with weighted user flows.

**Traffic Distribution**:
- 60% Browse only (window shoppers)
- 25% Browse + add to cart
- 12% Complete purchase
- 3% Quick browse

**Configuration**:
```yaml
load:
  model: "ramp"
  min: 10
  max: 200
  rampDuration: "5m"

scenarios:
  - name: "Browse Only"
    weight: 60
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        thinkTime: "2s"
```

**Real-World Pattern**:
```
Time    RPS    Browse  Cart  Checkout
0m      10     6       2     1
5m      50     30      13    6
10m     100    60      25    12
15m     200    120     50    24
```

**Use Cases**:
- E-commerce platforms
- Conversion funnel testing
- Black Friday simulation
- Realistic user behavior

**Customization**:
- Adjust weights based on your analytics
- Modify product search paths
- Add authentication headers
- Include payment gateway steps

---

### 3. Stress Test

**File**: `stress-test.yaml`

**Purpose**: High-load test to find system breaking points.

**Load Profile**:
```
RPS
1000 |                    ___________
     |                   /
 500 |                  /
     |                 /
  10 |_______________/
     0m    5m    10m   15m    60m
          Ramp         Sustain
```

**Configuration**:
```yaml
config:
  workers: 200
  duration: "1h"
load:
  model: "ramp"
  min: 10
  max: 1000
  rampDuration: "15m"
```

**Metrics to Watch**:
- Response time percentiles (p95, p99)
- Error rate increase
- CPU/memory utilization
- Database connections
- Auto-scaling events

**Use Cases**:
- Capacity planning
- Finding bottlenecks
- Validating auto-scaling
- SLA verification

**Warning**: ⚠️  Generates significant load. Use on test environments only.

---

### 4. Data-Driven Test

**File**: `data-driven-test.yaml`

**Purpose**: Load test using external CSV/JSON data files.

**Data File Setup**:

**CSV** (`users.csv`):
```csv
username,email,user_id
john.doe,john@example.com,1001
jane.smith,jane@example.com,1002
```

**JSON** (`products.json`):
```json
[
  {
    "product_name": "Laptop",
    "category": "electronics",
    "sku": "LAP-001"
  }
]
```

**Configuration**:
```yaml
scenarios:
  - name: "User Login with CSV Data"
    dataFile:
      path: "./examples/data/users.csv"
      format: "csv"
      strategy: "random"  # sequential | random | cycle
    steps:
      - request:
          method: "POST"
          path: "/login"
          body: '{"username": "${username}"}'
```

**Iteration Strategies**:
- **Sequential**: Process data in order (1, 2, 3, ...)
- **Random**: Pick random rows
- **Cycle**: Loop through data (1, 2, 3, 1, 2, 3, ...)

**Use Cases**:
- Testing with real user credentials
- Large dataset testing
- Parameterized API calls
- Database seeding validation

---

### 5. Authenticated API

**File**: `authenticated-api.yaml`

**Purpose**: Test APIs requiring authentication.

**Authentication Patterns**:

**JWT Authentication**:
```yaml
steps:
  - name: "Login"
    request:
      method: "POST"
      path: "/auth/login"
      body: '{"username": "user", "password": "pass"}'
    extract:
      - name: "token"
        jsonPath: "$.token"

  - name: "Use Token"
    request:
      method: "GET"
      path: "/protected"
      headers:
        Authorization: "Bearer ${token}"
```

**API Key**:
```yaml
config:
  customHeaders: "X-API-Key: your-key-here"
```

**OAuth 2.0**:
```yaml
steps:
  - name: "Get Access Token"
    request:
      method: "POST"
      path: "/oauth/token"
      body: '{"grant_type": "client_credentials"}'
    extract:
      - name: "accessToken"
        jsonPath: "$.access_token"
```

**Use Cases**:
- JWT token lifecycle testing
- OAuth flow validation
- API key rate limiting
- Session management

---

### 6. Microservices Test

**File**: `microservices-test.yaml`

**Purpose**: Test distributed microservices architecture.

**Service Distribution**:
- 25% User Service
- 30% Product Service
- 30% Order Service
- 15% Inventory Service

**Configuration**:
```yaml
config:
  baseUrl: "https://gateway.example.com"

scenarios:
  - name: "User Service Flow"
    weight: 25
    steps:
      - request:
          method: "POST"
          path: "/users/register"

  - name: "Product Service Flow"
    weight: 30
    steps:
      - request:
          method: "GET"
          path: "/products"
```

**Testing Patterns**:
- Service-to-service communication
- API gateway performance
- Circuit breaker behavior
- Service mesh metrics

**Use Cases**:
- Microservices platforms
- API gateway testing
- Service mesh validation
- Distributed tracing

---

### 7. GraphQL API

**File**: `graphql-api.yaml`

**Purpose**: Test GraphQL APIs with queries and mutations.

**Query Types**:

**Simple Query**:
```yaml
steps:
  - request:
      method: "POST"
      path: "/graphql"
      body: >
        {
          "query": "query { users { id name } }"
        }
```

**Query with Variables**:
```yaml
steps:
  - request:
      method: "POST"
      path: "/graphql"
      body: >
        {
          "query": "query GetUser($id: ID!) { user(id: $id) { name } }",
          "variables": {"id": "${userId}"}
        }
```

**Mutation**:
```yaml
steps:
  - request:
      method: "POST"
      path: "/graphql"
      body: >
        {
          "query": "mutation { createPost(input: {title: \"Test\"}) { id } }"
        }
```

**Use Cases**:
- GraphQL API testing
- Query complexity validation
- Schema performance
- Resolver optimization

---

### 8. Spike Test

**File**: `spike-test.yaml`

**Purpose**: Test system resilience under sudden traffic spikes.

**Spike Pattern**:
```
Workers
150 |       ████████
    |       ████████
 50 |       ████████
    |       ████████
 20 |██████         ████████
    0   5m  10m  15m  20m  25m
    Normal Spike  Recovery
```

**Configuration**:
```yaml
config:
  workers: 150  # High for spike
  duration: "30m"

scenarios:
  - name: "High-Traffic Endpoint"
    thinkTime:
      min: "100ms"
      max: "500ms"  # Short think time = aggressive
```

**Execution Plan**:
1. **Phase 1** (0-5m): Normal load - 20 workers
2. **Phase 2** (5-10m): Spike - 150 workers
3. **Phase 3** (10-20m): Recovery - 20 workers
4. **Phase 4** (20-30m): Validation - 20 workers

**Use Cases**:
- Flash sale simulation
- Viral content scenarios
- Auto-scaling validation
- Traffic surge preparation

**Implementation**:
```bash
# Manual spike test
rust-loadtest --config spike-test.yaml --workers 20 &
sleep 300
rust-loadtest --config spike-test.yaml --workers 150 &
sleep 300
rust-loadtest --config spike-test.yaml --workers 20
```

---

## Customization Guide

### Common Patterns

#### Change Base URL

**Option 1: Edit File**
```yaml
config:
  baseUrl: "https://your-api.com"
```

**Option 2: Environment Variable**
```bash
TARGET_URL=https://your-api.com rust-loadtest --config template.yaml
```

#### Adjust Load

**RPS Model**:
```yaml
load:
  model: "rps"
  target: 200  # Requests per second
```

**Ramp Model**:
```yaml
load:
  model: "ramp"
  min: 10
  max: 500
  rampDuration: "10m"
```

**Concurrent Model**:
```yaml
load:
  model: "concurrent"
config:
  workers: 100  # Concurrent users
```

#### Add Authentication

**JWT**:
```yaml
steps:
  - name: "Login"
    extract:
      - name: "token"
        jsonPath: "$.token"

  - name: "Protected Request"
    request:
      headers:
        Authorization: "Bearer ${token}"
```

**API Key**:
```yaml
config:
  customHeaders: "X-API-Key: ${API_KEY}"
```

#### Adjust Think Time

**Fixed**:
```yaml
thinkTime: "3s"
```

**Random**:
```yaml
thinkTime:
  min: "1s"
  max: "5s"
```

### Advanced Customization

#### Scenario Weighting

Based on production analytics:

```yaml
scenarios:
  - name: "Browse"
    weight: 70  # 70% of users browse

  - name: "Purchase"
    weight: 30  # 30% of users buy
```

#### Data Extraction

```yaml
extract:
  - name: "userId"
    jsonPath: "$.user.id"

  - name: "token"
    jsonPath: "$.auth.token"

  - name: "productId"
    regex: '"id":"([^"]+)"'
```

#### Custom Assertions

```yaml
assertions:
  - statusCode: 200
  - responseTime: "2s"
  - bodyContains: "success"
  - jsonPath:
      path: "$.status"
      expected: "ok"
  - headerExists: "X-Request-ID"
```

## Environment Variable Overrides

All templates support environment variable overrides:

```bash
# Override URL
TARGET_URL=https://staging.api.com

# Override workers
NUM_CONCURRENT_TASKS=50

# Override duration
TEST_DURATION=10m

# Override RPS
TARGET_RPS=200

# Run with overrides
env TARGET_URL=https://staging.api.com \
    NUM_CONCURRENT_TASKS=50 \
    rust-loadtest --config template.yaml
```

## Validation

Validate templates before running:

```bash
# Validate syntax and schema
rust-loadtest --config template.yaml --validate

# Dry run (parse without executing)
rust-loadtest --config template.yaml --dry-run
```

## CI/CD Integration

### GitHub Actions

```yaml
name: Load Test

on:
  schedule:
    - cron: '0 2 * * *'  # Daily at 2 AM

jobs:
  load-test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2

      - name: Run Load Test
        run: |
          rust-loadtest --config examples/configs/basic-api-test.yaml
        env:
          TARGET_URL: ${{ secrets.API_URL }}

      - name: Upload Results
        uses: actions/upload-artifact@v2
        with:
          name: load-test-results
          path: results/
```

### GitLab CI

```yaml
load-test:
  stage: test
  script:
    - rust-loadtest --config examples/configs/stress-test.yaml
  variables:
    TARGET_URL: $STAGING_API_URL
  artifacts:
    paths:
      - results/
  only:
    - schedules
```

## Best Practices

### 1. Start Small

Begin with basic templates and gradually increase complexity:

```
basic-api-test.yaml
    ↓
ecommerce-scenario.yaml (multi-step)
    ↓
stress-test.yaml (high load)
```

### 2. Use Realistic Data

```yaml
# ❌ Don't use dummy data
body: '{"user": "test123"}'

# ✅ Use realistic data from files
dataFile:
  path: "./real-users.csv"
  strategy: "random"
```

### 3. Monitor System Metrics

While running tests, monitor:
- CPU and memory usage
- Database connections
- Network I/O
- Error rates
- Response time percentiles

### 4. Validate Results

```bash
# Run test
rust-loadtest --config template.yaml > results.log

# Check results
grep "Success Rate" results.log
grep "p95" results.log
grep "p99" results.log
```

### 5. Document Customizations

```yaml
# Added by: John Doe
# Date: 2024-01-01
# Reason: Increased load for Black Friday
config:
  workers: 200  # Was: 50
```

## Troubleshooting

### Template Won't Load

```bash
# Check syntax
rust-loadtest --config template.yaml --validate

# Common issues:
# - Invalid YAML indentation
# - Missing required fields
# - Invalid URL format
```

### High Error Rates

```yaml
# Increase timeout
config:
  timeout: "60s"  # Was: 30s

# Add retry logic (if supported)
config:
  retryCount: 3
```

### Data File Not Found

```yaml
# Use absolute path
dataFile:
  path: "/full/path/to/data.csv"

# Or relative to working directory
dataFile:
  path: "./data/users.csv"
```

## Related Documentation

- [YAML Configuration Guide](/docs/YAML_CONFIG.md)
- [Scenario Definitions](/docs/SCENARIO_YAML.md)
- [Load Models](/docs/LOAD_MODELS.md)
- [Multi-Scenario Execution](/docs/MULTI_SCENARIO.md)
- [Configuration Hot-Reload](/docs/CONFIG_HOT_RELOAD.md)

## Contributing Templates

To contribute a new template:

1. Create YAML file in `examples/configs/`
2. Add comprehensive comments
3. Include usage examples
4. Add validation test in `tests/config_examples_tests.rs`
5. Update `examples/configs/README.md`
6. Submit pull request

## Support

- **Issues**: Report problems on GitHub
- **Questions**: Ask in Discussions
- **Examples**: Check `/examples` directory
- **Documentation**: See `/docs` directory
