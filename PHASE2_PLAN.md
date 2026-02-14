# Phase 2: Configuration System - Implementation Plan

**Branch**: `phase2-advanced-features`
**Duration**: ~4 weeks (estimated)
**Target**: Replace environment variables with declarative YAML configuration files

---

## Overview

Phase 2 transforms the rust-loadtest tool from environment variable configuration to a declarative YAML-based configuration system. This enables version-controlled test plans, reusable scenarios, and eliminates the need for complex environment setups.

### Key Capabilities to Add:
- YAML configuration file support
- Comprehensive config schema with validation
- Default value merging
- Environment variable overrides (backward compatibility)
- Config versioning and migration
- Scenario definitions in YAML
- Multiple scenario support per test run
- Config file hot-reload (development mode)

### Configuration Format:
```yaml
version: "1.0"
metadata:
  name: "E-commerce Load Test"
  description: "Full checkout flow testing"
  tags: ["production", "critical"]

config:
  baseUrl: "https://shop.example.com"
  timeout: 30s
  workers: 10
  duration: 10m

load:
  model: ramp
  rps:
    min: 10
    max: 100
  rampDuration: 2m

scenarios:
  - name: "Browse and Purchase"
    weight: 70
    steps:
      - name: "Homepage"
        request:
          method: GET
          path: "/"
        assertions:
          - statusCode: 200
        thinkTime: 2s

      - name: "Search"
        request:
          method: GET
          path: "/search?q=laptop"
        extract:
          - name: productId
            jsonPath: "$.products[0].id"
        thinkTime: 3s
```

---

## Implementation Waves

### Wave 1: Core YAML Support (Week 1)
Basic YAML parsing and schema validation.

### Wave 2: Advanced Config Features (Week 2)
Default merging, env overrides, and validation.

### Wave 3: Scenario YAML Integration (Week 3)
Load scenarios from YAML files.

### Wave 4: Polish & Migration (Week 4)
Hot-reload, migration tools, documentation.

---

## Issues and Progress Tracker

### ✅ Completed
- [x] **Issue #37**: YAML config file parser (P0, M) - **COMPLETE** ✅
  - Branch: `feature/issue-37-yaml-config-parser` (merged to phase2)
  - 629 lines of implementation + 705 lines of tests
  - Full YAML parsing with validation
  - 22 comprehensive integration tests
- [x] **Issue #38**: Config schema and validation (P0, L) - **COMPLETE** ✅
  - Branch: `feature/issue-38-config-schema-validation` (merged to phase2)
  - 540 lines of validation + 569 lines of tests
  - Enhanced validation with field-level errors
  - JSON Schema export for tooling
  - 24 comprehensive tests
- [x] **Issue #39**: Default value merging (P1, S) - **COMPLETE** ✅
  - Branch: `feature/issue-39-default-value-merging` (merged to phase2)
  - 306 lines of implementation + 227 lines of unit tests + 375 lines of integration tests
  - ConfigDefaults with default values (workers: 10, timeout: 30s, etc.)
  - ConfigMerger implementing precedence (env > yaml > default)
  - ConfigPrecedence with comprehensive documentation
  - 35 comprehensive tests (17 unit + 18 integration)
- [x] **Issue #40**: Environment variable overrides (P0, M) - **COMPLETE** ✅
  - Branch: `feature/issue-40-env-var-overrides` (merged to phase2)
  - 161 lines of implementation + 599 lines of tests + 348 lines of docs
  - Config::from_yaml_with_env_overrides() method
  - Complete env var mapping for all config fields
  - Load model parameter and complete override support
  - Invalid/empty env value fallback to YAML
  - 20 comprehensive integration tests
  - Full documentation with CI/CD patterns
- [x] **Issue #41**: Config versioning (P2, M) - **COMPLETE** ✅
  - Branch: `feature/issue-41-config-versioning` (merged to phase2)
  - 463 lines of implementation + 542 lines of tests + 461 lines of docs
  - Version struct with semantic versioning (major.minor)
  - VersionChecker for compatibility validation
  - Migration trait and MigrationRegistry framework
  - Integrated with YamlConfig validation
  - 55 comprehensive tests (30 unit + 25 integration)
  - Complete versioning guide with migration examples
- [x] **Issue #42**: Scenario YAML definitions (P0, XL) - **COMPLETE** ✅
  - Branch: `feature/issue-42-scenario-yaml-definitions` (merged to phase2)
  - 78 lines of implementation + 695 lines of tests + 686 lines of docs
  - Data file support (CSV, JSON) with strategies (sequential, random, cycle)
  - Random think time (min/max range) for realistic user behavior
  - Scenario-level config overrides (timeout, retry logic)
  - Enhanced YamlScenario with dataFile and config fields
  - 23 comprehensive integration tests
  - Complete scenario guide with real-world examples
