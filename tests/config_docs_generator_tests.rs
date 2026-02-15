//! Integration tests for config documentation generator (Issue #46).
//!
//! These tests validate:
//! - JSON Schema generation
//! - Markdown documentation generation
//! - VS Code snippets generation
//! - Output file generation

use rust_loadtest::config_docs_generator::ConfigDocsGenerator;
use std::fs;
use tempfile::TempDir;

#[test]
fn test_generate_json_schema() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();

    assert!(!schema.is_empty());
    assert!(schema.contains("\"$schema\""));
    assert!(schema.contains("\"title\": \"Rust LoadTest Configuration\""));

    println!("✅ JSON Schema generation works");
}

#[test]
fn test_json_schema_contains_all_sections() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();

    // Check all major sections are present
    assert!(schema.contains("\"version\""));
    assert!(schema.contains("\"metadata\""));
    assert!(schema.contains("\"config\""));
    assert!(schema.contains("\"load\""));
    assert!(schema.contains("\"scenarios\""));

    println!("✅ JSON Schema contains all required sections");
}

#[test]
fn test_json_schema_has_load_models() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();

    // Check all load models are documented
    assert!(schema.contains("concurrent"));
    assert!(schema.contains("\"rps\""));
    assert!(schema.contains("\"ramp\""));

    println!("✅ JSON Schema documents all load models");
}

#[test]
fn test_json_schema_is_valid_json() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();

    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&schema);
    assert!(parsed.is_ok(), "JSON Schema should be valid JSON");

    let json = parsed.unwrap();
    assert_eq!(json["$schema"], "http://json-schema.org/draft-07/schema#");

    println!("✅ JSON Schema is valid JSON");
}

#[test]
fn test_json_schema_required_fields() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();
    let json: serde_json::Value = serde_json::from_str(&schema).unwrap();

    // Check required fields at root level
    let required = json["required"].as_array().unwrap();
    assert!(required.contains(&serde_json::json!("version")));
    assert!(required.contains(&serde_json::json!("config")));
    assert!(required.contains(&serde_json::json!("load")));
    assert!(required.contains(&serde_json::json!("scenarios")));

    println!("✅ JSON Schema has correct required fields");
}

#[test]
fn test_json_schema_config_properties() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();
    let json: serde_json::Value = serde_json::from_str(&schema).unwrap();

    // Check config section properties
    let config_props = &json["properties"]["config"]["properties"];
    assert!(config_props["baseUrl"].is_object());
    assert!(config_props["timeout"].is_object());
    assert!(config_props["workers"].is_object());
    assert!(config_props["duration"].is_object());

    println!("✅ JSON Schema config section is correct");
}

#[test]
fn test_generate_markdown_docs() {
    let generator = ConfigDocsGenerator::new();
    let markdown = generator.generate_markdown_docs();

    assert!(!markdown.is_empty());
    assert!(markdown.contains("# Configuration Schema Reference"));

    println!("✅ Markdown documentation generation works");
}

#[test]
fn test_markdown_docs_has_all_sections() {
    let generator = ConfigDocsGenerator::new();
    let markdown = generator.generate_markdown_docs();

    // Check all major sections
    assert!(markdown.contains("## Version"));
    assert!(markdown.contains("## Metadata"));
    assert!(markdown.contains("## Config"));
    assert!(markdown.contains("## Load Models"));
    assert!(markdown.contains("## Scenarios"));
    assert!(markdown.contains("## Complete Example"));

    println!("✅ Markdown docs contain all sections");
}

#[test]
fn test_markdown_docs_has_examples() {
    let generator = ConfigDocsGenerator::new();
    let markdown = generator.generate_markdown_docs();

    // Check that code examples are present
    assert!(markdown.contains("```yaml"));
    assert!(markdown.contains("version: \"1.0\""));
    assert!(markdown.contains("baseUrl:"));
    assert!(markdown.contains("scenarios:"));

    println!("✅ Markdown docs include YAML examples");
}

#[test]
fn test_markdown_docs_has_tables() {
    let generator = ConfigDocsGenerator::new();
    let markdown = generator.generate_markdown_docs();

    // Check that tables are present
    assert!(markdown.contains("| Property"));
    assert!(markdown.contains("|-------"));

    println!("✅ Markdown docs include property tables");
}

#[test]
fn test_generate_vscode_snippets() {
    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();

    assert!(!snippets.is_empty());
    assert!(snippets.contains("\"loadtest-basic\""));

    println!("✅ VS Code snippets generation works");
}

