# Configuration Schema Reference

Complete reference for rust-loadtest YAML configuration format.

## Table of Contents

- [Version](#version)
- [Metadata](#metadata)
- [Config](#config)
- [Load Models](#load-models)
- [Scenarios](#scenarios)
- [Complete Example](#complete-example)

---

## Version

**Field**: `version` (required)

**Type**: String

**Description**: Configuration version using semantic versioning.

**Format**: `major.minor`

**Example**:
```yaml
version: "1.0"
```

---

## Metadata

**Field**: `metadata` (optional)

**Type**: Object

**Description**: Optional metadata about the test configuration.

**Properties**:

| Property | Type | Description |
|----------|------|-------------|
| `name` | string | Human-readable test name |
| `description` | string | Test description |
| `author` | string | Test author |
| `tags` | array | Tags for categorization |

**Example**:
```yaml
metadata:
  name: "API Load Test"
  description: "Testing API endpoints"
  author: "DevOps Team"
  tags: ["api", "production"]
```

---

## Config

**Field**: `config` (required)

**Type**: Object

**Description**: Global test configuration.

**Properties**:

| Property | Type | Required | Default | Description |
|----------|------|----------|---------|-------------|
| `baseUrl` | string | Yes | - | Base URL of the API |
| `timeout` | string/int | No | `30s` | Request timeout |
| `workers` | integer | No | `10` | Concurrent workers |
| `duration` | string/int | Yes | - | Test duration |
| `skipTlsVerify` | boolean | No | `false` | Skip TLS verification |
| `customHeaders` | string | No | - | Custom HTTP headers |

**Duration Format**: `<number><unit>` where unit is `s` (seconds), `m` (minutes), or `h` (hours)

**Example**:
```yaml
config:
  baseUrl: "https://api.example.com"
  timeout: "30s"
  workers: 50
  duration: "10m"
  skipTlsVerify: false
  customHeaders: "Authorization: Bearer token123"
```

---

## Load Models

**Field**: `load` (required)

**Type**: Object

**Description**: Load generation model.

### Concurrent Model

Fixed number of concurrent workers.

```yaml
load:
  model: "concurrent"
```

### RPS Model

Target requests per second.

```yaml
load:
  model: "rps"
  target: 100  # 100 requests/second
```

### Ramp Model

Gradually increase RPS over time.

```yaml
load:
  model: "ramp"
  min: 10       # Starting RPS
  max: 500      # Ending RPS
  rampDuration: "5m"  # Ramp over 5 minutes
```

---

## Scenarios

**Field**: `scenarios` (required)

**Type**: Array

**Description**: Test scenarios with steps.

**Properties**:

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | Yes | Scenario name |
| `weight` | number | No | Traffic distribution weight |
| `steps` | array | Yes | Scenario steps |
| `dataFile` | object | No | External data file |
| `config` | object | No | Scenario-level overrides |

### Step Properties

| Property | Type | Required | Description |
|----------|------|----------|-------------|
| `name` | string | No | Step name |
| `request` | object | Yes | HTTP request |
| `thinkTime` | string/object | No | Delay after step |
| `assertions` | array | No | Response assertions |
| `extract` | array | No | Data extractors |

**Example**:
```yaml
scenarios:
  - name: "User Login"
    weight: 100
    steps:
      - name: "Login Request"
        request:
          method: "POST"
          path: "/auth/login"
          body: '{"username": "user", "password": "pass"}'
        assertions:
          - statusCode: 200
        extract:
          - name: "token"
            jsonPath: "$.token"
        thinkTime: "2s"
```

---

## Complete Example

```yaml
version: "1.0"

metadata:
  name: "API Load Test"
  description: "Testing main API endpoints"
  tags: ["api", "production"]

config:
  baseUrl: "https://api.example.com"
  timeout: "30s"
  workers: 50
  duration: "10m"

load:
  model: "rps"
  target: 100

scenarios:
  - name: "Get Users"
    weight: 70
    steps:
      - request:
          method: "GET"
          path: "/users"
        assertions:
          - statusCode: 200

  - name: "Create User"
    weight: 30
    steps:
      - request:
          method: "POST"
          path: "/users"
          body: '{"name": "Test User"}'
        assertions:
          - statusCode: 201
```
