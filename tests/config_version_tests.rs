//! Integration tests for config versioning (Issue #41).
//!
//! These tests validate version parsing, validation, compatibility checking,
//! and the migration framework.

use rust_loadtest::config_version::{
    Migration, MigrationRegistry, Version, VersionChecker, VersionError, VersionInfo,
};
use std::str::FromStr;

#[test]
fn test_version_parsing_valid() {
    let version = Version::from_str("1.0").unwrap();
    assert_eq!(version.major, 1);
    assert_eq!(version.minor, 0);

    let version = Version::from_str("2.5").unwrap();
    assert_eq!(version.major, 2);
    assert_eq!(version.minor, 5);

    let version = Version::from_str("10.99").unwrap();
    assert_eq!(version.major, 10);
    assert_eq!(version.minor, 99);

    println!("✅ Valid version parsing works");
}

#[test]
fn test_version_parsing_invalid() {
    assert!(Version::from_str("1").is_err());
    assert!(Version::from_str("1.0.0").is_err());
    assert!(Version::from_str("invalid").is_err());
    assert!(Version::from_str("1.x").is_err());
    assert!(Version::from_str("x.0").is_err());
    assert!(Version::from_str("").is_err());
    assert!(Version::from_str("1.").is_err());
    assert!(Version::from_str(".0").is_err());

    println!("✅ Invalid version parsing is rejected");
}

#[test]
fn test_version_display() {
    assert_eq!(Version::new(1, 0).to_string(), "1.0");
    assert_eq!(Version::new(2, 5).to_string(), "2.5");
    assert_eq!(Version::new(10, 99).to_string(), "10.99");

    println!("✅ Version display formatting works");
}

#[test]
fn test_version_equality() {
    assert_eq!(Version::new(1, 0), Version::new(1, 0));
    assert_eq!(Version::new(2, 5), Version::new(2, 5));
    assert_ne!(Version::new(1, 0), Version::new(1, 1));
    assert_ne!(Version::new(1, 0), Version::new(2, 0));

    println!("✅ Version equality comparison works");
}

#[test]
fn test_version_ordering() {
    // Minor version comparison
    assert!(Version::new(1, 0) < Version::new(1, 1));
    assert!(Version::new(1, 5) < Version::new(1, 6));
    assert!(Version::new(1, 9) < Version::new(1, 10));

    // Major version comparison
    assert!(Version::new(1, 0) < Version::new(2, 0));
    assert!(Version::new(1, 9) < Version::new(2, 0));
    assert!(Version::new(2, 5) < Version::new(3, 0));

    // Greater than
    assert!(Version::new(1, 1) > Version::new(1, 0));
    assert!(Version::new(2, 0) > Version::new(1, 9));
    assert!(Version::new(3, 0) > Version::new(2, 99));

    println!("✅ Version ordering comparison works");
}

#[test]
fn test_version_constants() {
    assert_eq!(Version::CURRENT, Version::new(1, 0));
    assert_eq!(Version::MINIMUM_SUPPORTED, Version::new(1, 0));
    assert_eq!(Version::MAXIMUM_SUPPORTED, Version::new(1, 0));

    println!("✅ Version constants are correct");
}

#[test]
fn test_version_is_supported() {
    // Current version should be supported
    assert!(Version::CURRENT.is_supported());
    assert!(Version::new(1, 0).is_supported());

    // Future versions not yet supported
    assert!(!Version::new(2, 0).is_supported());
    assert!(!Version::new(1, 1).is_supported());

    // Old versions not supported
    assert!(!Version::new(0, 9).is_supported());
    assert!(!Version::new(0, 1).is_supported());

    println!("✅ Version support detection works");
}

#[test]
fn test_version_needs_migration() {
    // Current version doesn't need migration
    assert!(!Version::CURRENT.needs_migration());
    assert!(!Version::new(1, 0).needs_migration());

    // Future versions would need migration (once we have multiple versions)
    // For now, only 1.0 exists, so no migrations needed yet

    println!("✅ Version migration detection works");
}

#[test]
fn test_version_supported_list() {
    let versions = Version::supported_versions();
    assert!(!versions.is_empty());
    assert!(versions.contains(&Version::new(1, 0)));

    let version_string = Version::supported_versions_string();
    assert!(version_string.contains("1.0"));

    println!("✅ Supported versions list is correct");
}

#[test]
fn test_version_checker_validate_supported() {
    let result = VersionChecker::validate(&Version::new(1, 0));
    assert!(result.is_ok());

    println!("✅ Supported version passes validation");
}

