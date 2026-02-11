# Phase 1: Core Engine Enhancement - Implementation Plan

**Branch**: `develop/phase1-scenario-engine`
**Duration**: ~7 weeks (estimated)
**Target**: Enable realistic multi-step scenario testing for e-commerce flows

---

## Overview

Phase 1 transforms the rust-loadtest tool from a simple RPS generator into a full-featured scenario execution engine capable of testing complex user journeys like shopping flows, authentication sequences, and multi-step API interactions.

### Key Capabilities to Add:
- Multi-step scenario execution (register → browse → add to cart → checkout)
- Variable extraction from responses (product IDs, auth tokens, cart IDs)
- Session and cookie management (JWT tokens, session cookies)
- Response assertions (validate success criteria)
- Realistic user behavior (think times, delays)
- Advanced metrics (percentile latencies P50/P90/P95/P99)

### Testing Target:
- Mock E-commerce API: https://ecom.edge.baugus-lab.com
- 12 comprehensive test scenarios (see LOAD_TEST_SCENARIOS.md)

---

## Implementation Waves

### Wave 1: Foundation (Weeks 1-3)
Critical P0 issues that unblock all other work.

### Wave 2: Realistic Behavior (Weeks 4-5)
Make tests behave like real users with assertions and delays.

### Wave 3: Enhanced Capabilities (Weeks 6-7)
Additional features for comprehensive testing.

---

## Issues and Progress Tracker

### ✅ Completed
_None yet - starting Phase 1_

### 🚧 In Progress
- [x] **Issue #26**: Multi-step scenario execution engine (P0, XL - 2+ weeks) - **IN PROGRESS**
  - Branch: `feature/issue-26-multi-step-scenarios`

### 📋 Todo - Wave 1 (Weeks 1-3)
- [x] **Issue #26**: Multi-step scenario execution engine (P0, XL)
  - [x] Design: Scenario and Step data structures (src/scenario.rs)
  - [x] Design: Variable context per virtual user (ScenarioContext)
  - [x] Implement: Sequential step execution (src/executor.rs)
  - [x] Implement: Step result propagation (StepResult, ScenarioResult)
  - [x] Implement: Error handling per step (error messages, failed_at_step)
  - [x] Implement: Variable substitution in requests (${var} and $var syntax)
  - [x] Tests: Unit tests for ScenarioContext (8 tests passing)
  - [ ] Tests: Integration test with 3-step flow (TODO)
  - [ ] Integration: Wire into worker.rs (TODO)
  - [ ] Example: Create example scenario config (TODO)
  - [ ] Documentation: Usage examples (TODO)

- [ ] **Issue #27**: Variable extraction from responses (P0, L)
  - [ ] Implement: JSONPath extractor (serde_json)
  - [ ] Implement: Regex extractor (regex crate)
  - [ ] Implement: Header extractor
  - [ ] Implement: Variable storage in user context
  - [ ] Implement: Variable substitution in requests
  - [ ] Tests: Extract product_id from JSON
  - [ ] Tests: Extract auth token from response

- [ ] **Issue #28**: Cookie and session management (P0, M)
  - [ ] Implement: Cookie jar per virtual user
  - [ ] Implement: Automatic cookie handling
  - [ ] Implement: Authorization header management
  - [ ] Implement: Session persistence across steps
  - [ ] Tests: Login flow with token persistence
  - [ ] Tests: Cart operations with session

### 📋 Todo - Wave 2 (Weeks 4-5)
- [ ] **Issue #29**: Think times and delays (P1, S)
  - [ ] Implement: Fixed delay configuration
  - [ ] Implement: Random delay (min-max range)
  - [ ] Implement: Per-step think time
  - [ ] Tests: Verify timing accuracy

- [ ] **Issue #30**: Response assertions framework (P0, L)
  - [ ] Design: Assertion types enum
  - [ ] Implement: Status code assertions
  - [ ] Implement: JSONPath assertions
  - [ ] Implement: Response time assertions
  - [ ] Implement: Content matching (regex, contains)
  - [ ] Implement: Assertion result tracking
  - [ ] Tests: Failed assertion handling

