//! Integration tests for CSV data-driven testing (Issue #31).
//!
//! These tests validate that CSV data can be loaded, distributed across
//! virtual users, and used for variable substitution in scenarios.

use rust_loadtest::data_source::CsvDataSource;
use rust_loadtest::executor::{ScenarioExecutor, SessionStore};
use rust_loadtest::scenario::{Assertion, RequestConfig, Scenario, ScenarioContext, Step};
use std::collections::HashMap;
use std::time::Duration;
use tempfile::NamedTempFile;

const BASE_URL: &str = "https://httpbin.org";

fn create_test_client() -> reqwest::Client {
    reqwest::Client::builder()
        .cookie_store(true)
        .timeout(Duration::from_secs(30))
        .build()
        .expect("Failed to create HTTP client")
}

#[test]
fn test_csv_load_from_string() {
    let csv = "username,password,email\nuser1,pass1,user1@test.com\nuser2,pass2,user2@test.com";
    let ds = CsvDataSource::from_string(csv).unwrap();

    assert_eq!(ds.row_count(), 2);
    assert_eq!(ds.headers(), &["username", "password", "email"]);

    let row1 = ds.next_row().unwrap();
    assert_eq!(row1.get("username").unwrap(), "user1");
    assert_eq!(row1.get("password").unwrap(), "pass1");

    println!("✅ CSV loading from string works");
}

#[test]
fn test_csv_load_from_file() {
    // Create temporary CSV file
    let csv_content =
        "product_id,name,price\n101,Widget,19.99\n102,Gadget,29.99\n103,Doohickey,39.99";

    let mut temp_file = NamedTempFile::new().unwrap();
    use std::io::Write;
    temp_file.write_all(csv_content.as_bytes()).unwrap();
    temp_file.flush().unwrap();

    let ds = CsvDataSource::from_file(temp_file.path()).unwrap();

    assert_eq!(ds.row_count(), 3);
    assert_eq!(ds.headers(), &["product_id", "name", "price"]);

    let row = ds.next_row().unwrap();
    assert_eq!(row.get("product_id").unwrap(), "101");
    assert_eq!(row.get("name").unwrap(), "Widget");
    assert_eq!(row.get("price").unwrap(), "19.99");

    println!("✅ CSV loading from file works");
}

#[test]
fn test_csv_round_robin_distribution() {
    let csv = "user_id,role\n1,admin\n2,user\n3,guest";
    let ds = CsvDataSource::from_string(csv).unwrap();

    // Get 6 rows (2 full cycles through 3 users)
    let ids: Vec<String> = (0..6)
        .map(|_| ds.next_row().unwrap().get("user_id").unwrap().clone())
        .collect();

    assert_eq!(ids, vec!["1", "2", "3", "1", "2", "3"]);

    println!("✅ Round-robin distribution works");
}

#[test]
fn test_csv_reset() {
    let csv = "id,value\n1,a\n2,b\n3,c";
    let ds = CsvDataSource::from_string(csv).unwrap();

    ds.next_row().unwrap();
    ds.next_row().unwrap();

    ds.reset();

    let row = ds.next_row().unwrap();
    assert_eq!(row.get("id").unwrap(), "1");

    println!("✅ CSV reset works");
}

#[test]
fn test_context_load_data_row() {
    let csv = "username,api_key,region\ntestuser,abc123,us-west";
    let ds = CsvDataSource::from_string(csv).unwrap();
    let row = ds.next_row().unwrap();

    let mut context = ScenarioContext::new();
    context.load_data_row(&row);

    assert_eq!(
        context.get_variable("username"),
        Some(&"testuser".to_string())
    );
    assert_eq!(context.get_variable("api_key"), Some(&"abc123".to_string()));
    assert_eq!(context.get_variable("region"), Some(&"us-west".to_string()));

    println!("✅ Context loads data row correctly");
}

#[test]
fn test_variable_substitution_from_csv() {
    let csv = "user_id,product_id,quantity\n42,SKU-999,5";
    let ds = CsvDataSource::from_string(csv).unwrap();
    let row = ds.next_row().unwrap();

    let mut context = ScenarioContext::new();
    context.load_data_row(&row);

    let path = context
        .substitute_variables("/users/${user_id}/cart?product=${product_id}&qty=${quantity}");
    assert_eq!(path, "/users/42/cart?product=SKU-999&qty=5");

    println!("✅ Variable substitution from CSV works");
}

#[tokio::test]
async fn test_scenario_with_csv_data() {
    let csv = "username,email\ntestuser1,test1@example.com\ntestuser2,test2@example.com";
    let ds = CsvDataSource::from_string(csv).unwrap();

    let scenario = Scenario {
        name: "CSV Data Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Request with CSV data".to_string(),
            request: RequestConfig {
                method: "POST".to_string(),
                path: "/post".to_string(),
                body: Some(r#"{"username": "${username}", "email": "${email}"}"#.to_string()),
                headers: {
                    let mut h = HashMap::new();
                    h.insert("Content-Type".to_string(), "application/json".to_string());
                    h
                },
            },
            extractions: vec![],
            assertions: vec![],
            cache: None,
            think_time: None,
        }],
    };

    // Execute scenario twice with different data rows
    for i in 0..2 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

        let mut context = ScenarioContext::new();
        let row = ds.next_row().unwrap();
        context.load_data_row(&row);

        let result = executor.execute(&scenario, &mut context, &mut SessionStore::new()).await;

        assert!(result.steps[0].status_code.is_some());
        println!(
            "  Execution {} completed with status {:?}",
            i + 1,
            result.steps[0].status_code
        );
    }

    println!("✅ Scenario with CSV data works");
}

