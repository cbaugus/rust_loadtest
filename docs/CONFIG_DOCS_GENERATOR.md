# Configuration Documentation Generator

## Overview

The configuration documentation generator automatically generates schema documentation, JSON Schema files, and IDE snippets from the configuration structures. This ensures documentation stays in sync with the code.

## Features

✅ **JSON Schema Generation** - Exports complete JSON Schema for IDE support and validation
✅ **Markdown Documentation** - Auto-generates reference documentation
✅ **VS Code Snippets** - Creates code snippets for faster config authoring
✅ **Auto-sync** - Documentation generated from code, always up-to-date
✅ **IDE Integration** - JSON Schema enables auto-completion in IDEs

## Usage

### Programmatic API

```rust
use rust_loadtest::config_docs_generator::ConfigDocsGenerator;
use std::fs;

let generator = ConfigDocsGenerator::new();

// Generate JSON Schema
let json_schema = generator.generate_json_schema();
fs::write("schema.json", json_schema)?;

// Generate Markdown docs
let markdown = generator.generate_markdown_docs();
fs::write("CONFIG_SCHEMA.md", markdown)?;

// Generate VS Code snippets
let snippets = generator.generate_vscode_snippets();
fs::write("snippets.json", snippets)?;
```

### Using the Generator Script

```bash
# Run the documentation generator
cargo run --example generate_docs

# This creates:
#   - docs/config-schema.json
#   - docs/CONFIG_SCHEMA.md
#   - .vscode/rust-loadtest.code-snippets
```

## Generated Files

### 1. JSON Schema (`config-schema.json`)

**Purpose**: Machine-readable schema for validation and IDE support

**Features**:
- Complete type definitions
- Validation rules (required fields, patterns, ranges)
- Examples for each field
- Enum values for constrained fields
- Format specifications

**Usage**:

**VS Code** - Add to `settings.json`:
```json
{
  "yaml.schemas": {
    "./docs/config-schema.json": "loadtest*.yaml"
  }
}
```

**IntelliJ/PyCharm** - Settings → Languages & Frameworks → Schemas and DTDs → JSON Schema Mappings

**Schema Validators**:
```bash
# Validate with ajv-cli
npm install -g ajv-cli
ajv validate -s docs/config-schema.json -d loadtest.yaml

# Validate with Python
pip install jsonschema pyyaml
python -c "import yaml, jsonschema; jsonschema.validate(yaml.safe_load(open('loadtest.yaml')), json.load(open('docs/config-schema.json')))"
```

### 2. Markdown Documentation (`CONFIG_SCHEMA.md`)

**Purpose**: Human-readable reference documentation

**Sections**:
- Version - Configuration versioning
- Metadata - Test metadata fields
- Config - Global configuration
- Load Models - Concurrent, RPS, Ramp models
- Scenarios - Scenario and step definitions
- Complete Example - Full working example

**Features**:
- Property tables
- Type information
- Required/optional indicators
- Default values
- YAML examples for each section

### 3. VS Code Snippets (`rust-loadtest.code-snippets`)

**Purpose**: Code snippets for faster YAML authoring

**Available Snippets**:

| Prefix | Description | Result |
|--------|-------------|--------|
| `loadtest-basic` | Complete basic config | Full config template |
| `loadtest-rps` | RPS load model | RPS configuration |
| `loadtest-ramp` | Ramp load model | Ramp configuration |
| `loadtest-scenario` | Test scenario | Scenario with steps |
| `loadtest-step` | Test step | Step with request |
| `loadtest-assertion-status` | Status assertion | Status code check |
| `loadtest-assertion-jsonpath` | JSONPath assertion | JSONPath validation |
| `loadtest-extract-jsonpath` | JSONPath extractor | Variable extraction |
| `loadtest-datafile` | Data file config | CSV/JSON data file |

**Usage in VS Code**:
1. Open YAML file
2. Type snippet prefix (e.g., `loadtest-basic`)
3. Press `Tab` to expand
4. Use `Tab` to navigate placeholders

## JSON Schema Details

### Schema Structure

```json
{
  "$schema": "http://json-schema.org/draft-07/schema#",
  "title": "Rust LoadTest Configuration",
  "type": "object",
  "required": ["version", "config", "load", "scenarios"],
  "properties": {
    "version": { ... },
    "config": { ... },
    "load": { ... },
    "scenarios": { ... }
  }
}
```

### Type Definitions

**Duration Fields**:
```json
{
  "oneOf": [
    { "type": "string", "pattern": "^[0-9]+(s|m|h)$" },
    { "type": "integer", "minimum": 1 }
  ]
}
```

**Load Model Union**:
```json
{
  "oneOf": [
    { "properties": { "model": { "const": "concurrent" } } },
    { "properties": { "model": { "const": "rps" }, "target": {...} } },
    { "properties": { "model": { "const": "ramp" }, "min": {...}, "max": {...} } }
  ]
}
```

### Validation Rules

- **Required Fields**: `version`, `config`, `load`, `scenarios`
- **Version Pattern**: `^[0-9]+\.[0-9]+$` (e.g., "1.0")
- **Duration Pattern**: `^[0-9]+(s|m|h)$` (e.g., "5m")
- **Workers Minimum**: 1
- **RPS Minimum**: 0.1
- **HTTP Methods**: GET, POST, PUT, DELETE, PATCH, HEAD, OPTIONS

