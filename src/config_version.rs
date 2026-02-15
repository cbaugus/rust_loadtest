//! Configuration versioning and migration framework (Issue #41).
//!
//! This module provides version management for YAML configuration files,
//! including version validation, compatibility checking, and migration
//! framework for evolving config schemas over time.

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::fmt;
use std::str::FromStr;
use thiserror::Error;

/// Version parsing and validation errors.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum VersionError {
    #[error("Invalid version format: {0}. Expected format: X.Y (e.g., 1.0, 2.1)")]
    InvalidFormat(String),

    #[error("Unsupported version: {version}. Supported versions: {supported}")]
    UnsupportedVersion { version: String, supported: String },

    #[error("Version {current} is too old. Minimum supported version: {minimum}")]
    VersionTooOld { current: String, minimum: String },

    #[error("Version {current} is too new. Maximum supported version: {maximum}")]
    VersionTooNew { current: String, maximum: String },

    #[error("Migration failed from {from} to {to}: {reason}")]
    MigrationFailed {
        from: String,
        to: String,
        reason: String,
    },
}

/// Semantic version for config files.
///
/// Supports major.minor versioning (e.g., 1.0, 2.1).
/// Patch versions are not used as config changes typically warrant
/// at least a minor version bump.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
}

impl Version {
    /// Create a new version.
    pub fn new(major: u32, minor: u32) -> Self {
        Self { major, minor }
    }

    /// Current supported version.
    pub const CURRENT: Version = Version { major: 1, minor: 0 };

    /// Minimum supported version (oldest version that can be loaded).
    pub const MINIMUM_SUPPORTED: Version = Version { major: 1, minor: 0 };

    /// Maximum supported version (newest version that can be loaded).
    pub const MAXIMUM_SUPPORTED: Version = Version { major: 1, minor: 0 };

    /// Check if this version is supported.
    pub fn is_supported(&self) -> bool {
        *self >= Self::MINIMUM_SUPPORTED && *self <= Self::MAXIMUM_SUPPORTED
    }

    /// Check if this version requires migration to current.
    pub fn needs_migration(&self) -> bool {
        *self < Self::CURRENT
    }

    /// Get list of all supported versions.
    pub fn supported_versions() -> Vec<Version> {
        vec![Version::new(1, 0)]
    }

    /// Get supported versions as a formatted string.
    pub fn supported_versions_string() -> String {
        Self::supported_versions()
            .iter()
            .map(|v| v.to_string())
            .collect::<Vec<_>>()
            .join(", ")
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}", self.major, self.minor)
    }
}

impl FromStr for Version {
    type Err = VersionError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = s.split('.').collect();
        if parts.len() != 2 {
            return Err(VersionError::InvalidFormat(s.to_string()));
        }

        let major = parts[0]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?;
        let minor = parts[1]
            .parse::<u32>()
            .map_err(|_| VersionError::InvalidFormat(s.to_string()))?;

        Ok(Version::new(major, minor))
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.major.cmp(&other.major) {
            Ordering::Equal => self.minor.cmp(&other.minor),
            other => other,
        }
    }
}

/// Version compatibility checker.
pub struct VersionChecker;

impl VersionChecker {
    /// Validate that a version is supported.
    pub fn validate(version: &Version) -> Result<(), VersionError> {
        if !version.is_supported() {
            if *version < Version::MINIMUM_SUPPORTED {
                return Err(VersionError::VersionTooOld {
                    current: version.to_string(),
                    minimum: Version::MINIMUM_SUPPORTED.to_string(),
                });
            } else if *version > Version::MAXIMUM_SUPPORTED {
                return Err(VersionError::VersionTooNew {
                    current: version.to_string(),
                    maximum: Version::MAXIMUM_SUPPORTED.to_string(),
                });
            } else {
                return Err(VersionError::UnsupportedVersion {
                    version: version.to_string(),
                    supported: Version::supported_versions_string(),
                });
            }
        }
        Ok(())
    }

    /// Parse and validate a version string.
    pub fn parse_and_validate(version_str: &str) -> Result<Version, VersionError> {
        let version = Version::from_str(version_str)?;
        Self::validate(&version)?;
        Ok(version)
    }

    /// Check version compatibility and return migration path if needed.
    pub fn check_compatibility(version: &Version) -> Result<Option<Vec<Version>>, VersionError> {
        Self::validate(version)?;

        if version.needs_migration() {
            Ok(Some(Self::get_migration_path(version)))
        } else {
            Ok(None)
        }
    }