#[tokio::test]
async fn test_multiple_users_different_data() {
    let csv = "username,password\nuser1,pass1\nuser2,pass2\nuser3,pass3";
    let ds = CsvDataSource::from_string(csv).unwrap();

    let scenario = Scenario {
        name: "Multi-User Test".to_string(),
        weight: 1.0,
        steps: vec![Step {
            name: "Login with user data".to_string(),
            request: RequestConfig {
                method: "GET".to_string(),
                path: "/get".to_string(), // Simple GET endpoint
                body: None,
                headers: HashMap::new(),
            },
            extractions: vec![],
            assertions: vec![Assertion::StatusCode(200)],
            cache: None,
            think_time: None,
        }],
    };

    // Simulate 3 virtual users, each getting different data
    let mut users_data = Vec::new();

    for i in 0..3 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

        let mut context = ScenarioContext::new();
        let row = ds.next_row().unwrap();
        let username = row.get("username").unwrap().clone();
        users_data.push(username.clone());

        context.load_data_row(&row);

        let result = executor.execute(&scenario, &mut context, &mut SessionStore::new()).await;

        assert!(result.success, "Virtual user {} should succeed", i + 1);
        println!("  Virtual user {} used data: {}", i + 1, username);
    }

    // Verify each user got different data
    assert_eq!(users_data, vec!["user1", "user2", "user3"]);

    println!("✅ Multiple users with different data works");
}

#[tokio::test]
async fn test_realistic_user_pool() {
    // Simulate a realistic user pool with credentials
    let user_csv = r#"username,password,email,role
alice,alice123,alice@company.com,admin
bob,bob456,bob@company.com,user
carol,carol789,carol@company.com,user
dave,dave012,dave@company.com,manager"#;

    let ds = CsvDataSource::from_string(user_csv).unwrap();

    let scenario = Scenario {
        name: "User Pool Test".to_string(),
        weight: 1.0,
        steps: vec![
            Step {
                name: "Health Check".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/get".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![Assertion::StatusCode(200)],
                cache: None,
                think_time: None,
            },
            Step {
                name: "Check Status".to_string(),
                request: RequestConfig {
                    method: "GET".to_string(),
                    path: "/json".to_string(),
                    body: None,
                    headers: HashMap::new(),
                },
                extractions: vec![],
                assertions: vec![],
                cache: None,
                think_time: None,
            },
        ],
    };

    // Simulate 8 virtual users (2 full cycles through 4 users)
    for i in 0..8 {
        let client = create_test_client();
        let executor = ScenarioExecutor::new(BASE_URL.to_string(), client);

        let mut context = ScenarioContext::new();
        let row = ds.next_row().unwrap();
        let username = row.get("username").unwrap();
        let role = row.get("role").unwrap();

        context.load_data_row(&row);

        let result = executor.execute(&scenario, &mut context, &mut SessionStore::new()).await;

        assert!(result.success, "User {} should succeed", username);
        println!("  VU {} as {} (role: {})", i + 1, username, role);
    }

    println!("✅ Realistic user pool test works");
}

#[test]
fn test_csv_with_special_characters() {
    let csv = r#"username,password,notes
user1,p@ss!123,"Has special chars"
user2,"pass,with,comma","Multi, line, value"
user3,simple,Normal"#;

    let ds = CsvDataSource::from_string(csv).unwrap();

    let row1 = ds.next_row().unwrap();
    assert_eq!(row1.get("password").unwrap(), "p@ss!123");

    let row2 = ds.next_row().unwrap();
    assert_eq!(row2.get("password").unwrap(), "pass,with,comma");

    println!("✅ CSV with special characters works");
}

#[test]
fn test_empty_csv_error() {
    let empty_csv = "username,password\n";
    let result = CsvDataSource::from_string(empty_csv);

    assert!(result.is_err());
    println!("✅ Empty CSV properly returns error");
}

#[test]
fn test_csv_concurrent_access() {
    use std::sync::Arc;
    use std::thread;

    let csv = "id,value\n1,a\n2,b\n3,c\n4,d\n5,e";
    let ds = Arc::new(CsvDataSource::from_string(csv).unwrap());

    let mut handles = vec![];

    // Spawn 10 threads, each getting 3 rows
    for thread_id in 0..10 {
        let ds_clone = Arc::clone(&ds);
        let handle = thread::spawn(move || {
            let mut ids = Vec::new();
            for _ in 0..3 {
                let row = ds_clone.next_row().unwrap();
                ids.push(row.get("id").unwrap().clone());
            }
            (thread_id, ids)
        });
        handles.push(handle);
    }

    let mut all_ids = Vec::new();
    for handle in handles {
        let (thread_id, ids) = handle.join().unwrap();
        println!("  Thread {} got IDs: {:?}", thread_id, ids);
        all_ids.extend(ids);
    }

    // Should have distributed 30 rows total (10 threads * 3 rows each)
    assert_eq!(all_ids.len(), 30);

    println!("✅ Concurrent CSV access works correctly");
}

#[test]
fn test_csv_builder() {
    let csv = "a,b,c\n1,2,3";

    let ds = rust_loadtest::data_source::CsvDataSourceBuilder::new()
        .content(csv)
        .build()
        .unwrap();

    assert_eq!(ds.row_count(), 1);
    println!("✅ CSV builder works");
}
