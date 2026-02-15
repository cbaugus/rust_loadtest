//! Configuration schema validation (Issue #38).
//!
//! This module provides comprehensive validation for YAML configuration files
//! with detailed error messages and field-level validation rules.

use std::collections::HashMap;
use thiserror::Error;

/// Validation error with context about which field failed.
#[derive(Error, Debug, Clone)]
pub enum ValidationError {
    #[error("Field '{field}': {message}")]
    FieldError { field: String, message: String },

    #[error("Field '{field}' is required but not provided")]
    RequiredField { field: String },

    #[error("Field '{field}': value {value} is out of range ({min} to {max})")]
    OutOfRange {
        field: String,
        value: String,
        min: String,
        max: String,
    },

    #[error("Field '{field}': invalid format - {message}")]
    InvalidFormat { field: String, message: String },

    #[error("Field '{field}': invalid enum value '{value}'. Expected one of: {expected}")]
    InvalidEnum {
        field: String,
        value: String,
        expected: String,
    },

    #[error("Multiple validation errors: {0}")]
    Multiple(String),
}

/// Result type for validation operations.
pub type ValidationResult<T> = Result<T, ValidationError>;

/// Validation context for building error messages.
pub struct ValidationContext {
    field_path: Vec<String>,
    errors: Vec<ValidationError>,
}

