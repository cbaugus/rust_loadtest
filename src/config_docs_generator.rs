//! Configuration documentation generator (Issue #46).
//!
//! This module provides automatic documentation generation from config structures:
//! - JSON Schema export
//! - Markdown documentation
//! - VS Code snippets
//!
//! # Example
//! ```no_run
//! use rust_loadtest::config_docs_generator::ConfigDocsGenerator;
//!
//! let generator = ConfigDocsGenerator::new();
//!
//! // Generate JSON Schema
//! let json_schema = generator.generate_json_schema();
//! std::fs::write("schema.json", json_schema).unwrap();
//!
//! // Generate Markdown docs
//! let markdown = generator.generate_markdown_docs();
//! std::fs::write("CONFIG_SCHEMA.md", markdown).unwrap();
//!
//! // Generate VS Code snippets
//! let snippets = generator.generate_vscode_snippets();
//! std::fs::write("snippets.json", snippets).unwrap();
//! ```

use serde_json;
use std::collections::HashMap;

/// Configuration documentation generator.
pub struct ConfigDocsGenerator {
    /// Application name
    app_name: String,

    /// Version
    version: String,
}

impl ConfigDocsGenerator {
    /// Create a new documentation generator.
    pub fn new() -> Self {
        Self {
            app_name: "rust-loadtest".to_string(),
            version: "1.0".to_string(),
        }
    }

    /// Generate JSON Schema for the configuration.
    ///
    /// Produces a JSON Schema that describes the YAML configuration format,
    /// enabling IDE support, validation tools, and documentation generation.
    pub fn generate_json_schema(&self) -> String {
        let schema = self.build_json_schema();
        serde_json::to_string_pretty(&schema).unwrap()
    }

