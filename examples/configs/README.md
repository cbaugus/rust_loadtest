# Load Test Configuration Examples

This directory contains ready-to-use YAML configuration templates for common load testing scenarios. Each template is fully documented and can be used as-is or customized for your specific needs.

## Available Templates

### 1. Basic API Test (`basic-api-test.yaml`)

**Purpose**: Simple load test for a single API endpoint

**Use Cases**:
- API health checks
- Simple endpoint testing
- Getting started with load testing
- Smoke testing

**Key Features**:
- Single endpoint testing
- RPS load model (100 RPS)
- Basic assertions (status code, response time)
- 5-minute duration

**Quick Start**:
```bash
# Edit the baseUrl in the file
vim basic-api-test.yaml

# Run the test
rust-loadtest --config basic-api-test.yaml
```

**Customize**:
- `baseUrl`: Change to your API endpoint
- `workers`: Adjust for desired concurrency
- `target`: Modify target RPS
- `duration`: Change test duration

---

### 2. E-Commerce Scenario (`ecommerce-scenario.yaml`)

**Purpose**: Realistic e-commerce load test with multiple user flows

**Use Cases**:
- E-commerce platforms
- Multi-step user journeys
- Realistic traffic simulation
- Conversion funnel testing

**Key Features**:
- 4 weighted scenarios (browse, add to cart, checkout, quick browse)
- Variable think times
- Data extraction (product IDs, prices)
- Realistic user behavior patterns

**Traffic Distribution**:
- 60% Browse only
- 25% Browse and add to cart
- 12% Complete purchase
- 3% Quick browse

**Quick Start**:
```bash
rust-loadtest --config ecommerce-scenario.yaml
```

**Customize**:
- Adjust scenario weights to match your traffic
- Modify think times for your user behavior
- Update product search/checkout paths
- Add authentication if needed

---

### 3. Stress Test (`stress-test.yaml`)

**Purpose**: High-load stress test to find system breaking points

**Use Cases**:
- Capacity planning
- Finding system limits
- Performance bottleneck identification
- Auto-scaling validation

**Key Features**:
- Ramp load model (10 → 1000 RPS)
- High worker count (200)
- Long duration (1 hour)
- Mixed read/write operations

**Load Profile**:
- Start: 10 RPS
- End: 1000 RPS
- Ramp: 15 minutes
- Sustain: 45 minutes

**Quick Start**:
```bash
# ⚠️  Warning: This generates significant load
rust-loadtest --config stress-test.yaml
```

**Customize**:
- `max`: Adjust maximum RPS based on your system
- `rampDuration`: Change ramp speed (gradual vs rapid)
- `workers`: Scale based on your infrastructure
- `duration`: Extend for longer stress tests

---

### 4. Data-Driven Test (`data-driven-test.yaml`)

**Purpose**: Load test using external CSV/JSON data files

**Use Cases**:
- Testing with realistic user data
- Large dataset testing
- Parameterized load tests
- Credential-based testing

**Key Features**:
- CSV and JSON data file support
- Multiple iteration strategies (sequential, random, cycle)
- Variable substitution in requests
- Separate scenarios for each data source

**Data File Examples**:

**CSV** (`examples/data/users.csv`):
```csv
username,email,user_id
john.doe,john@example.com,1001
jane.smith,jane@example.com,1002
```

**JSON** (`examples/data/products.json`):
```json
[
  {"product_name": "Laptop", "category": "electronics", "sku": "LAP-001"}
]
```

**Quick Start**:
```bash
# Data files are included in examples/data/
rust-loadtest --config data-driven-test.yaml
```

**Customize**:
- Create your own CSV/JSON files
- Update `dataFile.path` to point to your files
- Change `strategy` (sequential, random, cycle)
- Use data variables in requests: `${variable_name}`

---

### 5. Authenticated API (`authenticated-api.yaml`)

**Purpose**: Load test for APIs requiring authentication

**Use Cases**:
- JWT authentication testing
- API key validation
- OAuth 2.0 flows
- Token refresh testing

**Key Features**:
- JWT authentication flow
- API key authentication
- OAuth token refresh
- Token extraction and reuse

**Authentication Methods**:
- JWT tokens (login → use token)
- API keys (static header)
- OAuth 2.0 (token + refresh)

**Quick Start**:
```bash
# Set credentials
export USERNAME="testuser@example.com"
export PASSWORD="securePassword123"
export API_KEY="your-api-key"

rust-loadtest --config authenticated-api.yaml
```

**Customize**:
- Update authentication endpoints
- Modify token extraction JSONPath
- Add custom auth headers
- Change credentials format

---

### 6. Microservices Test (`microservices-test.yaml`)

**Purpose**: Load test for distributed microservices architecture

**Use Cases**:
- Microservices platforms
- API gateway testing
- Inter-service communication
- Distributed system validation

**Key Features**:
- Multiple service endpoints
- Service-specific scenarios
- Weighted traffic distribution
- End-to-end flows

**Services Tested**:
- User Service (25%)
- Product Service (30%)
- Order Service (30%)
- Inventory Service (15%)

**Quick Start**:
```bash
rust-loadtest --config microservices-test.yaml
```

**Customize**:
- Update service endpoints
- Adjust scenario weights
- Add service-specific assertions
- Modify service interaction flows

---

### 7. GraphQL API (`graphql-api.yaml`)

**Purpose**: Load test for GraphQL APIs

**Use Cases**:
- GraphQL API testing
- Query complexity testing
- Mutation performance
- Schema validation

**Key Features**:
- Simple and complex queries
- Mutations (create, update, delete)
- Query variables
- Nested object fetching

