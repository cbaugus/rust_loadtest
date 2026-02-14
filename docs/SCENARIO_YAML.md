//! Documentation for Scenario YAML Definitions (Issue #42)

# Scenario YAML Definitions

## Overview

Scenarios define multi-step user journeys for load testing. Each scenario represents a realistic user flow with sequential steps, variable extraction, assertions, and realistic timing.

## Key Features

✅ **Multiple scenarios per config** - Mix different user flows
✅ **Weighted traffic distribution** - Control scenario selection probability
✅ **Multi-step sequences** - Complex user journeys
✅ **Variable extraction** - Extract and reuse data between steps
✅ **Assertions** - Validate responses at each step
✅ **Think time** - Realistic delays (fixed or random)
✅ **Data files** - CSV/JSON data for data-driven testing
✅ **Scenario-level config** - Override global settings per scenario

## Basic Scenario

```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "10m"
load:
  model: "concurrent"
scenarios:
  - name: "API Health Check"
    steps:
      - request:
          method: "GET"
          path: "/health"
```

## Multiple Scenarios with Weighting

Weight determines traffic distribution. Total weights don't need to sum to 100.

```yaml
scenarios:
  - name: "Read Operations"
    weight: 80  # 80% of traffic
    steps:
      - request:
          method: "GET"
          path: "/api/read"

  - name: "Write Operations"
    weight: 15  # 15% of traffic
    steps:
      - request:
          method: "POST"
          path: "/api/write"

  - name: "Delete Operations"
    weight: 5  # 5% of traffic
    steps:
      - request:
          method: "DELETE"
          path: "/api/delete"
```

**Traffic calculation:** `scenario_weight / sum(all_weights) = traffic_percentage`

## Multi-Step Scenarios

### E-commerce Example

```yaml
scenarios:
  - name: "Shopping Flow"
    weight: 70
    steps:
      # Step 1: Homepage
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"

      # Step 2: Search with extraction
      - name: "Search Products"
        request:
          method: "GET"
          path: "/search?q=laptop"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.products[0].id"
        thinkTime: "3s"

      # Step 3: Use extracted variable
      - name: "Product Details"
        request:
          method: "GET"
          path: "/products/${productId}"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "5s"

      # Step 4: Add to cart
      - name: "Add to Cart"
        request:
          method: "POST"
          path: "/cart"
          body: '{"productId": "${productId}", "quantity": 1}'
        assertions:
          - type: "statusCode"
            expected: 201
```

## Think Time

Think time simulates realistic user behavior by adding delays between steps.

### Fixed Think Time

```yaml
steps:
  - request:
      method: "GET"
      path: "/page1"
    thinkTime: "3s"  # Always 3 seconds

  - request:
      method: "GET"
      path: "/page2"
    thinkTime: "5000"  # Raw milliseconds
```

### Random Think Time

```yaml
steps:
  - request:
      method: "GET"
      path: "/browse"
    thinkTime:
      min: "2s"
      max: "5s"  # Random delay between 2-5 seconds

  - request:
      method: "GET"
      path: "/search"
    thinkTime:
      min: "1s"
      max: "10s"  # Variable user reading time
```

## Variable Extraction

Extract data from responses to use in subsequent steps.

### JSON Path Extraction

```yaml
steps:
  - name: "Get User"
    request:
      method: "GET"
      path: "/user/profile"
    extract:
      - type: "jsonPath"
        name: "userId"
        jsonPath: "$.id"
      - type: "jsonPath"
        name: "email"
        jsonPath: "$.email"
```

### Header Extraction

```yaml
extract:
  - type: "header"
    name: "authToken"
    header: "X-Auth-Token"
```

### Cookie Extraction

```yaml
extract:
  - type: "cookie"
    name: "sessionId"
    cookie: "JSESSIONID"
```

### Regex Extraction

```yaml
extract:
  - type: "regex"
    name: "transactionId"
    regex: "Transaction ID: (\\d+)"
```

## Using Extracted Variables

Variables use `${variableName}` syntax:

```yaml
steps:
  # Extract variable
  - request:
      method: "POST"
      path: "/auth/login"
      body: '{"email": "user@test.com", "password": "pass123"}'
    extract:
      - type: "jsonPath"
        name: "token"
        jsonPath: "$.accessToken"

  # Use in header
  - request:
      method: "GET"
      path: "/api/profile"
      headers:
        Authorization: "Bearer ${token}"

  # Use in path
  - request:
      method: "GET"
      path: "/users/${userId}/orders"

  # Use in body
  - request:
      method: "POST"
      path: "/api/purchase"
      body: '{"userId": "${userId}", "productId": "${productId}"}'
```