    /// Build the JSON Schema structure.
    fn build_json_schema(&self) -> serde_json::Value {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Rust LoadTest Configuration",
            "description": "YAML configuration schema for rust-loadtest load testing tool",
            "type": "object",
            "required": ["version", "config", "load", "scenarios"],
            "properties": {
                "version": {
                    "type": "string",
                    "description": "Configuration version (semantic versioning)",
                    "pattern": "^[0-9]+\\.[0-9]+$",
                    "examples": ["1.0"]
                },
                "metadata": {
                    "type": "object",
                    "description": "Optional metadata about the test configuration",
                    "properties": {
                        "name": {
                            "type": "string",
                            "description": "Human-readable test name"
                        },
                        "description": {
                            "type": "string",
                            "description": "Test description"
                        },
                        "author": {
                            "type": "string",
                            "description": "Test author"
                        },
                        "tags": {
                            "type": "array",
                            "description": "Tags for categorization",
                            "items": {
                                "type": "string"
                            }
                        }
                    }
                },
                "config": {
                    "type": "object",
                    "description": "Global test configuration",
                    "required": ["baseUrl", "duration"],
                    "properties": {
                        "baseUrl": {
                            "type": "string",
                            "description": "Base URL of the API to test",
                            "format": "uri",
                            "examples": ["https://api.example.com"]
                        },
                        "timeout": {
                            "description": "Request timeout (e.g., '30s', '1m')",
                            "oneOf": [
                                {"type": "string", "pattern": "^[0-9]+(s|m|h)$"},
                                {"type": "integer", "minimum": 1}
                            ],
                            "default": "30s"
                        },
                        "workers": {
                            "type": "integer",
                            "description": "Number of concurrent workers",
                            "minimum": 1,
                            "default": 10
                        },
                        "duration": {
                            "description": "Test duration (e.g., '5m', '1h')",
                            "oneOf": [
                                {"type": "string", "pattern": "^[0-9]+(s|m|h)$"},
                                {"type": "integer", "minimum": 1}
                            ]
                        },
                        "skipTlsVerify": {
                            "type": "boolean",
                            "description": "Skip TLS certificate verification (insecure)",
                            "default": false
                        },
                        "customHeaders": {
                            "type": "string",
                            "description": "Custom HTTP headers (e.g., 'Authorization: Bearer token')"
                        }
                    }
                },
                "load": {
                    "type": "object",
                    "description": "Load model configuration",
                    "required": ["model"],
                    "oneOf": [
                        {
                            "properties": {
                                "model": {"const": "concurrent"},
                            },
                            "required": ["model"]
                        },
                        {
                            "properties": {
                                "model": {"const": "rps"},
                                "target": {
                                    "type": "number",
                                    "description": "Target requests per second",
                                    "minimum": 0.1
                                }
                            },
                            "required": ["model", "target"]
                        },
                        {
                            "properties": {
                                "model": {"const": "ramp"},
                                "min": {
                                    "type": "number",
                                    "description": "Starting RPS",
                                    "minimum": 0.1
                                },
                                "max": {
                                    "type": "number",
                                    "description": "Ending RPS",
                                    "minimum": 0.1
                                },
                                "rampDuration": {
                                    "description": "Ramp duration (e.g., '5m')",
                                    "oneOf": [
                                        {"type": "string", "pattern": "^[0-9]+(s|m|h)$"},
                                        {"type": "integer", "minimum": 1}
                                    ]
                                }
                            },
                            "required": ["model", "min", "max", "rampDuration"]
                        }
                    ]
                },
                "scenarios": {
                    "type": "array",
                    "description": "Test scenarios",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["name", "steps"],
                        "properties": {
                            "name": {
                                "type": "string",
                                "description": "Scenario name"
                            },
                            "weight": {
                                "type": "number",
                                "description": "Scenario weight for traffic distribution",
                                "minimum": 0.1,
                                "default": 100.0
                            },
                            "steps": {
                                "type": "array",
                                "description": "Scenario steps",
                                "minItems": 1,
                                "items": {
                                    "type": "object",
                                    "required": ["request"],
                                    "properties": {
                                        "name": {
                                            "type": "string",
                                            "description": "Step name"
                                        },
                                        "request": {
                                            "type": "object",
                                            "required": ["method", "path"],
                                            "properties": {
                                                "method": {
                                                    "type": "string",
                                                    "enum": ["GET", "POST", "PUT", "DELETE", "PATCH", "HEAD", "OPTIONS"],
                                                    "description": "HTTP method"
                                                },
                                                "path": {
                                                    "type": "string",
                                                    "description": "Request path (relative to baseUrl)"
                                                },
                                                "body": {
                                                    "type": "string",
                                                    "description": "Request body"
                                                },
                                                "headers": {
                                                    "type": "object",
                                                    "description": "Custom request headers",
                                                    "additionalProperties": {"type": "string"}
                                                }
                                            }
                                        },
                                        "thinkTime": {
                                            "description": "Think time after step",
                                            "oneOf": [
                                                {"type": "string", "pattern": "^[0-9]+(s|m|h)$"},
                                                {"type": "integer", "minimum": 0},
                                                {
                                                    "type": "object",
                                                    "properties": {
                                                        "min": {"type": "string"},
                                                        "max": {"type": "string"}
                                                    },
                                                    "required": ["min", "max"]
                                                }
                                            ]
                                        },
                                        "assertions": {
                                            "type": "array",
                                            "description": "Response assertions",
                                            "items": {
                                                "type": "object"
                                            }
                                        },
                                        "extract": {
                                            "type": "array",
                                            "description": "Data extractors",
                                            "items": {
                                                "type": "object",
                                                "properties": {
                                                    "name": {"type": "string"},
                                                    "jsonPath": {"type": "string"},
                                                    "regex": {"type": "string"}
                                                }
                                            }
                                        }
                                    }
                                }
                            },
                            "dataFile": {
                                "type": "object",
                                "description": "External data file",
                                "required": ["path", "format", "strategy"],
                                "properties": {
                                    "path": {
                                        "type": "string",
                                        "description": "Path to data file"
                                    },
                                    "format": {
                                        "type": "string",
                                        "enum": ["csv", "json"],
                                        "description": "Data file format"
                                    },
                                    "strategy": {
                                        "type": "string",
                                        "enum": ["sequential", "random", "cycle"],
                                        "description": "Data iteration strategy"
                                    }
                                }
                            },
                            "config": {
                                "type": "object",
                                "description": "Scenario-level config overrides",
                                "properties": {
                                    "timeout": {"type": "string"},
                                    "retryCount": {"type": "integer"},
                                    "retryDelay": {"type": "string"}
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    /// Generate Markdown documentation for the configuration schema.
    pub fn generate_markdown_docs(&self) -> String {
        let mut md = String::new();

        md.push_str("# Configuration Schema Reference\n\n");
        md.push_str("Complete reference for rust-loadtest YAML configuration format.\n\n");
        md.push_str("## Table of Contents\n\n");
        md.push_str("- [Version](#version)\n");
        md.push_str("- [Metadata](#metadata)\n");
        md.push_str("- [Config](#config)\n");
        md.push_str("- [Load Models](#load-models)\n");
        md.push_str("- [Scenarios](#scenarios)\n");
        md.push_str("- [Complete Example](#complete-example)\n\n");
        md.push_str("---\n\n");

        // Version
        md.push_str("## Version\n\n");
        md.push_str("**Field**: `version` (required)\n\n");
        md.push_str("**Type**: String\n\n");
        md.push_str("**Description**: Configuration version using semantic versioning.\n\n");
        md.push_str("**Format**: `major.minor`\n\n");
        md.push_str("**Example**:\n```yaml\nversion: \"1.0\"\n```\n\n");
        md.push_str("---\n\n");

        // Metadata
        md.push_str("## Metadata\n\n");
        md.push_str("**Field**: `metadata` (optional)\n\n");
        md.push_str("**Type**: Object\n\n");
        md.push_str("**Description**: Optional metadata about the test configuration.\n\n");
        md.push_str("**Properties**:\n\n");
        md.push_str("| Property | Type | Description |\n");
        md.push_str("|----------|------|-------------|\n");
        md.push_str("| `name` | string | Human-readable test name |\n");
        md.push_str("| `description` | string | Test description |\n");
        md.push_str("| `author` | string | Test author |\n");
        md.push_str("| `tags` | array | Tags for categorization |\n\n");
        md.push_str("**Example**:\n```yaml\nmetadata:\n  name: \"API Load Test\"\n  description: \"Testing API endpoints\"\n  author: \"DevOps Team\"\n  tags: [\"api\", \"production\"]\n```\n\n");
        md.push_str("---\n\n");

        // Config
        md.push_str("## Config\n\n");
        md.push_str("**Field**: `config` (required)\n\n");
        md.push_str("**Type**: Object\n\n");
        md.push_str("**Description**: Global test configuration.\n\n");
        md.push_str("**Properties**:\n\n");
        md.push_str("| Property | Type | Required | Default | Description |\n");
        md.push_str("|----------|------|----------|---------|-------------|\n");
        md.push_str("| `baseUrl` | string | Yes | - | Base URL of the API |\n");
        md.push_str("| `timeout` | string/int | No | `30s` | Request timeout |\n");
        md.push_str("| `workers` | integer | No | `10` | Concurrent workers |\n");
        md.push_str("| `duration` | string/int | Yes | - | Test duration |\n");
        md.push_str("| `skipTlsVerify` | boolean | No | `false` | Skip TLS verification |\n");
        md.push_str("| `customHeaders` | string | No | - | Custom HTTP headers |\n\n");
        md.push_str("**Duration Format**: `<number><unit>` where unit is `s` (seconds), `m` (minutes), or `h` (hours)\n\n");
        md.push_str("**Example**:\n```yaml\nconfig:\n  baseUrl: \"https://api.example.com\"\n  timeout: \"30s\"\n  workers: 50\n  duration: \"10m\"\n  skipTlsVerify: false\n  customHeaders: \"Authorization: Bearer token123\"\n```\n\n");
        md.push_str("---\n\n");

        // Load Models
        md.push_str("## Load Models\n\n");
        md.push_str("**Field**: `load` (required)\n\n");
        md.push_str("**Type**: Object\n\n");
        md.push_str("**Description**: Load generation model.\n\n");
        md.push_str("### Concurrent Model\n\n");
        md.push_str("Fixed number of concurrent workers.\n\n");
        md.push_str("```yaml\nload:\n  model: \"concurrent\"\n```\n\n");
        md.push_str("### RPS Model\n\n");
        md.push_str("Target requests per second.\n\n");
        md.push_str("```yaml\nload:\n  model: \"rps\"\n  target: 100  # 100 requests/second\n```\n\n");
        md.push_str("### Ramp Model\n\n");
        md.push_str("Gradually increase RPS over time.\n\n");
        md.push_str("```yaml\nload:\n  model: \"ramp\"\n  min: 10       # Starting RPS\n  max: 500      # Ending RPS\n  rampDuration: \"5m\"  # Ramp over 5 minutes\n```\n\n");
        md.push_str("---\n\n");

        // Scenarios
        md.push_str("## Scenarios\n\n");
        md.push_str("**Field**: `scenarios` (required)\n\n");
        md.push_str("**Type**: Array\n\n");
        md.push_str("**Description**: Test scenarios with steps.\n\n");
        md.push_str("**Properties**:\n\n");
        md.push_str("| Property | Type | Required | Description |\n");
        md.push_str("|----------|------|----------|-------------|\n");
        md.push_str("| `name` | string | Yes | Scenario name |\n");
        md.push_str("| `weight` | number | No | Traffic distribution weight |\n");
        md.push_str("| `steps` | array | Yes | Scenario steps |\n");
        md.push_str("| `dataFile` | object | No | External data file |\n");
        md.push_str("| `config` | object | No | Scenario-level overrides |\n\n");
        md.push_str("### Step Properties\n\n");
        md.push_str("| Property | Type | Required | Description |\n");
        md.push_str("|----------|------|----------|-------------|\n");
        md.push_str("| `name` | string | No | Step name |\n");
        md.push_str("| `request` | object | Yes | HTTP request |\n");
        md.push_str("| `thinkTime` | string/object | No | Delay after step |\n");
        md.push_str("| `assertions` | array | No | Response assertions |\n");
        md.push_str("| `extract` | array | No | Data extractors |\n\n");
        md.push_str("**Example**:\n```yaml\nscenarios:\n  - name: \"User Login\"\n    weight: 100\n    steps:\n      - name: \"Login Request\"\n        request:\n          method: \"POST\"\n          path: \"/auth/login\"\n          body: '{\"username\": \"user\", \"password\": \"pass\"}'\n        assertions:\n          - statusCode: 200\n        extract:\n          - name: \"token\"\n            jsonPath: \"$.token\"\n        thinkTime: \"2s\"\n```\n\n");
        md.push_str("---\n\n");

        // Complete Example
        md.push_str("## Complete Example\n\n");
        md.push_str("```yaml\nversion: \"1.0\"\n\nmetadata:\n  name: \"API Load Test\"\n  description: \"Testing main API endpoints\"\n  tags: [\"api\", \"production\"]\n\nconfig:\n  baseUrl: \"https://api.example.com\"\n  timeout: \"30s\"\n  workers: 50\n  duration: \"10m\"\n\nload:\n  model: \"rps\"\n  target: 100\n\nscenarios:\n  - name: \"Get Users\"\n    weight: 70\n    steps:\n      - request:\n          method: \"GET\"\n          path: \"/users\"\n        assertions:\n          - statusCode: 200\n\n  - name: \"Create User\"\n    weight: 30\n    steps:\n      - request:\n          method: \"POST\"\n          path: \"/users\"\n          body: '{\"name\": \"Test User\"}'\n        assertions:\n          - statusCode: 201\n```\n\n");

        md
    }

    /// Generate VS Code snippets for configuration.
    pub fn generate_vscode_snippets(&self) -> String {
        let mut snippets = HashMap::new();

        // Basic config snippet
        snippets.insert("loadtest-basic", serde_json::json!({
            "prefix": "loadtest-basic",
            "body": [
                "version: \"1.0\"",
                "",
                "config:",
                "  baseUrl: \"${1:https://api.example.com}\"",
                "  workers: ${2:10}",
                "  duration: \"${3:5m}\"",
                "",
                "load:",
                "  model: \"${4|concurrent,rps,ramp|}\"",
                "  ${5:target: 100}",
                "",
                "scenarios:",
                "  - name: \"${6:My Scenario}\"",
                "    steps:",
                "      - request:",
                "          method: \"${7|GET,POST,PUT,DELETE|}\"",
                "          path: \"${8:/endpoint}\"",
                "        assertions:",
                "          - statusCode: ${9:200}"
            ],
            "description": "Basic load test configuration"
        }));

        // RPS load model snippet
        snippets.insert("loadtest-rps", serde_json::json!({
            "prefix": "loadtest-rps",
            "body": [
                "load:",
                "  model: \"rps\"",
                "  target: ${1:100}"
            ],
            "description": "RPS load model"
        }));

        // Ramp load model snippet
        snippets.insert("loadtest-ramp", serde_json::json!({
            "prefix": "loadtest-ramp",
            "body": [
                "load:",
                "  model: \"ramp\"",
                "  min: ${1:10}",
                "  max: ${2:500}",
                "  rampDuration: \"${3:5m}\""
            ],
            "description": "Ramp load model"
        }));

        // Scenario snippet
        snippets.insert("loadtest-scenario", serde_json::json!({
            "prefix": "loadtest-scenario",
            "body": [
                "- name: \"${1:Scenario Name}\"",
                "  weight: ${2:100}",
                "  steps:",
                "    - name: \"${3:Step Name}\"",
                "      request:",
                "        method: \"${4|GET,POST,PUT,DELETE|}\"",
                "        path: \"${5:/path}\"",
                "      assertions:",
                "        - statusCode: ${6:200}"
            ],
            "description": "Test scenario"
        }));

        // Step snippet
        snippets.insert("loadtest-step", serde_json::json!({
            "prefix": "loadtest-step",
            "body": [
                "- name: \"${1:Step Name}\"",
                "  request:",
                "    method: \"${2|GET,POST,PUT,DELETE|}\"",
                "    path: \"${3:/path}\"",
                "    ${4:body: '${5:{}}'",
                "  ${6:thinkTime: \"${7:2s}\"}",
                "  assertions:",
                "    - statusCode: ${8:200}"
            ],
            "description": "Test step"
        }));

        // Assertion snippets
        snippets.insert("loadtest-assertion-status", serde_json::json!({
            "prefix": "loadtest-assertion-status",
            "body": ["- statusCode: ${1:200}"],
            "description": "Status code assertion"
        }));

        snippets.insert("loadtest-assertion-jsonpath", serde_json::json!({
            "prefix": "loadtest-assertion-jsonpath",
            "body": [
                "- jsonPath:",
                "    path: \"${1:\\$.field}\"",
                "    expected: \"${2:value}\""
            ],
            "description": "JSONPath assertion"
        }));

        // Extractor snippets
        snippets.insert("loadtest-extract-jsonpath", serde_json::json!({
            "prefix": "loadtest-extract-jsonpath",
            "body": [
                "- name: \"${1:varName}\"",
                "  jsonPath: \"${2:\\$.field}\""
            ],
            "description": "JSONPath extractor"
        }));

        // Data file snippet
        snippets.insert("loadtest-datafile", serde_json::json!({
            "prefix": "loadtest-datafile",
            "body": [
                "dataFile:",
                "  path: \"${1:./data.csv}\"",
                "  format: \"${2|csv,json|}\"",
                "  strategy: \"${3|sequential,random,cycle|}\""
            ],
            "description": "External data file"
        }));

        serde_json::to_string_pretty(&snippets).unwrap()
    }
}

impl Default for ConfigDocsGenerator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_json_schema_generation() {
        let generator = ConfigDocsGenerator::new();
        let schema = generator.generate_json_schema();

        assert!(!schema.is_empty());
        assert!(schema.contains("\"$schema\""));
        assert!(schema.contains("\"version\""));
        assert!(schema.contains("\"config\""));
        assert!(schema.contains("\"load\""));
        assert!(schema.contains("\"scenarios\""));

        println!("✅ JSON Schema generation works");
    }

    #[test]
    fn test_json_schema_is_valid_json() {
        let generator = ConfigDocsGenerator::new();
        let schema = generator.generate_json_schema();

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&schema);
        assert!(parsed.is_ok(), "Generated schema should be valid JSON");

        println!("✅ JSON Schema is valid JSON");
    }

    #[test]
    fn test_markdown_docs_generation() {
        let generator = ConfigDocsGenerator::new();
        let markdown = generator.generate_markdown_docs();

        assert!(!markdown.is_empty());
        assert!(markdown.contains("# Configuration Schema Reference"));
        assert!(markdown.contains("## Version"));
        assert!(markdown.contains("## Config"));
        assert!(markdown.contains("## Load Models"));
        assert!(markdown.contains("## Scenarios"));

        println!("✅ Markdown documentation generation works");
    }

    #[test]
    fn test_vscode_snippets_generation() {
        let generator = ConfigDocsGenerator::new();
        let snippets = generator.generate_vscode_snippets();

        assert!(!snippets.is_empty());
        assert!(snippets.contains("loadtest-basic"));
        assert!(snippets.contains("loadtest-rps"));
        assert!(snippets.contains("loadtest-scenario"));

        println!("✅ VS Code snippets generation works");
    }

    #[test]
    fn test_vscode_snippets_is_valid_json() {
        let generator = ConfigDocsGenerator::new();
        let snippets = generator.generate_vscode_snippets();

        let parsed: Result<serde_json::Value, _> = serde_json::from_str(&snippets);
        assert!(parsed.is_ok(), "Generated snippets should be valid JSON");

        println!("✅ VS Code snippets are valid JSON");
    }

    #[test]
    fn test_json_schema_has_required_fields() {
        let generator = ConfigDocsGenerator::new();
        let schema = generator.generate_json_schema();
        let parsed: serde_json::Value = serde_json::from_str(&schema).unwrap();

        // Check required root-level fields
        assert!(parsed["required"].as_array().is_some());
        let required = parsed["required"].as_array().unwrap();
        assert!(required.iter().any(|v| v == "version"));
        assert!(required.iter().any(|v| v == "config"));
        assert!(required.iter().any(|v| v == "load"));
        assert!(required.iter().any(|v| v == "scenarios"));

        println!("✅ JSON Schema has correct required fields");
    }

    #[test]
    fn test_json_schema_has_load_model_types() {
        let generator = ConfigDocsGenerator::new();
        let schema = generator.generate_json_schema();

        // Check that all load models are documented
        assert!(schema.contains("concurrent"));
        assert!(schema.contains("rps"));
        assert!(schema.contains("ramp"));

        println!("✅ JSON Schema includes all load model types");
    }
}