#[test]
fn test_vscode_snippets_is_valid_json() {
    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();

    let parsed: Result<serde_json::Value, _> = serde_json::from_str(&snippets);
    assert!(parsed.is_ok(), "Snippets should be valid JSON");

    println!("✅ VS Code snippets are valid JSON");
}

#[test]
fn test_vscode_snippets_has_all_snippets() {
    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();

    // Check all major snippets are present
    assert!(snippets.contains("\"loadtest-basic\""));
    assert!(snippets.contains("\"loadtest-rps\""));
    assert!(snippets.contains("\"loadtest-ramp\""));
    assert!(snippets.contains("\"loadtest-scenario\""));
    assert!(snippets.contains("\"loadtest-step\""));

    println!("✅ VS Code snippets include all snippet types");
}

#[test]
fn test_vscode_snippets_structure() {
    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();
    let json: serde_json::Value = serde_json::from_str(&snippets).unwrap();

    // Check snippet structure
    let basic = &json["loadtest-basic"];
    assert!(basic["prefix"].is_string());
    assert!(basic["body"].is_array());
    assert!(basic["description"].is_string());

    println!("✅ VS Code snippets have correct structure");
}

#[test]
fn test_vscode_snippet_basic_config() {
    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();
    let json: serde_json::Value = serde_json::from_str(&snippets).unwrap();

    let basic = &json["loadtest-basic"];
    let body = basic["body"].as_array().unwrap();

    // Check that basic config includes all essential parts
    let body_str = body.iter()
        .map(|v| v.as_str().unwrap())
        .collect::<Vec<_>>()
        .join("\n");

    assert!(body_str.contains("version:"));
    assert!(body_str.contains("config:"));
    assert!(body_str.contains("load:"));
    assert!(body_str.contains("scenarios:"));

    println!("✅ Basic snippet includes all essential sections");
}

#[test]
fn test_write_json_schema_to_file() {
    let temp_dir = TempDir::new().unwrap();
    let schema_path = temp_dir.path().join("schema.json");

    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();

    fs::write(&schema_path, schema).unwrap();

    assert!(schema_path.exists());

    let content = fs::read_to_string(&schema_path).unwrap();
    assert!(!content.is_empty());

    println!("✅ Can write JSON Schema to file");
}

#[test]
fn test_write_markdown_docs_to_file() {
    let temp_dir = TempDir::new().unwrap();
    let docs_path = temp_dir.path().join("schema.md");

    let generator = ConfigDocsGenerator::new();
    let markdown = generator.generate_markdown_docs();

    fs::write(&docs_path, markdown).unwrap();

    assert!(docs_path.exists());

    let content = fs::read_to_string(&docs_path).unwrap();
    assert!(!content.is_empty());

    println!("✅ Can write Markdown docs to file");
}

#[test]
fn test_write_vscode_snippets_to_file() {
    let temp_dir = TempDir::new().unwrap();
    let snippets_path = temp_dir.path().join("snippets.json");

    let generator = ConfigDocsGenerator::new();
    let snippets = generator.generate_vscode_snippets();

    fs::write(&snippets_path, snippets).unwrap();

    assert!(snippets_path.exists());

    let content = fs::read_to_string(&snippets_path).unwrap();
    assert!(!content.is_empty());

    println!("✅ Can write VS Code snippets to file");
}

#[test]
fn test_generator_default() {
    let generator = ConfigDocsGenerator::default();
    let schema = generator.generate_json_schema();

    assert!(!schema.is_empty());

    println!("✅ Default constructor works");
}

#[test]
fn test_json_schema_examples() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();
    let json: serde_json::Value = serde_json::from_str(&schema).unwrap();

    // Check that examples are provided
    let version_examples = &json["properties"]["version"]["examples"];
    assert!(version_examples.is_array());
    assert_eq!(version_examples[0], "1.0");

    println!("✅ JSON Schema includes examples");
}

#[test]
fn test_json_schema_patterns() {
    let generator = ConfigDocsGenerator::new();
    let schema = generator.generate_json_schema();
    let json: serde_json::Value = serde_json::from_str(&schema).unwrap();

    // Check that version has a pattern
    let version_pattern = &json["properties"]["version"]["pattern"];
    assert!(version_pattern.is_string());

    println!("✅ JSON Schema includes validation patterns");
}
