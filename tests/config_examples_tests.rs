//! Integration tests for config examples and templates (Issue #45).
//!
//! These tests validate:
//! - All example configs parse successfully
//! - All configs pass validation
//! - Templates have correct structure
//! - Example data files are valid

use rust_loadtest::yaml_config::YamlConfig;
use std::fs;
use std::path::Path;

fn load_example_config(filename: &str) -> YamlConfig {
    let path = format!("examples/configs/{}", filename);
    YamlConfig::from_file(&path)
        .unwrap_or_else(|e| panic!("Failed to load {}: {}", filename, e))
}

fn validate_example_config(filename: &str) {
    let config = load_example_config(filename);

    // Basic structure validation
    assert!(!config.version.is_empty(), "{}: version is empty", filename);
    assert!(
        !config.config.base_url.is_empty(),
        "{}: baseUrl is empty",
        filename
    );
    assert!(config.config.workers > 0, "{}: workers must be > 0", filename);
    assert!(
        !config.scenarios.is_empty(),
        "{}: scenarios are empty",
        filename
    );

    println!("✅ {} is valid", filename);
}

#[test]
fn test_basic_api_test_template() {
    let config = load_example_config("basic-api-test.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 10);
    assert_eq!(config.scenarios.len(), 1);
    assert_eq!(config.scenarios[0].name, "API Health Check");
    assert_eq!(config.scenarios[0].weight, 100.0);

    println!("✅ basic-api-test.yaml is valid");
}

#[test]
fn test_ecommerce_scenario_template() {
    let config = load_example_config("ecommerce-scenario.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://shop.example.com");
    assert_eq!(config.config.workers, 50);
    assert_eq!(config.scenarios.len(), 4);

    // Check scenario weights
    assert_eq!(config.scenarios[0].name, "Browse Only");
    assert_eq!(config.scenarios[0].weight, 60.0);
    assert_eq!(config.scenarios[1].name, "Browse and Add to Cart");
    assert_eq!(config.scenarios[1].weight, 25.0);
    assert_eq!(config.scenarios[2].name, "Complete Purchase");
    assert_eq!(config.scenarios[2].weight, 12.0);
    assert_eq!(config.scenarios[3].name, "Quick Browse");
    assert_eq!(config.scenarios[3].weight, 3.0);

    // Total weight should be 100
    let total_weight: f64 = config.scenarios.iter().map(|s| s.weight).sum();
    assert_eq!(total_weight, 100.0);

    println!("✅ ecommerce-scenario.yaml is valid");
}

#[test]
fn test_stress_test_template() {
    let config = load_example_config("stress-test.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 200);
    assert_eq!(config.scenarios.len(), 3);

    // Check scenario distribution
    let total_weight: f64 = config.scenarios.iter().map(|s| s.weight).sum();
    assert_eq!(total_weight, 100.0);

    println!("✅ stress-test.yaml is valid");
}

#[test]
fn test_data_driven_test_template() {
    let config = load_example_config("data-driven-test.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 20);
    assert_eq!(config.scenarios.len(), 2);

    // Check data file configurations
    assert_eq!(config.scenarios[0].name, "User Login with CSV Data");
    assert!(config.scenarios[0].data_file.is_some());
    let csv_data_file = config.scenarios[0].data_file.as_ref().unwrap();
    assert_eq!(csv_data_file.format, "csv");
    assert_eq!(csv_data_file.strategy, "random");

    assert_eq!(config.scenarios[1].name, "Product Search with JSON Data");
    assert!(config.scenarios[1].data_file.is_some());
    let json_data_file = config.scenarios[1].data_file.as_ref().unwrap();
    assert_eq!(json_data_file.format, "json");
    assert_eq!(json_data_file.strategy, "cycle");

    println!("✅ data-driven-test.yaml is valid");
}

#[test]
fn test_authenticated_api_template() {
    let config = load_example_config("authenticated-api.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 25);
    assert_eq!(config.scenarios.len(), 3);

    // Check authentication scenarios
    assert_eq!(config.scenarios[0].name, "JWT Authenticated Requests");
    assert_eq!(config.scenarios[0].weight, 60.0);
    assert_eq!(config.scenarios[1].name, "API Key Authenticated Requests");
    assert_eq!(config.scenarios[1].weight, 30.0);
    assert_eq!(config.scenarios[2].name, "OAuth Token Refresh Flow");
    assert_eq!(config.scenarios[2].weight, 10.0);

    println!("✅ authenticated-api.yaml is valid");
}

#[test]
fn test_microservices_test_template() {
    let config = load_example_config("microservices-test.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://gateway.example.com");
    assert_eq!(config.config.workers, 40);
    assert_eq!(config.scenarios.len(), 4);

    // Check service scenarios
    assert_eq!(config.scenarios[0].name, "User Service Flow");
    assert_eq!(config.scenarios[0].weight, 25.0);
    assert_eq!(config.scenarios[1].name, "Product Service Flow");
    assert_eq!(config.scenarios[1].weight, 30.0);
    assert_eq!(config.scenarios[2].name, "Order Service Flow");
    assert_eq!(config.scenarios[2].weight, 30.0);
    assert_eq!(config.scenarios[3].name, "Inventory Service Flow");
    assert_eq!(config.scenarios[3].weight, 15.0);

    println!("✅ microservices-test.yaml is valid");
}

#[test]
fn test_graphql_api_template() {
    let config = load_example_config("graphql-api.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://graphql.example.com");
    assert_eq!(config.config.workers, 30);
    assert_eq!(config.scenarios.len(), 4);

    // Check GraphQL scenarios
    assert_eq!(config.scenarios[0].name, "Simple GraphQL Queries");
    assert_eq!(config.scenarios[0].weight, 40.0);
    assert_eq!(config.scenarios[1].name, "Complex Nested Queries");
    assert_eq!(config.scenarios[1].weight, 25.0);
    assert_eq!(config.scenarios[2].name, "GraphQL Mutations");
    assert_eq!(config.scenarios[2].weight, 25.0);
    assert_eq!(config.scenarios[3].name, "GraphQL Search and Filter");
    assert_eq!(config.scenarios[3].weight, 10.0);

    println!("✅ graphql-api.yaml is valid");
}

#[test]
fn test_spike_test_template() {
    let config = load_example_config("spike-test.yaml");

    assert_eq!(config.version, "1.0");
    assert_eq!(config.config.base_url, "https://api.example.com");
    assert_eq!(config.config.workers, 150);
    assert_eq!(config.scenarios.len(), 3);

    // Check spike scenarios
    assert_eq!(config.scenarios[0].name, "High-Traffic Endpoint");
    assert_eq!(config.scenarios[0].weight, 80.0);
    assert_eq!(config.scenarios[1].name, "Spike Write Operations");
    assert_eq!(config.scenarios[1].weight, 15.0);
    assert_eq!(config.scenarios[2].name, "System Health Check");
    assert_eq!(config.scenarios[2].weight, 5.0);

    println!("✅ spike-test.yaml is valid");
}

#[test]
fn test_all_templates_parse() {
    let templates = vec![
        "basic-api-test.yaml",
        "ecommerce-scenario.yaml",
        "stress-test.yaml",
        "data-driven-test.yaml",
        "authenticated-api.yaml",
        "microservices-test.yaml",
        "graphql-api.yaml",
        "spike-test.yaml",
    ];

    for template in &templates {
        validate_example_config(template);
    }

    println!("✅ All {} templates are valid", templates.len());
}

#[test]
fn test_all_templates_have_metadata() {
    let templates = vec![
        "basic-api-test.yaml",
        "ecommerce-scenario.yaml",
        "stress-test.yaml",
        "data-driven-test.yaml",
        "authenticated-api.yaml",
        "microservices-test.yaml",
        "graphql-api.yaml",
        "spike-test.yaml",
    ];

    for template in templates {
        let config = load_example_config(template);

        assert!(
            config.metadata.name.is_some(),
            "{}: metadata.name is missing",
            template
        );
        assert!(
            config.metadata.description.is_some(),
            "{}: metadata.description is missing",
            template
        );
        assert!(
            !config.metadata.tags.is_empty(),
            "{}: metadata.tags are empty",
            template
        );
    }

    println!("✅ All templates have complete metadata");
}

#[test]
fn test_all_templates_have_valid_scenarios() {
    let templates = vec![
        "basic-api-test.yaml",
        "ecommerce-scenario.yaml",
        "stress-test.yaml",
        "data-driven-test.yaml",
        "authenticated-api.yaml",
        "microservices-test.yaml",
        "graphql-api.yaml",
        "spike-test.yaml",
    ];

    for template in templates {
        let config = load_example_config(template);

        // All templates should have at least one scenario
        assert!(
            !config.scenarios.is_empty(),
            "{}: no scenarios defined",
            template
        );

        // All scenarios should have valid properties
        for scenario in &config.scenarios {
            assert!(
                !scenario.name.is_empty(),
                "{}: scenario name is empty",
                template
            );
            assert!(
                scenario.weight > 0.0,
                "{}: scenario weight must be > 0",
                template
            );
            assert!(
                !scenario.steps.is_empty(),
                "{}: scenario '{}' has no steps",
                template,
                scenario.name
            );
        }
    }

    println!("✅ All templates have valid scenarios");
}

#[test]
fn test_example_data_files_exist() {
    let data_files = vec![
        "examples/data/users.csv",
        "examples/data/products.json",
    ];

    for file in data_files {
        assert!(
            Path::new(file).exists(),
            "Data file not found: {}",
            file
        );
    }

    println!("✅ All example data files exist");
}

#[test]
fn test_users_csv_format() {
    let csv_content = fs::read_to_string("examples/data/users.csv")
        .expect("Failed to read users.csv");

    // Check header
    assert!(csv_content.contains("username,email,user_id"));

    // Count lines (header + data)
    let line_count = csv_content.lines().count();
    assert!(line_count > 1, "CSV file should have data rows");

    // Check first data row
    assert!(csv_content.contains("john.doe"));

    println!("✅ users.csv has correct format ({} rows)", line_count - 1);
}

#[test]
fn test_products_json_format() {
    let json_content = fs::read_to_string("examples/data/products.json")
        .expect("Failed to read products.json");

    // Parse JSON
    let products: serde_json::Value = serde_json::from_str(&json_content)
        .expect("Failed to parse products.json");

    // Should be an array
    assert!(products.is_array(), "products.json should be an array");

    let products_array = products.as_array().unwrap();
    assert!(!products_array.is_empty(), "products.json should not be empty");

    // Check first product has required fields
    let first_product = &products_array[0];
    assert!(first_product.get("product_name").is_some());
    assert!(first_product.get("category").is_some());
    assert!(first_product.get("sku").is_some());
    assert!(first_product.get("price").is_some());

    println!("✅ products.json has correct format ({} products)", products_array.len());
}

#[test]
fn test_readme_exists() {
    assert!(
        Path::new("examples/configs/README.md").exists(),
        "README.md not found in examples/configs/"
    );

    let readme = fs::read_to_string("examples/configs/README.md")
        .expect("Failed to read README.md");

    // Check that README documents all templates
    assert!(readme.contains("basic-api-test.yaml"));
    assert!(readme.contains("ecommerce-scenario.yaml"));
    assert!(readme.contains("stress-test.yaml"));
    assert!(readme.contains("data-driven-test.yaml"));
    assert!(readme.contains("authenticated-api.yaml"));
    assert!(readme.contains("microservices-test.yaml"));
    assert!(readme.contains("graphql-api.yaml"));
    assert!(readme.contains("spike-test.yaml"));

    println!("✅ README.md exists and documents all templates");
}

#[test]
fn test_template_weights_sum_correctly() {
    let templates_with_weights = vec![
        "ecommerce-scenario.yaml",
        "stress-test.yaml",
        "authenticated-api.yaml",
        "microservices-test.yaml",
        "graphql-api.yaml",
        "spike-test.yaml",
    ];

    for template in templates_with_weights {
        let config = load_example_config(template);
        let total_weight: f64 = config.scenarios.iter().map(|s| s.weight).sum();

        assert!(
            (total_weight - 100.0).abs() < 0.001,
            "{}: weights sum to {}, expected 100",
            template,
            total_weight
        );
    }

    println!("✅ All multi-scenario templates have weights summing to 100");
}

#[test]
fn test_templates_have_reasonable_settings() {
    let templates = vec![
        "basic-api-test.yaml",
        "ecommerce-scenario.yaml",
        "stress-test.yaml",
        "data-driven-test.yaml",
        "authenticated-api.yaml",
        "microservices-test.yaml",
        "graphql-api.yaml",
        "spike-test.yaml",
    ];

    for template in templates {
        let config = load_example_config(template);

        // Workers should be reasonable (1-500)
        assert!(
            config.config.workers >= 1 && config.config.workers <= 500,
            "{}: workers {} out of reasonable range (1-500)",
            template,
            config.config.workers
        );

        // Should have example.com URLs (not real production URLs)
        assert!(
            config.config.base_url.contains("example.com"),
            "{}: should use example.com URLs",
            template
        );
    }

    println!("✅ All templates have reasonable settings");
}