- [x] **Issue #43**: Multi-scenario execution (P0, L) - **COMPLETE** ✅
  - Branch: `feature/issue-43-multi-scenario-execution` (merged to phase2)
  - 512 lines of implementation + 523 lines of tests + 514 lines of docs
  - ScenarioSelector for weighted random selection
  - RoundRobinDistributor for even distribution
  - ScenarioMetrics for per-scenario tracking
  - Thread-safe atomic counters
  - 44 comprehensive tests (10 unit + 34 integration)
  - Complete multi-scenario guide with real-world examples
- [x] **Issue #44**: Config file hot-reload (P2, S) - **COMPLETE** ✅
  - Branch: `feature/issue-44-config-hot-reload` (merged to phase2)
  - 571 lines of implementation + 504 lines of tests + 661 lines of docs
  - ConfigWatcher for file watching with notify crate
  - HotReloadConfig for hot-reload behavior control
  - ReloadNotifier for event-based config change handling
  - Debouncing to prevent multiple reloads for rapid changes
  - Full validation before applying config changes
  - 22 comprehensive integration tests
  - Complete hot-reload guide with examples and best practices

### 🚧 In Progress
_None - 🎉 ✅ Wave 4 in progress (1/3 done)_

### 📋 Todo - Wave 1 (Week 1)

- [x] **Issue #37**: YAML config file parser (P0, M) ✅
  - [x] Add serde_yaml dependency
  - [x] Create Config struct for YAML format
  - [x] Implement from_yaml() method
  - [x] Support loading from file path
  - [x] Support loading from string (testing)
  - [x] Backward compatibility with env vars (ready)
  - [x] Unit tests for YAML parsing
  - [x] Integration tests

- [x] **Issue #38**: Config schema and validation (P0, L) ✅
  - [x] Define comprehensive ConfigSchema
  - [x] Add validation rules (required fields, ranges, formats)
  - [x] URL validation
  - [x] Duration format validation
  - [x] Enum validation (load models, HTTP methods)
  - [x] Custom validation errors with helpful messages
  - [x] Unit tests for validation
  - [x] Integration tests

### 📋 Todo - Wave 2 (Week 2)

- [x] **Issue #39**: Default value merging (P1, S) ✅
  - [x] Define default values for all config fields
  - [x] Implement merge logic (defaults + file + env)
  - [x] Precedence: env vars > file > defaults
  - [x] Test precedence order
  - [x] Document precedence rules

- [x] **Issue #40**: Environment variable overrides (P0, M) ✅
  - [x] Map env vars to YAML config paths
  - [x] Support dot notation (e.g., CONFIG_LOAD_MODEL)
  - [x] Override specific YAML values with env vars
  - [x] Maintain backward compatibility
  - [x] Document override patterns
  - [x] Integration tests

- [x] **Issue #41**: Config versioning (P2, M) ✅
  - [x] Add version field to config
  - [x] Version detection
  - [x] Migration framework for v1.0 -> v2.0
  - [x] Migration tests
  - [x] Version validation

### 📋 Todo - Wave 3 (Week 3)

- [x] **Issue #42**: Scenario YAML definitions (P0, XL) ✅
  - [x] Scenario block in YAML
  - [x] Multiple scenarios per file
  - [x] Scenario weighting for mixed traffic
  - [x] Step definitions in YAML
  - [x] Request config in YAML
  - [x] Assertions in YAML
  - [x] Extractors in YAML
  - [x] Think times in YAML (fixed and random)
  - [x] Data files in YAML (CSV, JSON)
  - [x] Integration with existing executor
  - [x] Comprehensive tests

- [x] **Issue #43**: Multi-scenario execution (P0, L) ✅
  - [x] Load multiple scenarios from config
  - [x] Weighted scenario selection
  - [x] Round-robin scenario distribution
  - [x] Per-scenario worker allocation
  - [x] Per-scenario metrics
  - [x] Integration tests

### 📋 Todo - Wave 4 (Week 4)

- [x] **Issue #44**: Config file hot-reload (P2, S) ✅
  - [x] File watcher for config changes
  - [x] Graceful reload without stopping test
  - [x] Validation before reload
  - [x] Reload notification/logging
  - [x] Development mode flag
  - [x] Tests

- [ ] **Issue #45**: Config examples and templates (P1, S)
  - [ ] Create example YAML configs
  - [ ] Basic API test template
  - [ ] E-commerce scenario template
  - [ ] Stress test template
  - [ ] Documentation for each template
  - [ ] Template validation

- [ ] **Issue #46**: Config documentation generator (P2, M)
  - [ ] Auto-generate schema docs from code
  - [ ] JSON Schema export
  - [ ] Markdown documentation
  - [ ] VS Code snippet generation
  - [ ] Documentation tests

