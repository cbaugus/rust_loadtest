# Configuration Versioning

## Overview

rust-loadtest uses semantic versioning for YAML configuration files. This enables:
- **Version validation** - Ensure config files are compatible with current tool version
- **Forward/backward compatibility** - Clear error messages for incompatible versions
- **Migration framework** - Automated migration path for config schema changes
- **Future-proof design** - Prepared for schema evolution over time

## Version Format

Configuration versions use **major.minor** format:
- **Major version**: Breaking changes, incompatible schema modifications
- **Minor version**: Backward-compatible additions and enhancements

Examples: `1.0`, `1.1`, `2.0`, `2.5`

## Current Version

- **Current**: `1.0`
- **Minimum Supported**: `1.0`
- **Maximum Supported**: `1.0`

## Version in YAML

Every YAML configuration file must declare its version:

```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/health"
```

## Version Validation

### Supported Versions

The tool validates that config file versions fall within the supported range:

```rust
// Supported range check
if version < MINIMUM_SUPPORTED {
    Error: "Version 0.5 is too old. Minimum supported version: 1.0"
}

if version > MAXIMUM_SUPPORTED {
    Error: "Version 2.0 is too new. Maximum supported version: 1.0"
}
```

### Invalid Format

Version strings must follow `major.minor` format:

**✅ Valid:**
- `"1.0"`
- `"2.5"`
- `"10.99"`

**❌ Invalid:**
- `"1"` - Missing minor version
- `"1.0.0"` - Patch version not allowed
- `"invalid"` - Not a number
- `"1.x"` - Non-numeric component

### Error Messages

Version errors provide clear, actionable messages:

```
Invalid version format: 1.0.0. Expected format: X.Y (e.g., 1.0, 2.1)

Unsupported version: 2.0. Supported versions: 1.0

Version 0.5 is too old. Minimum supported version: 1.0

Version 3.0 is too new. Maximum supported version: 1.0
```

## Migration Framework

### Overview

When config schemas evolve, the migration framework automates version upgrades:

```
Version 1.0 → Migration → Version 2.0 → Migration → Version 3.0
```

### Migration Registry

Migrations are registered and applied automatically:

```rust
// Register a migration
let mut registry = MigrationRegistry::default_migrations();
registry.register(Box::new(MigrationV1ToV2));

// Apply migration
let upgraded_yaml = registry.migrate(
    original_yaml,
    &Version::new(1, 0),
    &Version::new(2, 0)
)?;
```

### Creating Migrations

Implement the `Migration` trait:

```rust
use rust_loadtest::config_version::{Migration, Version, VersionError};

struct MigrationV1ToV2;

impl Migration for MigrationV1ToV2 {
    fn from_version(&self) -> Version {
        Version::new(1, 0)
    }

    fn to_version(&self) -> Version {
        Version::new(2, 0)
    }

    fn description(&self) -> &str {
        "Add authentication section and rename baseUrl to base_url"
    }

    fn migrate(&self, yaml: &str) -> Result<String, VersionError> {
        // Parse YAML
        let mut config: serde_yaml::Value = serde_yaml::from_str(yaml)?;

        // Update version
        config["version"] = serde_yaml::Value::String("2.0".to_string());

        // Add new auth section
        config["auth"] = serde_yaml::Value::Mapping(Default::default());

        // Rename field
        if let Some(base_url) = config["config"]["baseUrl"].take() {
            config["config"]["base_url"] = base_url;
        }

        // Serialize back to YAML
        Ok(serde_yaml::to_string(&config)?)
    }
}
```

### Migration Best Practices

1. **Make migrations idempotent** - Running twice should produce same result
2. **Preserve data** - Don't lose user configuration data
3. **Validate after migration** - Ensure output is valid for target version
4. **Test thoroughly** - Cover edge cases and malformed configs
5. **Document changes** - Clear description of what changed

## Version Evolution Plan

### Version 1.0 (Current)

Initial release with:
- Basic YAML configuration
- Global config section
- Load models (concurrent, rps, ramp, dailytraffic)
- Scenario definitions
- Steps with requests, assertions, extractors

### Version 1.1 (Future)

Potential backward-compatible additions:
- Authentication section (API keys, OAuth, JWT)
- Advanced data sources (databases, APIs)
- Conditional logic in scenarios
- Variable scoping and namespaces
- Test hooks (before/after test, before/after scenario)

### Version 2.0 (Future)

Potential breaking changes:
- Restructured config schema
- New required fields
- Deprecated old load model syntax
- Enhanced scenario format

## Checking Version Compatibility

### From Code

```rust
use rust_loadtest::config_version::{Version, VersionChecker};

// Parse and validate
let version = VersionChecker::parse_and_validate("1.0")?;

// Check compatibility
match VersionChecker::check_compatibility(&version)? {
    None => println!("Version is current, no migration needed"),
    Some(migration_path) => {
        println!("Migration needed:");
        for target in migration_path {
            println!("  → {}", target);
        }
    }
}
```

### Version Info

Get current version information:

```rust
use rust_loadtest::config_version::VersionInfo;

println!("Current version: {}", VersionInfo::current());
println!("Supported range: {} to {}",
    VersionInfo::minimum_supported(),
    VersionInfo::maximum_supported()
);
```

## Migration Examples

### Example 1: Field Rename

**Config v1.0:**
```yaml
version: "1.0"
config:
  baseUrl: "https://api.example.com"
```

**Config v2.0 (hypothetical):**
```yaml
version: "2.0"
config:
  base_url: "https://api.example.com"  # Renamed for consistency
```

**Migration:**
```rust
config["config"]["base_url"] = config["config"]["baseUrl"].take();
```

### Example 2: Add Required Field

**Config v1.0:**
```yaml
version: "1.0"
load:
  model: "rps"
  target: 100
```

**Config v2.0 (hypothetical):**
```yaml
version: "2.0"
load:
  model: "rps"
  target: 100
  distribution: "uniform"  # New required field
```

**Migration:**
```rust
if config["load"]["model"] == "rps" {
    // Add default value for new required field
    config["load"]["distribution"] = Value::String("uniform".to_string());
}
```

### Example 3: Restructure Section

**Config v1.0:**
```yaml
version: "1.0"
config:
  timeout: "30s"
  workers: 10
```

**Config v2.0 (hypothetical):**
```yaml
version: "2.0"
config:
  execution:
    timeout: "30s"
    workers: 10
```

**Migration:**
```rust
let mut execution = Mapping::new();
execution.insert("timeout", config["config"]["timeout"].take());
execution.insert("workers", config["config"]["workers"].take());
config["config"]["execution"] = Value::Mapping(execution);
```

## Error Handling

### Unsupported Version

```yaml
version: "3.0"  # Not yet released
config:
  baseUrl: "https://test.com"
  duration: "5m"
```

**Error:**
```
YAML config error: Invalid configuration: version: Version 3.0 is too new.
Maximum supported version: 1.0
```

### Invalid Format

```yaml
version: "1.0.0"  # Three-part version not allowed
config:
  baseUrl: "https://test.com"
```

**Error:**
```
YAML config error: Invalid configuration: version: Invalid version format: 1.0.0.
Expected format: X.Y (e.g., 1.0, 2.1)
```

## Testing Version Compatibility

### Test Current Version

```yaml
version: "1.0"
config:
  baseUrl: "https://test.com"
  duration: "5m"
load:
  model: "concurrent"
scenarios:
  - name: "Test"
    steps:
      - request:
          method: "GET"
          path: "/"
```

**Result:** ✅ Loads successfully

### Test Future Version

```yaml
version: "2.0"
config:
  baseUrl: "https://test.com"
```

**Result:** ❌ Error: "Version 2.0 is too new"

### Test Old Version

```yaml
version: "0.5"
config:
  baseUrl: "https://test.com"
```

**Result:** ❌ Error: "Version 0.5 is too old"

## CLI Integration

### Check Config Version

```bash
# Validate config version
rust-loadtest --config test.yaml --validate-version

# Output:
# Config version: 1.0
# Status: ✅ Supported
# Current tool version: 1.0
```

### Migrate Config

```bash
# Auto-migrate config to current version
rust-loadtest --config test.yaml --migrate

# Output:
# Migrating from 1.0 to 2.0...
# Migration: Add authentication section
# ✅ Migration successful
# Updated config written to: test.v2.0.yaml
```

## FAQ

### Q: What happens if I use an unsupported version?

**A:** The tool will refuse to load the config and display a clear error message indicating the supported version range.

### Q: Can I downgrade a config file to an older version?

**A:** No. Migrations only support upgrading forward. Downgrading could lose data from newer features.

### Q: Will my v1.0 configs continue to work forever?

**A:** Yes, within reason. We maintain backward compatibility for at least 2 major versions. When v3.0 is released, v1.0 support may be deprecated with a clear migration path.

### Q: How do I know if a migration is needed?

**A:** The tool automatically detects version mismatches. If your config version is older than the current version, a migration path will be suggested.

### Q: What if migration fails?

**A:** Migration errors provide detailed information about what failed. You may need to manually update certain fields or fix malformed config before migration can succeed.

### Q: Can I skip version validation?

**A:** No. Version validation is mandatory to ensure config compatibility and prevent runtime errors from incompatible schemas.

## Related Documentation

- [YAML Configuration Guide](/docs/YAML_CONFIG.md)
- [Configuration Precedence](/docs/CONFIGURATION_PRECEDENCE.md)
- [Environment Variable Overrides](/docs/ENV_VAR_OVERRIDES.md)
- [Migration Guide](/docs/MIGRATION_GUIDE.md)

## Version History

| Version | Release Date | Major Changes |
|---------|--------------|---------------|
| 1.0     | 2026-02 | Initial release with YAML config support |

## Future Roadmap

### Version 1.1 (Planned)

- Authentication section
- Advanced data sources
- Conditional logic
- Test hooks

### Version 2.0 (Planned)

- Restructured schema
- Enhanced scenario format
- Plugin system
- Distributed testing support