#[test]
fn test_version_checker_validate_too_old() {
    let result = VersionChecker::validate(&Version::new(0, 5));
    assert!(result.is_err());

    match result.unwrap_err() {
        VersionError::VersionTooOld { current, minimum } => {
            assert_eq!(current, "0.5");
            assert_eq!(minimum, "1.0");
        }
        _ => panic!("Expected VersionTooOld error"),
    }

    println!("✅ Too old version is rejected with correct error");
}

#[test]
fn test_version_checker_validate_too_new() {
    let result = VersionChecker::validate(&Version::new(99, 0));
    assert!(result.is_err());

    match result.unwrap_err() {
        VersionError::VersionTooNew { current, maximum } => {
            assert_eq!(current, "99.0");
            assert_eq!(maximum, "1.0");
        }
        _ => panic!("Expected VersionTooNew error"),
    }

    println!("✅ Too new version is rejected with correct error");
}

#[test]
fn test_version_checker_parse_and_validate_valid() {
    let version = VersionChecker::parse_and_validate("1.0").unwrap();
    assert_eq!(version, Version::new(1, 0));

    println!("✅ Parse and validate works for valid version");
}

#[test]
fn test_version_checker_parse_and_validate_invalid_format() {
    let result = VersionChecker::parse_and_validate("invalid");
    assert!(result.is_err());

    match result.unwrap_err() {
        VersionError::InvalidFormat(msg) => {
            assert_eq!(msg, "invalid");
        }
        _ => panic!("Expected InvalidFormat error"),
    }

    println!("✅ Parse and validate rejects invalid format");
}

#[test]
fn test_version_checker_parse_and_validate_unsupported() {
    let result = VersionChecker::parse_and_validate("2.0");
    assert!(result.is_err());

    match result.unwrap_err() {
        VersionError::VersionTooNew { .. } => {
            // Expected
        }
        _ => panic!("Expected VersionTooNew error"),
    }

    println!("✅ Parse and validate rejects unsupported version");
}

#[test]
fn test_version_checker_check_compatibility_current() {
    let result = VersionChecker::check_compatibility(&Version::CURRENT);
    assert!(result.is_ok());
    assert!(result.unwrap().is_none()); // No migration needed for current version

    println!("✅ Compatibility check for current version succeeds");
}

#[test]
fn test_version_checker_check_compatibility_unsupported() {
    let result = VersionChecker::check_compatibility(&Version::new(99, 0));
    assert!(result.is_err());

    println!("✅ Compatibility check for unsupported version fails");
}

#[test]
fn test_migration_registry_empty() {
    let registry = MigrationRegistry::new();

    let migration = registry.find_migration(&Version::new(1, 0), &Version::new(2, 0));
    assert!(migration.is_none());

    println!("✅ Empty migration registry has no migrations");
}

#[test]
fn test_migration_registry_default() {
    let registry = MigrationRegistry::default_migrations();

    // Currently no migrations exist, but registry should be valid
    let migration = registry.find_migration(&Version::new(1, 0), &Version::new(2, 0));
    assert!(migration.is_none());

    println!("✅ Default migration registry is valid");
}

#[test]
fn test_migration_registry_migrate_same_version() {
    let registry = MigrationRegistry::default_migrations();
    let yaml = r#"
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
"#;

    let result = registry
        .migrate(yaml, &Version::new(1, 0), &Version::new(1, 0))
        .unwrap();

    assert_eq!(result, yaml);

    println!("✅ Migrating same version returns unchanged YAML");
}

#[test]
fn test_migration_registry_register() {
    struct DummyMigration;

    impl Migration for DummyMigration {
        fn from_version(&self) -> Version {
            Version::new(1, 0)
        }

        fn to_version(&self) -> Version {
            Version::new(2, 0)
        }

        fn description(&self) -> &str {
            "Test migration"
        }

        fn migrate(&self, yaml: &str) -> Result<String, VersionError> {
            Ok(yaml.replace("1.0", "2.0"))
        }
    }

    let mut registry = MigrationRegistry::new();
    registry.register(Box::new(DummyMigration));

    let migration = registry.find_migration(&Version::new(1, 0), &Version::new(2, 0));
    assert!(migration.is_some());
    assert_eq!(migration.unwrap().description(), "Test migration");

    println!("✅ Migration registration works");
}

#[test]
fn test_migration_registry_apply_migration() {
    struct TestMigration;

    impl Migration for TestMigration {
        fn from_version(&self) -> Version {
            Version::new(1, 0)
        }

        fn to_version(&self) -> Version {
            Version::new(1, 1)
        }

        fn description(&self) -> &str {
            "Add new field"
        }

        fn migrate(&self, yaml: &str) -> Result<String, VersionError> {
            // Simple test: replace version string
            Ok(yaml.replace("version: \"1.0\"", "version: \"1.1\""))
        }
    }

    let mut registry = MigrationRegistry::new();
    registry.register(Box::new(TestMigration));

    let yaml = "version: \"1.0\"";
    let result = registry
        .migrate(yaml, &Version::new(1, 0), &Version::new(1, 1))
        .unwrap();

    assert!(result.contains("version: \"1.1\""));

    println!("✅ Migration application works");
}