---

## Architecture Changes

### New Modules (Planned)
```
src/
  config/
    mod.rs           - Config module root
    yaml.rs          - YAML parsing
    schema.rs        - Config schema and validation
    merge.rs         - Default merging logic
    migration.rs     - Version migration
    examples.rs      - Built-in templates
```

### Updated Modules
```
src/
  config.rs         - Extend to support YAML loading
  main.rs           - Load config from file or env
  scenario.rs       - YAML deserialization
```

---

## Timeline

| Week | Focus | Issues | Deliverable |
|------|-------|--------|-------------|
| 1 | YAML Parsing | #37, #38 | Can load and validate YAML configs |
| 2 | Advanced Config | #39, #40, #41 | Defaults, overrides, versioning work |
| 3 | Scenarios | #42, #43 | Multi-scenario YAML execution |
| 4 | Polish | #44, #45, #46 | Hot-reload, templates, docs |

---

## Testing Strategy

### Unit Tests
- YAML parsing with various formats
- Schema validation with invalid inputs
- Default merging logic
- Environment override precedence
- Version migration

### Integration Tests
- Load full YAML config and execute test
- Multi-scenario execution with weighting
- Override YAML with environment variables
- Hot-reload during test execution
- Template validation

### Example Configs
- Simple single-endpoint test
- Multi-step scenario test
- Mixed traffic with multiple scenarios
- Data-driven test with CSV
- Stress test with ramping

---

## Success Criteria

Phase 2 is complete when:

- [ ] Can load complete test configuration from YAML file
- [ ] Can define multi-step scenarios in YAML
- [ ] Can run multiple scenarios with weighted distribution
- [ ] Environment variables can override YAML values
- [ ] Config validation provides helpful error messages
- [ ] Default values work for all optional fields
- [ ] Config versioning and migration works
- [ ] All tests passing (50+ new tests)
- [ ] Documentation includes YAML examples
- [ ] Backward compatibility maintained

---

## Dependencies

**New Rust Crates:**
```toml
serde_yaml = "0.9"           # YAML parsing
serde = { version = "1.0", features = ["derive"] }
validator = "0.16"           # Schema validation
notify = "6.0"               # File watching (hot-reload)
```

---

## Migration Strategy

### Backward Compatibility

Phase 2 must maintain 100% backward compatibility with Phase 1:
- All environment variables continue to work
- If no YAML file provided, use env vars (current behavior)
- If YAML file provided, env vars can override specific values
- Existing tests and deployments continue working

### Migration Path for Users

**Step 1: Generate config from current env vars**
```bash
rust-loadtest --generate-config > loadtest.yaml
```

**Step 2: Review and customize YAML**
```bash
vim loadtest.yaml
```

**Step 3: Run with YAML config**
```bash
rust-loadtest --config loadtest.yaml
```

**Step 4: Override specific values**
```bash
TARGET_RPS=500 rust-loadtest --config loadtest.yaml
```

---

## Example YAML Configs

### Simple API Test
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  workers: 10
  duration: 5m

load:
  model: rps
  target: 100

scenarios:
  - name: "API Health Check"
    steps:
      - request:
          method: GET
          path: "/health"
        assertions:
          - statusCode: 200
```

### E-commerce Flow
```yaml
version: "1.0"
config:
  baseUrl: "https://shop.example.com"
  workers: 50
  duration: 30m

load:
  model: ramp
  rps:
    min: 10
    max: 200
  rampDuration: 5m

scenarios:
  - name: "Browse and Purchase"
    weight: 70
    steps:
      - name: "Homepage"
        request:
          method: GET
          path: "/"
        thinkTime: 2s

      - name: "Search"
        request:
          method: GET
          path: "/search?q=laptop"
        extract:
          - name: productId
            jsonPath: "$.products[0].id"
        thinkTime: 3s

      - name: "Add to Cart"
        request:
          method: POST
          path: "/cart"
          body: '{"productId": "${productId}"}'
        assertions:
          - statusCode: 201

  - name: "Quick Browse"
    weight: 30
    steps:
      - request:
          method: GET
          path: "/"
```

---

## Notes

- **Backward Compatibility**: Critical - existing users must not break
- **Validation**: Provide clear, actionable error messages
- **Documentation**: Every YAML field must be documented
- **Examples**: Provide real-world config examples
- **Testing**: 50+ tests to ensure quality
- **Performance**: YAML parsing should add <10ms overhead

---

**Last Updated**: 2026-02-11 (continued)
**Status**: 🚀 Wave 4 in progress (1/3 issues done)
**Next Milestone**: Wave 4 - Issue #45 (Config Examples and Templates)
**Branch Status**: phase2-advanced-features (active development)
