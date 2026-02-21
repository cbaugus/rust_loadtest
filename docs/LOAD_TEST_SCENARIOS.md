# E-commerce Test Target - Load Testing Scenarios

**Application URL**: https://ecom.edge.baugus-lab.com
**Version**: 1.0.0
**API Documentation**: https://ecom.edge.baugus-lab.com/swagger/index.html

This document provides comprehensive load testing scenarios for the E-commerce Test Target API. Use these scenarios to build realistic load tests that simulate production traffic patterns.

---

## ⚠️ IMPORTANT: Memory Considerations

**Before running high-load tests, read [MEMORY_OPTIMIZATION.md](MEMORY_OPTIMIZATION.md)**

Key limits to avoid OOM (Out of Memory) errors:
- **With 4GB RAM**: Max 200 concurrent tasks, 5,000 RPS, 1h duration
- **With 8GB RAM**: Max 1,000 concurrent tasks, 25,000 RPS, 2h duration
- **HDR histograms consume 2-4MB each** - they grow unbounded per scenario/step

⚠️ **Your attempted config would need 8-12GB minimum:**
```bash
NUM_CONCURRENT_TASKS=5000  # ❌ Too high for 4GB
TARGET_RPS=50000           # ❌ Too high for 4GB
TEST_DURATION=24h          # ❌ Too long for 4GB
```

✅ **Safe starting config for 4GB:**
```bash
NUM_CONCURRENT_TASKS=200
TARGET_RPS=5000
TEST_DURATION=1h
LOAD_MODEL_TYPE=Rps
```

---

## Table of Contents