#[test]
fn test_version_error_display() {
    let err = VersionError::InvalidFormat("1.0.0".to_string());
    assert!(err.to_string().contains("Invalid version format"));
    assert!(err.to_string().contains("1.0.0"));

    let err = VersionError::VersionTooOld {
        current: "0.5".to_string(),
        minimum: "1.0".to_string(),
    };
    assert!(err.to_string().contains("too old"));
    assert!(err.to_string().contains("0.5"));
    assert!(err.to_string().contains("1.0"));

    let err = VersionError::VersionTooNew {
        current: "99.0".to_string(),
        maximum: "1.0".to_string(),
    };
    assert!(err.to_string().contains("too new"));
    assert!(err.to_string().contains("99.0"));

    println!("✅ Version error display messages are helpful");
}

#[test]
fn test_version_info_current() {
    let version = VersionInfo::current();
    assert_eq!(version, Version::new(1, 0));

    println!("✅ VersionInfo returns current version");
}

#[test]
fn test_version_info_supported_range() {
    let min = VersionInfo::minimum_supported();
    let max = VersionInfo::maximum_supported();

    assert_eq!(min, Version::new(1, 0));
    assert_eq!(max, Version::new(1, 0));
    assert!(min <= max);

    println!("✅ VersionInfo returns supported range");
}

#[test]
fn test_version_info_string() {
    let info = VersionInfo::info_string();

    assert!(info.contains("Current"));
    assert!(info.contains("1.0"));
    assert!(info.contains("Minimum Supported"));
    assert!(info.contains("Maximum Supported"));
    assert!(info.contains("Supported Versions"));

    println!("✅ VersionInfo string contains all information");
}

#[test]
fn test_version_error_equality() {
    let err1 = VersionError::InvalidFormat("test".to_string());
    let err2 = VersionError::InvalidFormat("test".to_string());
    let err3 = VersionError::InvalidFormat("other".to_string());

    assert_eq!(err1, err2);
    assert_ne!(err1, err3);

    println!("✅ VersionError equality comparison works");
}

#[test]
fn test_version_roundtrip() {
    let version = Version::new(2, 5);
    let version_str = version.to_string();
    let parsed = Version::from_str(&version_str).unwrap();
    assert_eq!(version, parsed);

    println!("✅ Version roundtrip (to_string -> from_str) works");
}

#[test]
fn test_version_with_yaml_config() {
    use rust_loadtest::yaml_config::YamlConfig;

    // Valid version should work
    let yaml = r#"
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
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_ok());

    println!("✅ Version 1.0 works with YamlConfig");
}

#[test]
fn test_unsupported_version_with_yaml_config() {
    use rust_loadtest::yaml_config::YamlConfig;

    // Unsupported version should fail
    let yaml = r#"
version: "2.0"
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
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("2.0"));
    assert!(err.to_string().contains("too new"));

    println!("✅ Unsupported version 2.0 is rejected by YamlConfig");
}

#[test]
fn test_invalid_version_format_with_yaml_config() {
    use rust_loadtest::yaml_config::YamlConfig;

    // Invalid version format should fail
    let yaml = r#"
version: "1.0.0"
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
"#;

    let result = YamlConfig::from_str(yaml);
    assert!(result.is_err());

    let err = result.unwrap_err();
    assert!(err.to_string().contains("Invalid version format"));

    println!("✅ Invalid version format is rejected by YamlConfig");
}

#[test]
fn test_future_version_scenario() {
    // Scenario: When we release version 2.0 in the future
    // Version 2.0 config should not be loadable with current code

    let version_2_0 = Version::new(2, 0);
    assert!(!version_2_0.is_supported());
    assert!(VersionChecker::validate(&version_2_0).is_err());

    println!("✅ Future version 2.0 is correctly rejected");
}

#[test]
fn test_version_comparison_comprehensive() {
    let versions = [Version::new(0, 9),
        Version::new(1, 0),
        Version::new(1, 1),
        Version::new(1, 9),
        Version::new(2, 0),
        Version::new(2, 1),
        Version::new(10, 0)];

    for i in 0..versions.len() {
        for j in i + 1..versions.len() {
            assert!(
                versions[i] < versions[j],
                "{} should be less than {}",
                versions[i],
                versions[j]
            );
        }
    }

    println!("✅ Comprehensive version comparison works");
}