impl ValidationContext {
    pub fn new() -> Self {
        Self {
            field_path: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Enter a nested field context.
    pub fn enter(&mut self, field: &str) {
        self.field_path.push(field.to_string());
    }

    /// Exit the current field context.
    pub fn exit(&mut self) {
        self.field_path.pop();
    }

    /// Get the current field path as a string.
    pub fn current_path(&self) -> String {
        self.field_path.join(".")
    }

    /// Add a validation error.
    pub fn add_error(&mut self, error: ValidationError) {
        self.errors.push(error);
    }

    /// Add a field error with automatic path.
    pub fn field_error(&mut self, message: String) {
        self.add_error(ValidationError::FieldError {
            field: self.current_path(),
            message,
        });
    }

    /// Check if any errors were collected.
    pub fn has_errors(&self) -> bool {
        !self.errors.is_empty()
    }

    /// Get all collected errors.
    pub fn errors(&self) -> &[ValidationError] {
        &self.errors
    }

    /// Consume the context and return a result.
    pub fn into_result(self) -> Result<(), ValidationError> {
        if self.errors.is_empty() {
            Ok(())
        } else {
            let messages: Vec<String> = self.errors.iter().map(|e| e.to_string()).collect();
            Err(ValidationError::Multiple(messages.join("; ")))
        }
    }
}

impl Default for ValidationContext {
    fn default() -> Self {
        Self::new()
    }
}

/// Validator for URLs.
pub struct UrlValidator;

impl UrlValidator {
    pub fn validate(url: &str) -> ValidationResult<()> {
        if url.is_empty() {
            return Err(ValidationError::InvalidFormat {
                field: "url".to_string(),
                message: "URL cannot be empty".to_string(),
            });
        }

        if !url.starts_with("http://") && !url.starts_with("https://") {
            return Err(ValidationError::InvalidFormat {
                field: "url".to_string(),
                message: format!(
                    "URL must start with http:// or https://, got: {}",
                    url
                ),
            });
        }

        // Basic validation - check for obvious issues
        if url.contains(' ') {
            return Err(ValidationError::InvalidFormat {
                field: "url".to_string(),
                message: "URL cannot contain spaces".to_string(),
            });
        }

        Ok(())
    }
}

/// Validator for durations.
pub struct DurationValidator;

impl DurationValidator {
    pub fn validate(duration_str: &str) -> ValidationResult<()> {
        // Try to parse using the utility function
        crate::utils::parse_duration_string(duration_str).map_err(|e| {
            ValidationError::InvalidFormat {
                field: "duration".to_string(),
                message: format!("Invalid duration format '{}': {}", duration_str, e),
            }
        })?;
        Ok(())
    }

    pub fn validate_positive(duration_str: &str) -> ValidationResult<()> {
        Self::validate(duration_str)?;

        let duration = crate::utils::parse_duration_string(duration_str).unwrap();
        if duration.as_secs() == 0 {
            return Err(ValidationError::OutOfRange {
                field: "duration".to_string(),
                value: "0s".to_string(),
                min: "1s".to_string(),
                max: "unlimited".to_string(),
            });
        }

        Ok(())
    }
}

/// Validator for numeric ranges.
pub struct RangeValidator;

impl RangeValidator {
    pub fn validate_u64(value: u64, min: u64, max: u64, field: &str) -> ValidationResult<()> {
        if value < min || value > max {
            return Err(ValidationError::OutOfRange {
                field: field.to_string(),
                value: value.to_string(),
                min: min.to_string(),
                max: max.to_string(),
            });
        }
        Ok(())
    }

    pub fn validate_f64(value: f64, min: f64, max: f64, field: &str) -> ValidationResult<()> {
        if value < min || value > max {
            return Err(ValidationError::OutOfRange {
                field: field.to_string(),
                value: value.to_string(),
                min: min.to_string(),
                max: max.to_string(),
            });
        }
        Ok(())
    }

    pub fn validate_positive_u64(value: u64, field: &str) -> ValidationResult<()> {
        if value == 0 {
            return Err(ValidationError::OutOfRange {
                field: field.to_string(),
                value: "0".to_string(),
                min: "1".to_string(),
                max: "unlimited".to_string(),
            });
        }
        Ok(())
    }

    pub fn validate_positive_f64(value: f64, field: &str) -> ValidationResult<()> {
        if value <= 0.0 {
            return Err(ValidationError::OutOfRange {
                field: field.to_string(),
                value: value.to_string(),
                min: "0.0 (exclusive)".to_string(),
                max: "unlimited".to_string(),
            });
        }
        Ok(())
    }
}

/// Validator for HTTP methods.
pub struct HttpMethodValidator;

impl HttpMethodValidator {
    const VALID_METHODS: &'static [&'static str] =
        &["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];

    pub fn validate(method: &str) -> ValidationResult<()> {
        let method_upper = method.to_uppercase();
        if !Self::VALID_METHODS.contains(&method_upper.as_str()) {
            return Err(ValidationError::InvalidEnum {
                field: "method".to_string(),
                value: method.to_string(),
                expected: Self::VALID_METHODS.join(", "),
            });
        }
        Ok(())
    }
}

/// Validator for load model types.
pub struct LoadModelValidator;

impl LoadModelValidator {
    pub fn validate_rps(target_rps: f64) -> ValidationResult<()> {
        RangeValidator::validate_positive_f64(target_rps, "load.target")
    }

    pub fn validate_ramp(min_rps: f64, max_rps: f64) -> ValidationResult<()> {
        RangeValidator::validate_positive_f64(min_rps, "load.min")?;
        RangeValidator::validate_positive_f64(max_rps, "load.max")?;

        if min_rps >= max_rps {
            return Err(ValidationError::FieldError {
                field: "load".to_string(),
                message: format!(
                    "min_rps ({}) must be less than max_rps ({})",
                    min_rps, max_rps
                ),
            });
        }

        Ok(())
    }

    pub fn validate_daily_traffic(min_rps: f64, mid_rps: f64, max_rps: f64) -> ValidationResult<()> {
        RangeValidator::validate_positive_f64(min_rps, "load.min")?;
        RangeValidator::validate_positive_f64(mid_rps, "load.mid")?;
        RangeValidator::validate_positive_f64(max_rps, "load.max")?;

        if !(min_rps < mid_rps && mid_rps < max_rps) {
            return Err(ValidationError::FieldError {
                field: "load".to_string(),
                message: format!(
                    "Daily traffic must satisfy: min ({}) < mid ({}) < max ({})",
                    min_rps, mid_rps, max_rps
                ),
            });
        }

        Ok(())
    }
}

/// Configuration schema definition and JSON Schema export.
pub struct ConfigSchema;

impl ConfigSchema {
    /// Generate JSON Schema for the YAML configuration.
    pub fn to_json_schema() -> serde_json::Value {
        serde_json::json!({
            "$schema": "http://json-schema.org/draft-07/schema#",
            "title": "Rust LoadTest Configuration",
            "type": "object",
            "required": ["version", "config", "load", "scenarios"],
            "properties": {
                "version": {
                    "type": "string",
                    "const": "1.0",
                    "description": "Configuration format version"
                },
                "metadata": {
                    "type": "object",
                    "properties": {
                        "name": { "type": "string" },
                        "description": { "type": "string" },
                        "author": { "type": "string" },
                        "tags": {
                            "type": "array",
                            "items": { "type": "string" }
                        }
                    }
                },
                "config": {
                    "type": "object",
                    "required": ["baseUrl", "duration"],
                    "properties": {
                        "baseUrl": {
                            "type": "string",
                            "format": "uri",
                            "pattern": "^https?://",
                            "description": "Base URL for all requests"
                        },
                        "workers": {
                            "type": "integer",
                            "minimum": 1,
                            "maximum": 10000,
                            "default": 10,
                            "description": "Number of concurrent workers"
                        },
                        "duration": {
                            "oneOf": [
                                { "type": "integer", "minimum": 1 },
                                { "type": "string", "pattern": "^\\d+[smhd]$" }
                            ],
                            "description": "Test duration (e.g., '5m', '2h', 300)"
                        },
                        "timeout": {
                            "oneOf": [
                                { "type": "integer", "minimum": 1 },
                                { "type": "string", "pattern": "^\\d+[smhd]$" }
                            ],
                            "default": 30,
                            "description": "Request timeout"
                        },
                        "skipTlsVerify": {
                            "type": "boolean",
                            "default": false,
                            "description": "Skip TLS certificate verification"
                        }
                    }
                },
                "load": {
                    "oneOf": [
                        {
                            "type": "object",
                            "required": ["model"],
                            "properties": {
                                "model": { "const": "concurrent" }
                            }
                        },
                        {
                            "type": "object",
                            "required": ["model", "target"],
                            "properties": {
                                "model": { "const": "rps" },
                                "target": { "type": "number", "minimum": 0.1 }
                            }
                        },
                        {
                            "type": "object",
                            "required": ["model", "min", "max", "rampDuration"],
                            "properties": {
                                "model": { "const": "ramp" },
                                "min": { "type": "number", "minimum": 0.1 },
                                "max": { "type": "number", "minimum": 0.1 },
                                "rampDuration": { "oneOf": [
                                    { "type": "integer" },
                                    { "type": "string" }
                                ]}
                            }
                        }
                    ]
                },
                "scenarios": {
                    "type": "array",
                    "minItems": 1,
                    "items": {
                        "type": "object",
                        "required": ["name", "steps"],
                        "properties": {
                            "name": { "type": "string" },
                            "weight": { "type": "number", "minimum": 0.1, "default": 1.0 },
                            "steps": {
                                "type": "array",
                                "minItems": 1,
                                "items": {
                                    "type": "object",
                                    "required": ["request"],
                                    "properties": {
                                        "name": { "type": "string" },
                                        "request": {
                                            "type": "object",
                                            "required": ["method", "path"],
                                            "properties": {
                                                "method": {
                                                    "type": "string",
                                                    "enum": ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"]
                                                },
                                                "path": { "type": "string" }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        })
    }

    /// Export JSON Schema to a file.
    pub fn export_json_schema() -> String {
        serde_json::to_string_pretty(&Self::to_json_schema()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_url_validator_valid() {
        assert!(UrlValidator::validate("https://example.com").is_ok());
        assert!(UrlValidator::validate("http://localhost:8080").is_ok());
        assert!(UrlValidator::validate("https://api.example.com/v1").is_ok());
    }

    #[test]
    fn test_url_validator_invalid() {
        assert!(UrlValidator::validate("").is_err());
        assert!(UrlValidator::validate("example.com").is_err());
        assert!(UrlValidator::validate("ftp://example.com").is_err());
        assert!(UrlValidator::validate("https://example .com").is_err());
    }

    #[test]
    fn test_duration_validator() {
        assert!(DurationValidator::validate("30s").is_ok());
        assert!(DurationValidator::validate("5m").is_ok());
        assert!(DurationValidator::validate("2h").is_ok());
        assert!(DurationValidator::validate("invalid").is_err());
    }

    #[test]
    fn test_duration_validator_positive() {
        assert!(DurationValidator::validate_positive("1s").is_ok());
        assert!(DurationValidator::validate_positive("0s").is_err());
    }

    #[test]
    fn test_range_validator_u64() {
        assert!(RangeValidator::validate_u64(50, 1, 100, "test").is_ok());
        assert!(RangeValidator::validate_u64(0, 1, 100, "test").is_err());
        assert!(RangeValidator::validate_u64(101, 1, 100, "test").is_err());
    }

    #[test]
    fn test_range_validator_positive() {
        assert!(RangeValidator::validate_positive_u64(1, "test").is_ok());
        assert!(RangeValidator::validate_positive_u64(0, "test").is_err());
    }

    #[test]
    fn test_http_method_validator() {
        assert!(HttpMethodValidator::validate("GET").is_ok());
        assert!(HttpMethodValidator::validate("POST").is_ok());
        assert!(HttpMethodValidator::validate("get").is_ok()); // case insensitive
        assert!(HttpMethodValidator::validate("INVALID").is_err());
    }

    #[test]
    fn test_load_model_validator_rps() {
        assert!(LoadModelValidator::validate_rps(100.0).is_ok());
        assert!(LoadModelValidator::validate_rps(0.0).is_err());
        assert!(LoadModelValidator::validate_rps(-10.0).is_err());
    }

    #[test]
    fn test_load_model_validator_ramp() {
        assert!(LoadModelValidator::validate_ramp(10.0, 100.0).is_ok());
        assert!(LoadModelValidator::validate_ramp(100.0, 10.0).is_err());
        assert!(LoadModelValidator::validate_ramp(50.0, 50.0).is_err());
    }

    #[test]
    fn test_load_model_validator_daily_traffic() {
        assert!(LoadModelValidator::validate_daily_traffic(10.0, 50.0, 100.0).is_ok());
        assert!(LoadModelValidator::validate_daily_traffic(100.0, 50.0, 10.0).is_err());
        assert!(LoadModelValidator::validate_daily_traffic(10.0, 10.0, 100.0).is_err());
    }

    #[test]
    fn test_validation_context() {
        let mut ctx = ValidationContext::new();

        ctx.enter("config");
        ctx.enter("baseUrl");
        assert_eq!(ctx.current_path(), "config.baseUrl");

        ctx.field_error("Invalid URL".to_string());
        assert!(ctx.has_errors());

        ctx.exit();
        ctx.exit();
        assert_eq!(ctx.current_path(), "");
    }

    #[test]
    fn test_json_schema_export() {
        let schema = ConfigSchema::to_json_schema();
        assert!(schema.is_object());

        let schema_str = ConfigSchema::export_json_schema();
        assert!(schema_str.contains("\"$schema\""));
        assert!(schema_str.contains("version"));
        assert!(schema_str.contains("config"));
    }
}