## Assertions

Validate responses at each step.

### Status Code

```yaml
assertions:
  - type: "statusCode"
    expected: 200
```

### Response Time

```yaml
assertions:
  - type: "responseTime"
    max: "500ms"
```

### Body Contains

```yaml
assertions:
  - type: "bodyContains"
    text: "success"
```

### Body Matches Regex

```yaml
assertions:
  - type: "bodyMatches"
    regex: "User-\\d+"
```

### JSON Path

```yaml
assertions:
  - type: "jsonPath"
    path: "$.status"
    expected: "active"
```

### Header Exists

```yaml
assertions:
  - type: "headerExists"
    header: "X-Request-ID"
```

### Multiple Assertions

```yaml
steps:
  - request:
      method: "POST"
      path: "/api/order"
      body: '{"items": [1, 2, 3]}'
    assertions:
      - type: "statusCode"
        expected: 201
      - type: "responseTime"
        max: "1s"
      - type: "jsonPath"
        path: "$.orderId"
      - type: "bodyContains"
        text: "confirmed"
```

## Headers and Query Parameters

### Custom Headers

```yaml
request:
  method: "GET"
  path: "/api/data"
  headers:
    Authorization: "Bearer ${token}"
    X-Custom-Header: "value"
    Content-Type: "application/json"
```

### Query Parameters

```yaml
request:
  method: "GET"
  path: "/api/search"
  queryParams:
    q: "laptop"
    limit: "20"
    sort: "price"
    order: "asc"
```

**Result:** `/api/search?q=laptop&limit=20&sort=price&order=asc`

## Data Files (Data-Driven Testing)

Load test data from CSV or JSON files.

### CSV Data File

**File: users.csv**
```csv
username,password,email
user1,pass1,user1@test.com
user2,pass2,user2@test.com
user3,pass3,user3@test.com
```

**YAML:**
```yaml
scenarios:
  - name: "Login Test"
    dataFile:
      path: "./testdata/users.csv"
      format: "csv"
      strategy: "sequential"  # or "random" or "cycle"
    steps:
      - request:
          method: "POST"
          path: "/login"
          body: '{"username": "${username}", "password": "${password}"}'
```

### JSON Data File

**File: products.json**
```json
[
  {"productId": "P001", "name": "Laptop"},
  {"productId": "P002", "name": "Mouse"},
  {"productId": "P003", "name": "Keyboard"}
]
```

**YAML:**
```yaml
scenarios:
  - name: "Product Test"
    dataFile:
      path: "./testdata/products.json"
      format: "json"
      strategy: "random"
    steps:
      - request:
          method: "GET"
          path: "/products/${productId}"
```

### Data Strategies

| Strategy | Behavior |
|----------|----------|
| `sequential` | Iterate through data rows in order (default) |
| `random` | Select random rows |
| `cycle` | Loop back to start when reaching end |

## Scenario-Level Configuration

Override global settings for specific scenarios.

```yaml
config:
  baseUrl: "https://api.example.com"
  timeout: "30s"  # Global timeout
  duration: "10m"

scenarios:
  - name: "Fast API"
    steps:
      - request:
          method: "GET"
          path: "/fast"

  - name: "Slow API"
    config:
      timeout: "120s"  # Override for this scenario
      retryCount: 3
      retryDelay: "5s"
    steps:
      - request:
          method: "GET"
          path: "/slow"
```

### Available Overrides

- `timeout` - Request timeout (overrides global)
- `retryCount` - Number of retry attempts
- `retryDelay` - Delay between retries

## Complete Example