    /// Get the migration path from one version to another.
    fn get_migration_path(from: &Version) -> Vec<Version> {
        let mut path = Vec::new();
        let mut current = *from;

        // For now, since we only have 1.0, no migration path exists yet
        // When we add 2.0, this would return [1.0, 2.0]
        while current < Version::CURRENT {
            // Increment to next minor version
            current.minor += 1;
            if current.minor >= 10 {
                current.major += 1;
                current.minor = 0;
            }
            path.push(current);
        }

        path
    }
}

/// Migration trait for config version migrations.
pub trait Migration {
    /// Source version this migration applies from.
    #[allow(clippy::wrong_self_convention)]
    fn from_version(&self) -> Version;

    /// Target version this migration applies to.
    #[allow(clippy::wrong_self_convention)]
    fn to_version(&self) -> Version;

    /// Description of what this migration does.
    fn description(&self) -> &str;

    /// Apply the migration to a YAML string.
    ///
    /// This takes the raw YAML as a string and returns the migrated YAML.
    /// Migrations can modify the structure, add/remove fields, or transform values.
    fn migrate(&self, yaml: &str) -> Result<String, VersionError>;
}

/// Registry of all available migrations.
pub struct MigrationRegistry {
    migrations: Vec<Box<dyn Migration>>,
}

impl MigrationRegistry {
    /// Create a new empty migration registry.
    pub fn new() -> Self {
        Self {
            migrations: Vec::new(),
        }
    }

    /// Create the default migration registry with all migrations.
    pub fn default_migrations() -> Self {
        // Future migrations will be registered here
        // Example: registry.register(Box::new(MigrationV1ToV2));
        Self::new()
    }

    /// Register a migration.
    pub fn register(&mut self, migration: Box<dyn Migration>) {
        self.migrations.push(migration);
    }

    /// Find a migration from one version to another.
    pub fn find_migration(&self, from: &Version, to: &Version) -> Option<&dyn Migration> {
        self.migrations
            .iter()
            .find(|m| m.from_version() == *from && m.to_version() == *to)
            .map(|m| m.as_ref())
    }

    /// Apply migrations to upgrade YAML from one version to another.
    pub fn migrate(
        &self,
        yaml: &str,
        from: &Version,
        to: &Version,
    ) -> Result<String, VersionError> {
        if from == to {
            return Ok(yaml.to_string());
        }

        let mut current_yaml = yaml.to_string();
        let mut current_version = *from;

        while current_version < *to {
            // Find next migration step
            let next_version = Version::new(
                if current_version.minor < 9 {
                    current_version.major
                } else {
                    current_version.major + 1
                },
                if current_version.minor < 9 {
                    current_version.minor + 1
                } else {
                    0
                },
            );

            if let Some(migration) = self.find_migration(&current_version, &next_version) {
                current_yaml = migration.migrate(&current_yaml)?;
                current_version = next_version;
            } else {
                return Err(VersionError::MigrationFailed {
                    from: current_version.to_string(),
                    to: next_version.to_string(),
                    reason: "No migration found".to_string(),
                });
            }

            // Safety check: don't loop forever
            if current_version > *to {
                break;
            }
        }

        Ok(current_yaml)
    }
}

impl Default for MigrationRegistry {
    fn default() -> Self {
        Self::default_migrations()
    }
}

/// Version information and utilities.
pub struct VersionInfo;

impl VersionInfo {
    /// Get the current config version.
    pub fn current() -> Version {
        Version::CURRENT
    }

    /// Get the minimum supported version.
    pub fn minimum_supported() -> Version {
        Version::MINIMUM_SUPPORTED
    }

    /// Get the maximum supported version.
    pub fn maximum_supported() -> Version {
        Version::MAXIMUM_SUPPORTED
    }

    /// Get version information as a formatted string.
    pub fn info_string() -> String {
        format!(
            "Config Version Info:\n\
             - Current: {}\n\
             - Minimum Supported: {}\n\
             - Maximum Supported: {}\n\
             - Supported Versions: {}",
            Version::CURRENT,
            Version::MINIMUM_SUPPORTED,
            Version::MAXIMUM_SUPPORTED,
            Version::supported_versions_string()
        )
    }