- [ ] **Issue #33**: Percentile latency metrics (P1, M)
  - [ ] Research: HDR Histogram vs alternatives
  - [ ] Implement: P50, P90, P95, P99 tracking
  - [ ] Implement: Per-endpoint percentiles
  - [ ] Implement: Final report with percentiles
  - [ ] Tests: Verify percentile calculations

### 📋 Todo - Wave 3 (Weeks 6-7)
- [ ] **Issue #32**: All HTTP methods (P2, S)
  - [ ] Implement: PUT, PATCH, DELETE support
  - [ ] Implement: HEAD, OPTIONS support
  - [ ] Tests: Cart update (PUT), delete (DELETE)

- [ ] **Issue #31**: CSV data-driven testing (P1, M)
  - [ ] Implement: CSV parser
  - [ ] Implement: Data row iteration per VU
  - [ ] Implement: Variable substitution from CSV
  - [ ] Tests: Load user pool from CSV

- [ ] **Issue #34**: Error categorization (P2, M)
  - [ ] Implement: Error type enum
  - [ ] Implement: Error counting by category
  - [ ] Implement: Error breakdown in metrics
  - [ ] Tests: Distinguish 4xx vs 5xx vs network

- [ ] **Issue #35**: Per-scenario throughput (P2, S)
  - [ ] Implement: Separate metrics per scenario
  - [ ] Implement: RPS tracking per scenario
  - [ ] Tests: Multi-scenario RPS reporting

- [ ] **Issue #36**: Connection pooling stats (P3, S)
  - [ ] Implement: Active connection tracking
  - [ ] Implement: Pool utilization metrics
  - [ ] Tests: Connection pool monitoring

---

## Scenario Support Matrix

| Scenario | Status | Required Features | Blocked By |
|----------|--------|------------------|------------|
| **1. Health & Status** | ✅ Works now | None | - |
| **2. Product Browsing** | 🔴 Blocked | #27 (extract product_id), #30 (assertions) | #26, #27, #30 |
| **3. Auth Flow** | 🔴 Blocked | #28 (tokens), #27 (extract), #30 (assert) | #26, #27, #28, #30 |
| **4. Shopping Flow** | 🔴 Blocked | All Wave 1+2 features | #26-30 |
| **5. Cart Operations** | 🔴 Blocked | #28, #27, #32 (PUT/DELETE), #30 | #26-28, #30, #32 |
| **6. Order Management** | 🔴 Blocked | #26, #27, #28, #30 | #26-28, #30 |
| **7. Search & Filter** | 🔴 Blocked | #27, #30 | #26, #27, #30 |
| **8. Streaming/WebSocket** | ⏸️ Future | Phase 5 work | TBD |
| **9. Response Variations** | ✅ Works now | None | - |
| **10. Error Handling** | 🟡 Partial | #34 (categorization), #30 (assert) | #34, #30 |
| **11. Mixed Traffic** | 🔴 Blocked | All Phase 1 features | All |
| **12. Stress Testing** | 🟡 Partial | #33 (percentiles critical) | #33 + all |

**Legend:**
- ✅ Works now - Can test today
- 🟡 Partial - Works but missing features
- 🔴 Blocked - Cannot test until features complete
- ⏸️ Future - Planned for later phase

---

## Success Criteria

Phase 1 is complete when:

- [x] All 11 Phase 1 issues (#26-36) are closed
- [ ] Can execute Scenario 4 (Complete Shopping Flow) end-to-end
- [ ] Can extract variables (product_id, token, cart_id) across steps
- [ ] Can authenticate and maintain session across requests
- [ ] Can assert on response content and status codes
- [ ] Percentile latencies (P50, P90, P95, P99) are tracked and reported
- [ ] All tests passing (>79 tests)
- [ ] Documentation updated with scenario examples
- [ ] LOAD_TEST_SCENARIOS.md scenarios 1-7, 9-12 can be implemented

---

## Architecture Changes

### New Modules (Planned)
```
src/
  scenario.rs       - Scenario and Step definitions
  executor.rs       - Scenario execution engine
  extractor.rs      - Variable extraction (JSON/Regex/XML)
  assertions.rs     - Response assertion framework
  session.rs        - Cookie jar and session management
  data_source.rs    - CSV data loading
```

### Updated Modules
```
src/
  config.rs         - Add scenario configuration support
  metrics.rs        - Add percentile tracking, error categorization
  worker.rs         - Integrate scenario execution
  client.rs         - Add cookie handling, all HTTP methods
```

---

## Timeline

| Week | Focus | Issues | Deliverable |
|------|-------|--------|-------------|
| 1-2 | Scenario Engine | #26 | Can execute multi-step flows |
| 3 | Variables & Sessions | #27, #28 | Can chain requests with extracted data |
| 4 | Assertions & Delays | #30, #29 | Can validate responses and add think times |
| 5 | Metrics & Methods | #33, #32 | Percentiles tracked, all HTTP methods |
| 6 | Data & Errors | #31, #34 | CSV support, error categorization |
| 7 | Final Polish | #35, #36 | Per-scenario metrics, connection stats |

---

## Testing Strategy

### Unit Tests
- Each module has comprehensive unit tests
- Mock HTTP responses for deterministic testing
- Edge cases: empty responses, malformed JSON, network errors

### Integration Tests
- 3-step flow: login → get data → logout
- Shopping flow: browse → add to cart → checkout
- Error scenarios: 404s, 500s, timeouts

### Manual Testing
- Run against https://ecom.edge.baugus-lab.com
- Validate all 12 scenarios from LOAD_TEST_SCENARIOS.md
- Performance testing: 100+ RPS sustained

---

## Notes

- **Long-lived branch**: `develop/phase1-scenario-engine` will be maintained for several months
- **Individual PRs**: Each issue gets its own feature branch → PR → merge to develop
- **Stability**: Merge develop → main only when stable and tested
- **Phase 2 timeline**: Start after Phase 1 complete (~Week 8)
- **Migration**: This file will be merged into `OVERALL_PROGRESS.md` when Phase 1 complete

---

---

## Recent Progress (2026-02-11)

### Issue #26: Multi-step Scenario Engine - 70% Complete ✓

**Completed:**
- ✅ Created `src/scenario.rs` with core data structures
  - Scenario, Step, RequestConfig, ScenarioContext
  - VariableExtraction, Extractor enum (JSONPath, Regex, Header, Cookie)
  - Assertion enum (StatusCode, ResponseTime, JsonPath, BodyContains, BodyMatches)
- ✅ Created `src/executor.rs` with execution engine
  - ScenarioExecutor with sequential step execution
  - StepResult and ScenarioResult for tracking
  - Variable substitution in all request fields
  - Error handling with early termination
- ✅ Variable substitution system
  - ${var} and $var syntax support
  - Special ${timestamp} variable for unique IDs
  - Works in paths, body, headers
- ✅ Unit tests (9 tests)
  - Variable storage and retrieval
  - Substitution in various contexts
  - Step counter and context reset
- ✅ Example scenario (examples/scenario_example.rs)
  - 6-step shopping flow
  - Demonstrates variable extraction and substitution
  - Shows auth token usage across steps

**Remaining Work (30%):**
- [ ] Integration tests with real HTTP calls
- [ ] Wire into worker.rs for load generation
- [ ] Add scenario metrics to metrics.rs
- [ ] Update config.rs to support scenario configuration
- [ ] Documentation and usage guide
- [ ] Complete validation against LOAD_TEST_SCENARIOS.md

**Blockers:**
- Variable extraction (#27) - Extractor is defined but not implemented
- Assertions (#30) - Assertion is defined but not executed
- These will be addressed in Wave 1 as separate issues

---

**Last Updated**: 2026-02-11 15:45 PST
**Status**: 🚧 In Progress (Week 1 - Issue #26 at 70%)
**Next Milestone**: Complete Issue #26 integration and testing
