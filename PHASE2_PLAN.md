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

### 🚧 In Progress
_None - Wave 1: 1/2 done_

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

- [ ] **Issue #38**: Config schema and validation (P0, L)
  - [ ] Define comprehensive ConfigSchema
  - [ ] Add validation rules (required fields, ranges, formats)
  - [ ] URL validation
  - [ ] Duration format validation
  - [ ] Enum validation (load models, HTTP methods)
  - [ ] Custom validation errors with helpful messages
  - [ ] Unit tests for validation
  - [ ] Integration tests

### 📋 Todo - Wave 2 (Week 2)

- [ ] **Issue #39**: Default value merging (P1, S)
  - [ ] Define default values for all config fields
  - [ ] Implement merge logic (defaults + file + env)
  - [ ] Precedence: env vars > file > defaults
  - [ ] Test precedence order
  - [ ] Document precedence rules

- [ ] **Issue #40**: Environment variable overrides (P0, M)
  - [ ] Map env vars to YAML config paths
  - [ ] Support dot notation (e.g., CONFIG_LOAD_MODEL)
  - [ ] Override specific YAML values with env vars
  - [ ] Maintain backward compatibility
  - [ ] Document override patterns
  - [ ] Integration tests

- [ ] **Issue #41**: Config versioning (P2, M)
  - [ ] Add version field to config
  - [ ] Version detection
  - [ ] Migration framework for v1.0 -> v2.0
  - [ ] Migration tests
  - [ ] Version validation

### 📋 Todo - Wave 3 (Week 3)

- [ ] **Issue #42**: Scenario YAML definitions (P0, XL)
  - [ ] Scenario block in YAML
  - [ ] Multiple scenarios per file
  - [ ] Scenario weighting for mixed traffic
  - [ ] Step definitions in YAML
  - [ ] Request config in YAML
  - [ ] Assertions in YAML
  - [ ] Extractors in YAML
  - [ ] Think times in YAML
  - [ ] Data files in YAML
  - [ ] Integration with existing executor
  - [ ] Comprehensive tests

- [ ] **Issue #43**: Multi-scenario execution (P0, L)
  - [ ] Load multiple scenarios from config
  - [ ] Weighted scenario selection
  - [ ] Round-robin scenario distribution
  - [ ] Per-scenario worker allocation
  - [ ] Per-scenario metrics
  - [ ] Integration tests

### 📋 Todo - Wave 4 (Week 4)

- [ ] **Issue #44**: Config file hot-reload (P2, S)
  - [ ] File watcher for config changes
  - [ ] Graceful reload without stopping test
  - [ ] Validation before reload
  - [ ] Reload notification/logging
  - [ ] Development mode flag
  - [ ] Tests

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

**Last Updated**: 2026-02-14 15:45 PST
**Status**: ✅ Wave 1: 1/2 complete! Issue #37 done
**Next Milestone**: Issue #38 (Config Schema and Validation)
**Branch Status**: phase2-advanced-features (active development)