1. [Quick Reference](#quick-reference)
2. [Scenario 1: Health & Status Monitoring](#scenario-1-health--status-monitoring)
3. [Scenario 2: Product Browsing](#scenario-2-product-browsing)
4. [Scenario 3: User Registration & Authentication](#scenario-3-user-registration--authentication)
5. [Scenario 4: Complete Shopping Flow](#scenario-4-complete-shopping-flow)
6. [Scenario 5: Cart Operations](#scenario-5-cart-operations)
7. [Scenario 6: Order Management](#scenario-6-order-management)
8. [Scenario 7: Search & Filter](#scenario-7-search--filter)
9. [Scenario 8: Streaming & WebSocket](#scenario-8-streaming--websocket)
10. [Scenario 9: Response Variations](#scenario-9-response-variations)
11. [Scenario 10: Error Handling](#scenario-10-error-handling)
12. [Scenario 11: Mixed Realistic Traffic](#scenario-11-mixed-realistic-traffic)
13. [Scenario 12: Stress Testing](#scenario-12-stress-testing)
14. [Performance Targets](#performance-targets)
15. [Load Patterns](#load-patterns)

---

## Quick Reference

### Base Configuration
```
BASE_URL=https://ecom.edge.baugus-lab.com
SKIP_TLS_VERIFY=false
```

### Key Endpoints
- Health: `GET /health`
- Products: `GET /products`
- Auth: `POST /auth/register`, `POST /auth/login`
- Cart: `GET /cart`, `POST /cart/items`
- Checkout: `POST /checkout`
- Metrics: `GET /metrics`

---

## Scenario 1: Health & Status Monitoring

**Purpose**: Verify service availability and monitor application health.

### Test Case 1.1: Basic Health Check
```bash
# Request
GET /health

# Expected Response (200 OK)
{
  "status": "healthy",
  "timestamp": "2026-02-10T21:00:00Z"
}

# Load Pattern
- Constant RPS: 10
- Duration: Continuous
- Success Criteria: 100% success rate, <50ms p95 latency
```

### Test Case 1.2: Detailed Status Check
```bash
# Request
GET /status

# Expected Response (200 OK)
{
  "status": "ok",
  "timestamp": "2026-02-10T21:00:00Z",
  "uptime": 86400,
  "requests_processed": 1500000,
  "version": "1.0.0"
}

# Load Pattern
- Constant RPS: 5
- Duration: Continuous
- Success Criteria: 100% success rate, <100ms p95 latency
```

### Test Case 1.3: Metrics Scraping
```bash
# Request
GET /metrics

# Expected Response (200 OK)
# TYPE http_requests_total counter
http_requests_total{method="GET",path="/health",status="200"} 1234567
...

# Load Pattern
- Interval: Every 15s (Prometheus scrape)
- Duration: Continuous
- Success Criteria: 100% success rate, <200ms p95 latency
```

---

## Scenario 2: Product Browsing

**Purpose**: Simulate users browsing the product catalog.

### Test Case 2.1: List All Products (Paginated)
```bash
# Request
GET /products?page=1&limit=20

# Expected Response (200 OK)
{
  "products": [...],  # 20 products
  "total": 1000,
  "page": 1,
  "limit": 20,
  "total_pages": 50
}

# Load Pattern
- Ramp: 0 → 100 concurrent users over 2 minutes
- Sustain: 100 concurrent users for 10 minutes
- Ramp down: 100 → 0 over 2 minutes
- Think time: 2-5 seconds between requests
- Success Criteria: <200ms p95 latency, <1% error rate
```

### Test Case 2.2: Get Product Details
```bash
# Setup: Get a product ID from /products
GET /products?limit=1

# Request
GET /products/{product_id}

# Expected Response (200 OK)
{
  "id": "prod-123",
  "name": "Product Name",
  "description": "...",
  "price": 99.99,
  "category": "electronics",
  "stock": 50,
  "image_url": "https://..."
}

# Load Pattern
- Concurrent users: 200
- Duration: 15 minutes
- Distribution: Random product IDs
- Think time: 1-3 seconds
- Success Criteria: <150ms p95 latency, <0.5% error rate
```

### Test Case 2.3: Category Filtering
```bash
# Request
GET /products?category=electronics&limit=50

# Expected Response (200 OK)
{
  "products": [...],  # Electronics products only
  "total": 250,
  "category": "electronics"
}

# Load Pattern
- Concurrent users: 50
- Duration: 10 minutes
- Categories: electronics, clothing, books, sports
- Success Criteria: <250ms p95 latency
```

### Test Case 2.4: Product Search
```bash
# Request
GET /products?search=laptop&limit=20

# Expected Response (200 OK)
{
  "products": [...],  # Products matching "laptop"
  "total": 15
}

# Load Pattern
- Concurrent users: 75
- Duration: 10 minutes
- Search terms: laptop, phone, shirt, book, etc.
- Success Criteria: <300ms p95 latency
```

---

## Scenario 3: User Registration & Authentication

**Purpose**: Test user account creation and login flows.

### Test Case 3.1: User Registration
```bash
# Request
POST /auth/register
Content-Type: application/json

{
  "email": "user-{timestamp}@example.com",
  "password": "SecurePass123!",
  "name": "Test User"
}

# Expected Response (201 Created)
{
  "user": {
    "id": "user-uuid",
    "email": "user-{timestamp}@example.com",
    "name": "Test User"
  },
  "token": "eyJhbGciOiJIUzI1NiIs..."
}

# Load Pattern
- Rate: 5 registrations/second
- Duration: 30 minutes
- Email: Use unique emails (timestamp or UUID)
- Success Criteria: <500ms p95 latency, 100% unique users
```

### Test Case 3.2: User Login
```bash
# Request
POST /auth/login
Content-Type: application/json

{
  "email": "existing-user@example.com",
  "password": "SecurePass123!"
}

# Expected Response (200 OK)
{
  "user": {
    "id": "user-uuid",
    "email": "existing-user@example.com",
    "name": "Test User"
  },
  "token": "eyJhbGciOiJIUzI1NiIs..."
}

# Load Pattern
- Concurrent logins: 100
- Duration: 15 minutes
- Pool: 1000 pre-created users
- Success Criteria: <300ms p95 latency, <1% error rate
```

### Test Case 3.3: Get User Profile
```bash
# Request (requires authentication)
GET /users/me
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "id": "user-uuid",
  "email": "user@example.com",
  "name": "Test User",
  "created_at": "2026-02-10T20:00:00Z"
}

# Load Pattern
- Concurrent users: 200
- Duration: 10 minutes
- Success Criteria: <100ms p95 latency
```

### Test Case 3.4: Logout
```bash
# Request (requires authentication)
POST /auth/logout
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "message": "Logged out successfully"
}

# Load Pattern
- Rate: 10 logouts/second
- Duration: 5 minutes
```

---

## Scenario 4: Complete Shopping Flow

**Purpose**: Simulate the complete e-commerce user journey from browsing to checkout.

### Test Case 4.1: End-to-End Shopping Flow
```bash
# Step 1: Register User
POST /auth/register
{
  "email": "shopper-{id}@example.com",
  "password": "Pass123!",
  "name": "Shopper {id}"
}
# Save token for subsequent requests

# Step 2: Browse Products (think time: 3-5s)
GET /products?limit=10

# Step 3: View Product Details (think time: 5-10s)
GET /products/{product_id}

# Step 4: Add to Cart (think time: 2-3s)
POST /cart/items
Authorization: Bearer {token}
{
  "product_id": "{product_id}",
  "quantity": 2
}

# Step 5: View Cart (think time: 2-3s)
GET /cart
Authorization: Bearer {token}

# Step 6: Add Another Product (think time: 10-15s)
POST /cart/items
Authorization: Bearer {token}
{
  "product_id": "{another_product_id}",
  "quantity": 1
}

# Step 7: Update Cart Item (think time: 2-3s)
PUT /cart/items/{item_id}
Authorization: Bearer {token}
{
  "quantity": 3
}

# Step 8: View Updated Cart (think time: 2-3s)
GET /cart
Authorization: Bearer {token}

# Step 9: Checkout (think time: 30-60s for entering payment)
POST /checkout
Authorization: Bearer {token}
{
  "cart_id": "{cart_id}",
  "shipping_address": {
    "street": "123 Main St",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94102",
    "country": "US"
  },
  "payment": {
    "method": "credit_card",
    "card_token": "tok_visa_{random}"
  }
}

# Step 10: View Order Confirmation (think time: 5s)
GET /orders/{order_id}
Authorization: Bearer {token}

# Load Pattern
- Concurrent flows: 50
- Duration: 30 minutes
- Completion rate: 70% (30% abandon at various stages)
- Think times: As specified per step
- Success Criteria:
  - <2% error rate across all steps
  - <500ms p95 for cart operations
  - <1s p95 for checkout
```

---

## Scenario 5: Cart Operations

**Purpose**: Test shopping cart functionality under load.

### Test Case 5.1: View Empty Cart
```bash
# Request
GET /cart
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "id": "cart-uuid",
  "user_id": "user-uuid",
  "items": [],
  "subtotal": 0,
  "tax": 0,
  "shipping": 0,
  "total": 0
}

# Load Pattern
- Concurrent users: 100
- Duration: 5 minutes
```

### Test Case 5.2: Add Item to Cart
```bash
# Request
POST /cart/items
Authorization: Bearer {token}
Content-Type: application/json

{
  "product_id": "prod-123",
  "quantity": 2
}

# Expected Response (201 Created)
{
  "cart": {
    "id": "cart-uuid",
    "items": [
      {
        "id": "item-uuid",
        "product_id": "prod-123",
        "quantity": 2,
        "price": 99.99,
        "subtotal": 199.98
      }
    ],
    "subtotal": 199.98,
    "tax": 16.00,
    "shipping": 10.00,
    "total": 225.98
  }
}

# Load Pattern
- Concurrent operations: 200
- Duration: 15 minutes
- Success Criteria: <300ms p95 latency
```

### Test Case 5.3: Update Cart Item Quantity
```bash
# Request
PUT /cart/items/{item_id}
Authorization: Bearer {token}
Content-Type: application/json

{
  "quantity": 5
}

# Expected Response (200 OK)
# Updated cart with new quantity

# Load Pattern
- Concurrent updates: 100
- Duration: 10 minutes
```

### Test Case 5.4: Remove Item from Cart
```bash
# Request
DELETE /cart/items/{item_id}
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "message": "Item removed from cart"
}

# Load Pattern
- Concurrent deletions: 50
- Duration: 10 minutes
```

### Test Case 5.5: Clear Cart
```bash
# Request
DELETE /cart
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "message": "Cart cleared"
}

# Load Pattern
- Rate: 20 clears/second
- Duration: 5 minutes
```

---

## Scenario 6: Order Management

**Purpose**: Test order placement and retrieval.

### Test Case 6.1: Place Order (Checkout)
```bash
# Request
POST /checkout
Authorization: Bearer {token}
Content-Type: application/json

{
  "cart_id": "cart-uuid",
  "shipping_address": {
    "street": "123 Main St",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94102",
    "country": "US"
  },
  "billing_address": {
    "street": "123 Main St",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94102",
    "country": "US"
  },
  "payment": {
    "method": "credit_card",
    "card_token": "tok_visa"
  }
}

# Expected Response (201 Created)
{
  "order_id": "order-uuid",
  "status": "confirmed",
  "total": 225.98,
  "confirmation_number": "ORD-12345678"
}

# Load Pattern
- Rate: 10 orders/second
- Duration: 20 minutes
- Success Criteria: <1s p95 latency, <0.5% error rate
```

### Test Case 6.2: Get Order Details
```bash
# Request
GET /orders/{order_id}
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "id": "order-uuid",
  "user_id": "user-uuid",
  "status": "confirmed",
  "items": [...],
  "shipping_address": {...},
  "total": 225.98,
  "confirmation_number": "ORD-12345678",
  "created_at": "2026-02-10T21:00:00Z"
}

# Load Pattern
- Concurrent users: 150
- Duration: 15 minutes
```

### Test Case 6.3: List User Orders
```bash
# Request
GET /orders
Authorization: Bearer {token}

# Expected Response (200 OK)
{
  "orders": [
    {
      "id": "order-uuid",
      "status": "confirmed",
      "total": 225.98,
      "created_at": "2026-02-10T21:00:00Z"
    },
    ...
  ]
}

# Load Pattern
- Concurrent users: 100
- Duration: 10 minutes
```

---

## Scenario 7: Search & Filter

**Purpose**: Test search and filtering performance.

### Test Case 7.1: Search Products
```bash
# Request
GET /products?search={query}&limit=20

# Search queries (rotate through):
- "laptop"
- "phone"
- "wireless"
- "pro"
- "gaming"
- "portable"

# Load Pattern
- Concurrent searches: 100
- Duration: 15 minutes
- Query distribution: Realistic search terms
- Success Criteria: <400ms p95 latency
```

### Test Case 7.2: Filter by Category
```bash
# Request
GET /products?category={category}&limit=50

# Categories (rotate through):
- electronics
- clothing
- books
- sports
- home

# Load Pattern
- Concurrent users: 75
- Duration: 10 minutes
```

### Test Case 7.3: Combined Search and Filter
```bash
# Request
GET /products?category=electronics&search=laptop&limit=20

# Load Pattern
- Concurrent users: 50
- Duration: 10 minutes
```

---

## Scenario 8: Streaming & WebSocket

**Purpose**: Test streaming endpoints and WebSocket connections.

### Test Case 8.1: Server-Sent Events (SSE)
```bash
# Request
GET /stream?events=10

# Expected: Stream of 10 events
data: {"id": 1, "message": "Event 1", "timestamp": "..."}

data: {"id": 2, "message": "Event 2", "timestamp": "..."}

...

# Load Pattern
- Concurrent streams: 50
- Events per stream: 10-100
- Duration: 15 minutes
- Success Criteria: All events received, no disconnects
```

### Test Case 8.2: WebSocket Echo
```bash
# Connect
ws://ecom.edge.baugus-lab.com/ws/echo

# Send messages
{"type": "ping", "data": "Hello"}

# Receive echo
{"type": "pong", "data": "Hello", "timestamp": "..."}

# Load Pattern
- Concurrent connections: 100
- Messages per connection: 50
- Duration: 10 minutes
- Success Criteria: 100% message delivery
```

---

## Scenario 9: Response Variations

**Purpose**: Test various response formats and sizes.

### Test Case 9.1: JSON Response
```bash
# Request
GET /bytes/1024?format=json

# Expected: 1KB JSON response

# Load Pattern
- Sizes: 1KB, 10KB, 100KB, 1MB
- Concurrent users: 50 per size
- Duration: 10 minutes
```

### Test Case 9.2: XML Response
```bash
# Request
GET /bytes/1024?format=xml

# Expected: 1KB XML response

# Load Pattern
- Concurrent users: 25
- Duration: 5 minutes
```

### Test Case 9.3: CSV Response
```bash
# Request
GET /csv

# Expected: CSV file with product data

# Load Pattern
- Concurrent downloads: 50
- Duration: 5 minutes
```

### Test Case 9.4: HTML Response
```bash
# Request
GET /html

# Expected: HTML page

# Load Pattern
- Concurrent requests: 30
- Duration: 5 minutes
```

---

## Scenario 10: Error Handling

**Purpose**: Test application resilience and error handling.

### Test Case 10.1: Simulated Delays
```bash
# Request
GET /delay/{milliseconds}

# Test delays: 100ms, 500ms, 1000ms, 2000ms

# Load Pattern
- 100ms delay: 50 concurrent, expect <150ms p95
- 500ms delay: 30 concurrent, expect <550ms p95
- 1s delay: 20 concurrent, expect <1.1s p95
- 2s delay: 10 concurrent, expect <2.1s p95
```

### Test Case 10.2: Error Simulation
```bash
# Request
GET /error/{status_code}

# Status codes: 400, 404, 500, 503

# Expected Responses:
400: {"error": "Bad Request"}
404: {"error": "Not Found"}
500: {"error": "Internal Server Error"}
503: {"error": "Service Unavailable"}

# Load Pattern
- Concurrent requests: 20 per status code
- Duration: 5 minutes
- Success Criteria: Correct error responses
```

### Test Case 10.3: Random Delay
```bash
# Request
GET /delay/random?max=2000

# Expected: Random delay 0-2000ms

# Load Pattern
- Concurrent requests: 50
- Duration: 10 minutes
```

---

## Scenario 11: Mixed Realistic Traffic

**Purpose**: Simulate realistic production traffic patterns.

### Test Case 11.1: Daily Traffic Pattern
```yaml
# Configuration
LOAD_MODEL_TYPE: DailyTraffic
DAILY_MIN_RPS: 100
DAILY_MID_RPS: 500
DAILY_MAX_RPS: 1500
DAILY_CYCLE_DURATION: 1h

# Traffic distribution (1 hour = 1 simulated day):
- 00:00-07:00 (0-12min): Night - 100 RPS
- 07:00-09:00 (12-18min): Morning ramp - 100→1500 RPS
- 09:00-12:00 (18-24min): Peak - 1500 RPS
- 12:00-14:00 (24-30min): Lunch decline - 1500→500 RPS
- 14:00-17:00 (30-42min): Afternoon - 500 RPS
- 17:00-20:00 (42-54min): Evening decline - 500→100 RPS
- 20:00-24:00 (54-60min): Night - 100 RPS

# Request mix:
- 40% Product browsing (GET /products)
- 20% Product details (GET /products/{id})
- 15% Search (GET /products?search=...)
- 10% Cart operations (POST/PUT/DELETE /cart/*)
- 10% Auth (POST /auth/login)
- 4% Checkout (POST /checkout)
- 1% Health checks (GET /health)

# User behavior:
- 30% bounce (single request)
- 40% browse only (2-5 requests)
- 20% add to cart (6-10 requests)
- 10% complete purchase (11-15 requests)

# Duration: 4 hours (4 simulated days)
```

### Test Case 11.2: Flash Sale Spike
```yaml
# Normal traffic: 200 RPS for 30 minutes
# Spike announcement: Ramp 200→2000 RPS over 2 minutes
# Flash sale: 2000 RPS for 15 minutes
# Post-sale: Decline 2000→300 RPS over 5 minutes
# Cooldown: 300 RPS for 15 minutes

# Request mix during spike:
- 60% Product details for sale items
- 25% Add to cart
- 10% Checkout
- 5% Other

# Success Criteria:
- <1s p95 latency during spike
- <5% error rate
- No service degradation
```

### Test Case 11.3: Black Friday Scenario
```yaml
# Pre-event: 500 RPS baseline
# Countdown (2 hours): Gradual increase 500→3000 RPS
# Event start: Spike to 5000 RPS
# Sustained (4 hours): 4000-5000 RPS
# Decline (2 hours): 5000→1000 RPS
# Post-event: 1000 RPS baseline

# Duration: 12 hours
# Total requests: ~100M

# Request mix:
- 35% Product browsing
- 30% Product details
- 15% Cart operations
- 12% Checkout
- 5% Search
- 3% Auth

# Success Criteria:
- <2s p95 latency
- <2% error rate
- Auto-scaling triggered appropriately
```

---

## Scenario 12: Stress Testing

**Purpose**: Find breaking points and maximum capacity.

### Test Case 12.1: Capacity Test
```yaml
# Objective: Find maximum sustainable RPS

# Method: Incremental load increase
- Start: 100 RPS
- Increment: +100 RPS every 5 minutes
- Continue until: Error rate >5% OR latency p95 >5s
- Endpoint mix: 70% reads, 30% writes

# Monitor:
- Response times (p50, p95, p99)
- Error rates
- System resources (CPU, memory, connections)
- Database performance

# Expected outcome:
- Identify maximum RPS capacity
- Identify bottlenecks
- Document degradation curve
```

### Test Case 12.2: Spike Test
```yaml
# Objective: Test recovery from sudden traffic spikes

# Pattern:
- Baseline: 200 RPS for 5 minutes
- Spike: Instant jump to 2000 RPS for 2 minutes
- Recovery: Drop to 200 RPS for 5 minutes
- Repeat: 3 times

# Success Criteria:
- No crashes
- Recovery within 30s after spike
- <10% error rate during spike
```

### Test Case 12.3: Soak Test
```yaml
# Objective: Identify memory leaks and resource exhaustion

# Pattern:
- Steady load: 500 RPS
- Duration: 24 hours
- Request mix: Realistic mix from Scenario 11.1

# Monitor:
- Memory usage over time
- Connection pool exhaustion
- Database connections
- Response time degradation

# Success Criteria:
- No memory leaks (stable memory usage)
- Consistent performance over 24h
- No resource exhaustion
```

### Test Case 12.4: Database Stress
```yaml
# Objective: Test database performance under heavy write load

# Pattern:
- 100 concurrent users
- Each user:
  - Register → Login → Add 10 items to cart → Checkout
  - Repeat continuously
- Duration: 30 minutes

# Expected:
- Heavy INSERT load (users, cart_items, orders, order_items)
- Transaction handling
- Lock contention

# Monitor:
- Database response times
- Connection pool saturation
- Transaction failures
- Lock timeouts
```

---

## Performance Targets

### Response Time Targets (p95)

| Endpoint Category | Target | Acceptable | Critical |
|------------------|--------|------------|----------|
| Health checks | <50ms | <100ms | <200ms |
| Product listing | <200ms | <500ms | <1s |
| Product details | <150ms | <300ms | <750ms |
| Search | <400ms | <800ms | <2s |
| Login | <300ms | <600ms | <1.5s |
| Registration | <500ms | <1s | <2s |
| Cart operations | <250ms | <500ms | <1s |
| Checkout | <800ms | <1.5s | <3s |
| Order retrieval | <200ms | <400ms | <1s |

### Throughput Targets

| Scenario | Target RPS | Peak RPS | Notes |
|----------|-----------|----------|-------|
| Normal traffic | 200-500 | 1000 | Typical weekday |
| Peak hours | 500-1000 | 2000 | Evening/weekend |
| Flash sale | 1000-2000 | 5000 | Limited duration |
| Black Friday | 2000-4000 | 8000 | Annual peak |

### Error Rate Targets

- **Normal operation**: <0.5% error rate
- **High load**: <2% error rate
- **Stress conditions**: <5% error rate
- **Critical**: Graceful degradation, no crashes

### Resource Utilization

- **CPU**: <70% average, <90% peak
- **Memory**: <80% allocated, no leaks
- **Connections**: <80% pool capacity
- **Database**: <70% connection pool

---

## Load Patterns

### Pattern 1: Constant Load
```yaml
Type: Constant RPS
RPS: 100
Duration: 30m
Use: Baseline performance testing
```

### Pattern 2: Ramp Up
```yaml
Type: RampRps
Start: 0 RPS
End: 1000 RPS
Duration: 10m
Use: Warm-up, gradual load increase
```

### Pattern 3: Step Load
```yaml
Type: Steps
Steps:
  - RPS: 100, Duration: 5m
  - RPS: 300, Duration: 5m
  - RPS: 500, Duration: 5m
  - RPS: 1000, Duration: 5m
Use: Capacity testing, finding limits
```

### Pattern 4: Spike
```yaml
Type: Spike
Baseline: 200 RPS
Spike: 2000 RPS
Spike Duration: 2m
Recovery: 200 RPS
Use: Resilience testing
```

### Pattern 5: Wave
```yaml
Type: Wave
Min: 100 RPS
Max: 1000 RPS
Period: 10m
Duration: 60m
Use: Variable load simulation
```

### Pattern 6: Daily Pattern
```yaml
Type: DailyTraffic
Min: 100 RPS (night)
Mid: 500 RPS (afternoon)
Max: 1500 RPS (peak)
Cycle: 1h
Use: Realistic traffic simulation
```

---

## Test Execution Guide

### Pre-Test Checklist

- [ ] Verify application is deployed and healthy
- [ ] Confirm monitoring is active (Prometheus, logs)
- [ ] Set up performance dashboards
- [ ] Configure alerts for critical metrics
- [ ] Create test user accounts
- [ ] Warm up the application (5 min at 10% load)
- [ ] Take baseline measurements
- [ ] Document test environment details

### During Test

- Monitor key metrics:
  - Response times (p50, p95, p99, max)
  - Error rates and types
  - Throughput (RPS)
  - Active connections
  - CPU and memory usage
  - Database performance

### Post-Test Analysis

- [ ] Verify no data corruption
- [ ] Check for memory leaks
- [ ] Analyze error logs
- [ ] Generate performance reports
- [ ] Compare against baselines
- [ ] Document bottlenecks found
- [ ] Create improvement recommendations

---

## Common Test Data

### Sample Users
```json
{
  "email": "loadtest-user-{id}@example.com",
  "password": "LoadTest123!",
  "name": "Load Test User {id}"
}
```

### Sample Products
```
Available via: GET /products
Total: 1000 products
Categories: electronics, clothing, books, sports, home
Price range: $9.99 - $1999.99
```

### Sample Addresses
```json
{
  "shipping_address": {
    "street": "123 Test Street",
    "city": "San Francisco",
    "state": "CA",
    "zip": "94102",
    "country": "US"
  }
}
```

### Payment Tokens
```
Valid test tokens:
- tok_visa
- tok_mastercard
- tok_amex
```

---

## Notes for Load Testing Team

1. **Authentication**: Most endpoints require JWT tokens. Implement token management:
   - Register users in setup phase
   - Reuse tokens across requests
   - Refresh expired tokens

2. **State Management**: Shopping flow requires maintaining state:
   - Cart IDs from cart creation
   - Product IDs from product listing
   - Order IDs from checkout

3. **Think Times**: Include realistic think times between requests (2-10 seconds) to simulate real user behavior.

4. **Data Cleanup**: Implement cleanup routines for test data:
   - Remove test users after tests
   - Clear abandoned carts
   - Archive test orders

5. **Error Handling**: Distinguish between:
   - Expected errors (404 for invalid product)
   - Test failures (500 errors, timeouts)
   - Network issues

6. **Distributed Load**: Consider running load generators from multiple locations to simulate geographic distribution.

7. **Monitoring**: Set up real-time monitoring dashboard to track test progress and identify issues early.

8. **Baseline**: Always run baseline tests before making changes to compare performance.

---

## Memory & Resource Planning

For detailed information on memory requirements and optimization:
- See [MEMORY_OPTIMIZATION.md](MEMORY_OPTIMIZATION.md) for memory analysis
- Estimate: **~1MB per 100 sustained RPS over 1 hour**
- HDR histogram overhead: **2-4MB per unique scenario/step**
- Concurrent task overhead: **~8KB per task**

Quick memory requirements:
- **512MB**: 10 tasks, 500 RPS, 5 min
- **2GB**: 100 tasks, 5,000 RPS, 30 min
- **4GB**: 500 tasks, 10,000 RPS, 1 hour
- **8GB+**: 1,000 tasks, 25,000 RPS, 2+ hours

Always start small and scale up gradually while monitoring `docker stats`.

---

## Support & Contact

- **Application URL**: https://ecom.edge.baugus-lab.com
- **API Documentation**: https://ecom.edge.baugus-lab.com/swagger/index.html
- **Health Check**: https://ecom.edge.baugus-lab.com/health
- **Metrics**: https://ecom.edge.baugus-lab.com/metrics
- **Repository**: https://github.com/cbaugus/ecom-test-target

---

**Document Version**: 1.0
**Last Updated**: 2026-02-10
**Application Version**: 1.0.0