```yaml
version: "1.0"
metadata:
  name: "E-commerce Load Test"
  description: "Realistic shopping flow with authentication"

config:
  baseUrl: "https://shop.example.com"
  workers: 50
  timeout: "30s"
  duration: "30m"

load:
  model: "ramp"
  min: 10
  max: 200
  rampDuration: "10m"

scenarios:
  # Scenario 1: Complete shopping flow (70% of traffic)
  - name: "Browse and Purchase"
    weight: 70
    config:
      timeout: "60s"
      retryCount: 2
    dataFile:
      path: "./users.csv"
      format: "csv"
      strategy: "cycle"
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "responseTime"
            max: "1s"
        thinkTime:
          min: "1s"
          max: "3s"

      - name: "Login"
        request:
          method: "POST"
          path: "/api/auth/login"
          body: '{"email": "${email}", "password": "${password}"}'
          headers:
            Content-Type: "application/json"
        extract:
          - type: "jsonPath"
            name: "authToken"
            jsonPath: "$.token"
        assertions:
          - type: "statusCode"
            expected: 200
        thinkTime: "2s"

      - name: "Search"
        request:
          method: "GET"
          path: "/api/products/search"
          queryParams:
            q: "laptop"
            limit: "20"
          headers:
            Authorization: "Bearer ${authToken}"
        extract:
          - type: "jsonPath"
            name: "productId"
            jsonPath: "$.results[0].id"
          - type: "jsonPath"
            name: "price"
            jsonPath: "$.results[0].price"
        thinkTime:
          min: "2s"
          max: "5s"

      - name: "View Product"
        request:
          method: "GET"
          path: "/api/products/${productId}"
          headers:
            Authorization: "Bearer ${authToken}"
        assertions:
          - type: "statusCode"
            expected: 200
          - type: "bodyContains"
            text: "${productId}"
        thinkTime: "4s"

      - name: "Add to Cart"
        request:
          method: "POST"
          path: "/api/cart/items"
          body: '{"productId": "${productId}", "quantity": 1}'
          headers:
            Authorization: "Bearer ${authToken}"
            Content-Type: "application/json"
        assertions:
          - type: "statusCode"
            expected: 201
          - type: "jsonPath"
            path: "$.cartTotal"
        thinkTime: "2s"

      - name: "Checkout"
        request:
          method: "POST"
          path: "/api/orders"
          body: '{}'
          headers:
            Authorization: "Bearer ${authToken}"
            Content-Type: "application/json"
        extract:
          - type: "jsonPath"
            name: "orderId"
            jsonPath: "$.orderId"
        assertions:
          - type: "statusCode"
            expected: 201
          - type: "responseTime"
            max: "2s"

  # Scenario 2: Quick browsing (30% of traffic)
  - name: "Quick Browse"
    weight: 30
    steps:
      - name: "Homepage"
        request:
          method: "GET"
          path: "/"
        thinkTime: "1s"

      - name: "Category"
        request:
          method: "GET"
          path: "/category/electronics"
        thinkTime:
          min: "2s"
          max: "4s"

      - name: "Product List"
        request:
          method: "GET"
          path: "/api/products"
          queryParams:
            category: "electronics"
            limit: "50"
        assertions:
          - type: "statusCode"
            expected: 200
```

## Best Practices

### 1. Realistic Think Times

Use random think times to simulate real user behavior:

```yaml
thinkTime:
  min: "2s"
  max: "10s"  # Reading time varies
```

### 2. Scenario Weighting

Base weights on real traffic patterns:

```yaml
scenarios:
  - name: "Read"
    weight: 90  # 90% reads
  - name: "Write"
    weight: 10  # 10% writes
```

### 3. Error Handling

Add retries for flaky endpoints:

```yaml
scenarios:
  - name: "External API"
    config:
      retryCount: 3
      retryDelay: "2s"
```

### 4. Assertions

Validate critical responses:

```yaml
assertions:
  - type: "statusCode"
    expected: 200
  - type: "responseTime"
    max: "500ms"
  - type: "jsonPath"
    path: "$.status"
    expected: "success"
```

### 5. Variable Extraction

Extract all needed data in one step:

```yaml
extract:
  - type: "jsonPath"
    name: "userId"
    jsonPath: "$.id"
  - type: "jsonPath"
    name: "token"
    jsonPath: "$.token"
  - type: "header"
    name: "sessionId"
    header: "X-Session-ID"
```

## Testing Scenarios

### Validate Syntax

```bash
rust-loadtest --config test.yaml --validate
```

### Dry Run

```bash
rust-loadtest --config test.yaml --dry-run --duration 1m
```

### Single Scenario

```bash
rust-loadtest --config test.yaml --scenario "Browse and Purchase"
```

## Related Documentation

- [YAML Configuration Guide](/docs/YAML_CONFIG.md)
- [Variable Extraction Guide](/docs/EXTRACTION.md)
- [Assertions Reference](/docs/ASSERTIONS.md)
- [Data Files Guide](/docs/DATA_FILES.md)