    /// Print version information to stdout.
    pub fn print_info() {
        println!("{}", Self::info_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_parsing() {
        assert_eq!(Version::from_str("1.0").unwrap(), Version::new(1, 0));
        assert_eq!(Version::from_str("2.5").unwrap(), Version::new(2, 5));
        assert_eq!(Version::from_str("10.99").unwrap(), Version::new(10, 99));

        println!("✅ Version parsing works");
    }

    #[test]
    fn test_version_parsing_errors() {
        assert!(Version::from_str("1").is_err());
        assert!(Version::from_str("1.0.0").is_err());
        assert!(Version::from_str("invalid").is_err());
        assert!(Version::from_str("1.x").is_err());

        println!("✅ Version parsing errors work");
    }

    #[test]
    fn test_version_display() {
        let version = Version::new(1, 0);
        assert_eq!(version.to_string(), "1.0");

        let version = Version::new(2, 5);
        assert_eq!(version.to_string(), "2.5");

        println!("✅ Version display works");
    }

    #[test]
    fn test_version_comparison() {
        assert!(Version::new(1, 0) < Version::new(1, 1));
        assert!(Version::new(1, 0) < Version::new(2, 0));
        assert!(Version::new(1, 5) < Version::new(2, 0));
        assert!(Version::new(2, 0) > Version::new(1, 9));
        assert_eq!(Version::new(1, 0), Version::new(1, 0));

        println!("✅ Version comparison works");
    }

    #[test]
    fn test_version_is_supported() {
        assert!(Version::new(1, 0).is_supported());
        // Future versions not yet supported
        assert!(!Version::new(2, 0).is_supported());
        assert!(!Version::new(0, 9).is_supported());

        println!("✅ Version support checking works");
    }

    #[test]
    fn test_version_needs_migration() {
        assert!(!Version::new(1, 0).needs_migration()); // Current version
                                                        // Future: when we have 2.0, version 1.0 will need migration
                                                        // assert!(Version::new(1, 0).needs_migration());

        println!("✅ Version migration checking works");
    }

    #[test]
    fn test_version_checker_validate() {
        assert!(VersionChecker::validate(&Version::new(1, 0)).is_ok());
        assert!(VersionChecker::validate(&Version::new(2, 0)).is_err());
        assert!(VersionChecker::validate(&Version::new(0, 9)).is_err());

        println!("✅ Version validation works");
    }

    #[test]
    fn test_version_checker_parse_and_validate() {
        assert!(VersionChecker::parse_and_validate("1.0").is_ok());
        assert!(VersionChecker::parse_and_validate("2.0").is_err());
        assert!(VersionChecker::parse_and_validate("invalid").is_err());

        println!("✅ Version parse and validate works");
    }

    #[test]
    fn test_version_too_old_error() {
        let result = VersionChecker::validate(&Version::new(0, 5));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("too old"));
        assert!(err.to_string().contains("0.5"));
        assert!(err.to_string().contains("1.0"));

        println!("✅ Version too old error message works");
    }

    #[test]
    fn test_version_too_new_error() {
        let result = VersionChecker::validate(&Version::new(99, 0));
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(err.to_string().contains("too new"));
        assert!(err.to_string().contains("99.0"));

        println!("✅ Version too new error message works");
    }

    #[test]
    fn test_version_supported_list() {
        let versions = Version::supported_versions();
        assert!(!versions.is_empty());
        assert!(versions.contains(&Version::new(1, 0)));

        let version_string = Version::supported_versions_string();
        assert!(version_string.contains("1.0"));

        println!("✅ Supported versions list works");
    }

    #[test]
    fn test_migration_registry_empty() {
        let registry = MigrationRegistry::new();
        assert!(registry
            .find_migration(&Version::new(1, 0), &Version::new(2, 0))
            .is_none());

        println!("✅ Empty migration registry works");
    }

    #[test]
    fn test_migration_registry_migrate_same_version() {
        let registry = MigrationRegistry::default_migrations();
        let yaml = "version: '1.0'";
        let result = registry
            .migrate(yaml, &Version::new(1, 0), &Version::new(1, 0))
            .unwrap();
        assert_eq!(result, yaml);

        println!("✅ Migrate same version returns unchanged YAML");
    }

    #[test]
    fn test_version_info_string() {
        let info = VersionInfo::info_string();
        assert!(info.contains("Current"));
        assert!(info.contains("1.0"));
        assert!(info.contains("Minimum Supported"));
        assert!(info.contains("Maximum Supported"));

        println!("✅ Version info string works");
    }

    #[test]
    fn test_version_constants() {
        assert_eq!(Version::CURRENT, Version::new(1, 0));
        assert_eq!(Version::MINIMUM_SUPPORTED, Version::new(1, 0));
        assert_eq!(Version::MAXIMUM_SUPPORTED, Version::new(1, 0));

        println!("✅ Version constants are correct");
    }

    #[test]
    fn test_check_compatibility() {
        let result = VersionChecker::check_compatibility(&Version::new(1, 0));
        assert!(result.is_ok());
        assert!(result.unwrap().is_none()); // No migration needed

        println!("✅ Compatibility checking works");
    }
}