**Operation Types**:
- Simple queries (40%)
- Complex nested queries (25%)
- Mutations (25%)
- Search and filter (10%)

**Quick Start**:
```bash
rust-loadtest --config graphql-api.yaml
```

**Customize**:
- Update GraphQL queries for your schema
- Adjust query complexity
- Modify mutation operations
- Add authentication headers

---

### 8. Spike Test (`spike-test.yaml`)

**Purpose**: Sudden traffic spike test for resilience validation

**Use Cases**:
- Flash sale simulation
- Viral content scenarios
- Auto-scaling response testing
- Traffic surge validation

**Key Features**:
- Sudden load increases
- System recovery observation
- High worker count (150)
- Short think times

**Spike Pattern**:
- Phase 1: Normal load (20 workers)
- Phase 2: Spike (150 workers)
- Phase 3: Recovery (20 workers)
- Phase 4: Validation (20 workers)

**Quick Start**:
```bash
# ⚠️  Warning: Generates sudden load spike
rust-loadtest --config spike-test.yaml
```

**Customize**:
- Adjust spike magnitude
- Modify spike duration
- Add health check endpoints
- Change recovery time

---

## Template Selection Guide

| Template | Complexity | Duration | Workers | RPS | Best For |
|----------|-----------|----------|---------|-----|----------|
| Basic API | Simple | 5m | 10 | 100 | Getting started, simple endpoints |
| E-Commerce | Medium | 30m | 50 | 10-200 | Multi-step flows, realistic behavior |
| Stress Test | High | 1h | 200 | 10-1000 | Finding limits, capacity planning |
| Data-Driven | Medium | 15m | 20 | 50 | Realistic data, parameterized tests |
| Authenticated | Medium | 20m | 25 | 75 | Auth flows, token management |
| Microservices | High | 30m | 40 | 20-150 | Distributed systems, multiple services |
| GraphQL | Medium | 20m | 30 | 80 | GraphQL APIs, complex queries |
| Spike Test | High | 30m | 150 | Burst | Resilience, auto-scaling |

## Customization Guide

### Common Customizations

#### 1. Change Base URL
```yaml
config:
  baseUrl: "https://your-api.example.com"
```

#### 2. Adjust Load
```yaml
# RPS Model
load:
  model: "rps"
  target: 200  # Change target RPS

# Ramp Model
load:
  model: "ramp"
  min: 50      # Start RPS
  max: 500     # End RPS
  rampDuration: "10m"

# Concurrent Model
load:
  model: "concurrent"
config:
  workers: 100  # Number of concurrent workers
```

#### 3. Modify Duration
```yaml
config:
  duration: "30m"  # Options: "30s", "5m", "1h"
```

#### 4. Add Authentication
```yaml
config:
  customHeaders: "Authorization: Bearer your-token-here"

# Or extract from login
steps:
  - name: "Login"
    request:
      method: "POST"
      path: "/auth/login"
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

#### 5. Adjust Think Times
```yaml
# Fixed think time
thinkTime: "3s"

# Random think time
thinkTime:
  min: "1s"
  max: "5s"
```

#### 6. Add Custom Assertions
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

### Environment Variable Overrides

All templates support environment variable overrides:

```bash
# Override base URL
TARGET_URL=https://staging.api.example.com rust-loadtest --config template.yaml

# Override workers
NUM_CONCURRENT_TASKS=50 rust-loadtest --config template.yaml

# Override duration
TEST_DURATION=10m rust-loadtest --config template.yaml

# Override RPS
TARGET_RPS=200 rust-loadtest --config template.yaml
```

## Validation

All templates are validated to ensure:
- ✅ Valid YAML syntax
- ✅ Correct schema structure
- ✅ Valid URLs (example.com placeholders)
- ✅ Valid duration formats
- ✅ Positive worker counts
- ✅ Valid load model parameters

To validate a template:
```bash
rust-loadtest --config template.yaml --validate
```

## Creating Custom Templates

### Template Structure

```yaml
version: "1.0"

metadata:
  name: "Your Test Name"
  description: "Brief description"
  tags: ["tag1", "tag2"]

config:
  baseUrl: "https://api.example.com"
  timeout: "30s"
  workers: 10
  duration: "5m"

load:
  model: "rps"
  target: 100

scenarios:
  - name: "Scenario Name"
    weight: 100
    steps:
      - name: "Step Name"
        request:
          method: "GET"
          path: "/endpoint"
        assertions:
          - statusCode: 200
```

### Best Practices

1. **Use Descriptive Names**: Clear scenario and step names
2. **Add Comments**: Document complex logic
3. **Set Realistic Timeouts**: Based on your SLA
4. **Add Assertions**: Validate responses
5. **Use Think Times**: Simulate real user behavior
6. **Extract Variables**: Reuse data across steps
7. **Weight Scenarios**: Match real traffic patterns

## Data Files

Example data files are provided in `examples/data/`:

- `users.csv` - Sample user data (10 users)
- `products.json` - Sample product data (10 products)

Create your own data files following the same format.

## Getting Help

- **Documentation**: See `/docs/` for detailed guides
- **Examples**: All templates include inline comments
- **Validation**: Use `--validate` flag to check configs
- **Issues**: Report problems on GitHub

## Contributing

To contribute a new template:

1. Create a new YAML file in `examples/configs/`
2. Add comprehensive comments
3. Include usage examples
4. Document customization options
5. Add validation tests
6. Update this README

## Version History

- **v1.0** - Initial template collection (8 templates)
  - Basic API, E-Commerce, Stress Test, Data-Driven
  - Authenticated API, Microservices, GraphQL, Spike Test