## IDE Integration

### VS Code

**Setup**:
1. Install YAML extension
2. Add to `.vscode/settings.json`:
```json
{
  "yaml.schemas": {
    "./docs/config-schema.json": "*.yaml"
  }
}
```

**Features**:
- ✅ Auto-completion
- ✅ Field descriptions on hover
- ✅ Error highlighting
- ✅ Enum value suggestions
- ✅ Format validation

### IntelliJ IDEA / PyCharm

**Setup**:
1. Settings → Languages & Frameworks → Schemas and DTDs
2. Add new JSON Schema mapping
3. Schema file: `docs/config-schema.json`
4. File pattern: `*.yaml`

### Vim/Neovim

**With CoC.nvim**:
```json
{
  "yaml.schemas": {
    "/path/to/docs/config-schema.json": "*.yaml"
  }
}
```

**With ALE**:
```vim
let g:ale_yaml_schemas = {
  \ '/path/to/docs/config-schema.json': '*.yaml'
  \ }
```

## Regenerating Documentation

Documentation should be regenerated when:
- Configuration structures change
- New fields are added
- Validation rules update
- Examples need updating

**Regenerate**:
```bash
cargo run --example generate_docs
```

**Automated Regeneration** (in CI/CD):
```yaml
# GitHub Actions example
- name: Generate Docs
  run: |
    cargo run --example generate_docs
    git diff --exit-code || echo "Docs need updating"
```

## Customization

### Adding New Snippets

Edit `src/config_docs_generator.rs`:

```rust
snippets.insert("loadtest-custom", serde_json::json!({
    "prefix": "loadtest-custom",
    "body": [
        "your:",
        "  custom: ${1:value}"
    ],
    "description": "Custom snippet"
}));
```

### Extending JSON Schema

Modify `build_json_schema()` method:

```rust
"properties": {
    "newField": {
        "type": "string",
        "description": "New field description",
        "examples": ["example"]
    }
}
```

### Updating Markdown Template

Edit `generate_markdown_docs()` method:

```rust
md.push_str("## New Section\n\n");
md.push_str("Description...\n\n");
```

## Validation

### Schema Validation

```bash
# Validate schema itself
ajv compile -s docs/config-schema.json

# Should output: schema is valid
```

### Config Validation

```bash
# Validate a config file
ajv validate -s docs/config-schema.json -d examples/configs/basic-api-test.yaml

# Or use rust-loadtest
rust-loadtest --config my-config.yaml --validate
```

## Best Practices

### 1. Keep Schema in Sync

Always regenerate docs after schema changes:
```bash
# After modifying YamlConfig structures
cargo run --example generate_docs
git add docs/ .vscode/
git commit -m "Update generated documentation"
```

### 2. Add Examples

Include examples in JSON Schema:
```json
{
  "examples": ["1.0", "2.0"]
}
```

### 3. Descriptive Error Messages

Use clear descriptions for validation:
```json
{
  "description": "Duration in format '5m', '1h', or '30s'"
}
```

### 4. IDE-Friendly Enums

Provide enum values for constrained fields:
```json
{
  "enum": ["GET", "POST", "PUT", "DELETE"]
}
```

### 5. Version Documentation

Update docs when schema version changes:
```rust
version: "2.0".to_string()
```

## Troubleshooting

### IDE Not Showing Completions

1. Check schema file path in settings
2. Verify schema JSON is valid
3. Reload IDE window
4. Check file pattern matches

### Schema Validation Errors

1. Validate schema file itself
2. Check for JSON syntax errors
3. Verify all `$ref` paths resolve

### Snippets Not Working

1. Check snippet file location (`.vscode/`)
2. Verify JSON syntax
3. Reload VS Code
4. Check snippet scope (YAML files)

## Related Documentation

- [YAML Configuration Guide](/docs/YAML_CONFIG.md)
- [Configuration Schema Reference](/docs/CONFIG_SCHEMA.md)
- [Configuration Examples](/docs/CONFIG_EXAMPLES.md)
- [Configuration Validation](/docs/CONFIG_VALIDATION.md)

## API Reference

### ConfigDocsGenerator

```rust
pub struct ConfigDocsGenerator {
    app_name: String,
    version: String,
}

impl ConfigDocsGenerator {
    /// Create new generator
    pub fn new() -> Self;

    /// Generate JSON Schema
    pub fn generate_json_schema(&self) -> String;

    /// Generate Markdown docs
    pub fn generate_markdown_docs(&self) -> String;

    /// Generate VS Code snippets
    pub fn generate_vscode_snippets(&self) -> String;
}
```

## Contributing

To improve the documentation generator:

1. Modify `src/config_docs_generator.rs`
2. Add tests to `tests/config_docs_generator_tests.rs`
3. Regenerate docs: `cargo run --example generate_docs`
4. Update this guide if API changes
5. Submit pull request

## Version History

- **v1.0** - Initial documentation generator
  - JSON Schema export
  - Markdown documentation
  - VS Code snippets
  - 9 built-in snippets
