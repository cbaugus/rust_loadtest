//! Error categorization for better diagnostics and reporting.
//!
//! This module provides classification of HTTP errors into meaningful categories
//! for better analysis of load test failures. Errors are categorized by type
//! (client errors, server errors, network issues, timeouts) for detailed reporting.

use std::fmt;

/// Categories of errors that can occur during load testing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ErrorCategory {
    /// HTTP 4xx errors (client errors)
    ClientError,

    /// HTTP 5xx errors (server errors)
    ServerError,

    /// Network connectivity errors (DNS, connection refused, etc.)
    NetworkError,

    /// Request timeout errors
    TimeoutError,

    /// TLS/SSL certificate errors
    TlsError,

    /// Other/unknown errors
    OtherError,
}

impl ErrorCategory {
    /// Categorize an HTTP status code.
    ///
    /// # Arguments
    /// * `status_code` - HTTP status code (200, 404, 500, etc.)
    ///
    /// # Returns
    /// The appropriate error category, or None if status is success (2xx/3xx)
    pub fn from_status_code(status_code: u16) -> Option<Self> {
        match status_code {
            200..=399 => None, // Success responses
            400..=499 => Some(ErrorCategory::ClientError),
            500..=599 => Some(ErrorCategory::ServerError),
            _ => Some(ErrorCategory::OtherError),
        }
    }

    /// Categorize a reqwest error.
    ///
    /// # Arguments
    /// * `error` - The reqwest error to categorize
    ///
    /// # Returns
    /// The appropriate error category
    pub fn from_reqwest_error(error: &reqwest::Error) -> Self {
        if error.is_timeout() {
            ErrorCategory::TimeoutError
        } else if error.is_connect() {
            ErrorCategory::NetworkError
        } else if error.is_request() {
            // Request building/sending errors
            ErrorCategory::NetworkError
        } else if error.is_body() || error.is_decode() {
            // Response body errors - usually network or server issues
            ErrorCategory::NetworkError
        } else if error.is_redirect() {
            // Redirect errors
            ErrorCategory::ClientError
        } else {
            // Check error message for common patterns
            let error_msg = error.to_string().to_lowercase();

            if error_msg.contains("certificate")
                || error_msg.contains("tls")
                || error_msg.contains("ssl")
            {
                ErrorCategory::TlsError
            } else if error_msg.contains("timeout") {
                ErrorCategory::TimeoutError
            } else if error_msg.contains("dns") || error_msg.contains("resolve") || error_msg.contains("connect") || error_msg.contains("connection") {
                ErrorCategory::NetworkError
            } else {
                ErrorCategory::OtherError
            }
        }
    }

    /// Get the Prometheus label for this error category.
    pub fn label(&self) -> &'static str {
        match self {
            ErrorCategory::ClientError => "client_error",
            ErrorCategory::ServerError => "server_error",
            ErrorCategory::NetworkError => "network_error",
            ErrorCategory::TimeoutError => "timeout_error",
            ErrorCategory::TlsError => "tls_error",
            ErrorCategory::OtherError => "other_error",
        }
    }

    /// Get a human-readable description of this error category.
    pub fn description(&self) -> &'static str {
        match self {
            ErrorCategory::ClientError => "HTTP 4xx Client Errors",
            ErrorCategory::ServerError => "HTTP 5xx Server Errors",
            ErrorCategory::NetworkError => "Network/Connection Errors",
            ErrorCategory::TimeoutError => "Request Timeout Errors",
            ErrorCategory::TlsError => "TLS/SSL Certificate Errors",
            ErrorCategory::OtherError => "Other/Unknown Errors",
        }
    }

    /// Get all error categories in a consistent order.
    pub fn all() -> Vec<ErrorCategory> {
        vec![
            ErrorCategory::ClientError,
            ErrorCategory::ServerError,
            ErrorCategory::NetworkError,
            ErrorCategory::TimeoutError,
            ErrorCategory::TlsError,
            ErrorCategory::OtherError,
        ]
    }
}

impl fmt::Display for ErrorCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.description())
    }
}

/// Detailed error information with categorization.
#[derive(Debug, Clone)]
pub struct CategorizedError {
    /// The error category
    pub category: ErrorCategory,

    /// HTTP status code if available
    pub status_code: Option<u16>,

    /// Error message
    pub message: String,

    /// Endpoint that failed
    pub endpoint: Option<String>,
}

impl CategorizedError {
    /// Create a new categorized error from an HTTP status code.
    pub fn from_status(
        status_code: u16,
        message: String,
        endpoint: Option<String>,
    ) -> Option<Self> {
        ErrorCategory::from_status_code(status_code).map(|category| Self {
            category,
            status_code: Some(status_code),
            message,
            endpoint,
        })
    }

    /// Create a new categorized error from a reqwest error.
    pub fn from_reqwest(error: &reqwest::Error, endpoint: Option<String>) -> Self {
        let category = ErrorCategory::from_reqwest_error(error);
        let status_code = error.status().map(|s| s.as_u16());
        let message = error.to_string();

        Self {
            category,
            status_code,
            message,
            endpoint,
        }
    }

    /// Create a custom categorized error.
    pub fn new(category: ErrorCategory, message: String) -> Self {
        Self {
            category,
            status_code: None,
            message,
            endpoint: None,
        }
    }
}

impl fmt::Display for CategorizedError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(status) = self.status_code {
            write!(
                f,
                "[{}] HTTP {}: {}",
                self.category.label(),
                status,
                self.message
            )
        } else {
            write!(f, "[{}] {}", self.category.label(), self.message)
        }
    }
}

/// Helper to categorize common HTTP status codes for display.
pub fn categorize_status_code(status_code: u16) -> &'static str {
    match status_code {
        // 2xx Success
        200 => "OK",
        201 => "Created",
        202 => "Accepted",
        204 => "No Content",

        // 3xx Redirection
        301 => "Moved Permanently",
        302 => "Found",
        304 => "Not Modified",

        // 4xx Client Errors
        400 => "Bad Request",
        401 => "Unauthorized",
        403 => "Forbidden",
        404 => "Not Found",
        405 => "Method Not Allowed",
        408 => "Request Timeout",
        409 => "Conflict",
        429 => "Too Many Requests",

        // 5xx Server Errors
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        504 => "Gateway Timeout",

        _ => "Unknown Status",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_success_codes() {
        assert_eq!(ErrorCategory::from_status_code(200), None);
        assert_eq!(ErrorCategory::from_status_code(201), None);
        assert_eq!(ErrorCategory::from_status_code(204), None);
        assert_eq!(ErrorCategory::from_status_code(301), None);
        assert_eq!(ErrorCategory::from_status_code(302), None);
    }

    #[test]
    fn test_categorize_4xx_errors() {
        assert_eq!(
            ErrorCategory::from_status_code(400),
            Some(ErrorCategory::ClientError)
        );
        assert_eq!(
            ErrorCategory::from_status_code(404),
            Some(ErrorCategory::ClientError)
        );
        assert_eq!(
            ErrorCategory::from_status_code(429),
            Some(ErrorCategory::ClientError)
        );
    }

    #[test]
    fn test_categorize_5xx_errors() {
        assert_eq!(
            ErrorCategory::from_status_code(500),
            Some(ErrorCategory::ServerError)
        );
        assert_eq!(
            ErrorCategory::from_status_code(502),
            Some(ErrorCategory::ServerError)
        );
        assert_eq!(
            ErrorCategory::from_status_code(503),
            Some(ErrorCategory::ServerError)
        );
    }

    #[test]
    fn test_error_category_labels() {
        assert_eq!(ErrorCategory::ClientError.label(), "client_error");
        assert_eq!(ErrorCategory::ServerError.label(), "server_error");
        assert_eq!(ErrorCategory::NetworkError.label(), "network_error");
        assert_eq!(ErrorCategory::TimeoutError.label(), "timeout_error");
        assert_eq!(ErrorCategory::TlsError.label(), "tls_error");
    }

    #[test]
    fn test_error_category_descriptions() {
        assert!(ErrorCategory::ClientError.description().contains("4xx"));
        assert!(ErrorCategory::ServerError.description().contains("5xx"));
        assert!(ErrorCategory::NetworkError
            .description()
            .contains("Network"));
    }

    #[test]
    fn test_categorized_error_from_status() {
        let err = CategorizedError::from_status(
            404,
            "Not Found".to_string(),
            Some("/api/test".to_string()),
        )
        .unwrap();

        assert_eq!(err.category, ErrorCategory::ClientError);
        assert_eq!(err.status_code, Some(404));
        assert_eq!(err.message, "Not Found");
    }

    #[test]
    fn test_categorized_error_display() {
        let err = CategorizedError::new(
            ErrorCategory::ServerError,
            "Service unavailable".to_string(),
        );

        let display = format!("{}", err);
        assert!(display.contains("server_error"));
        assert!(display.contains("Service unavailable"));
    }

    #[test]
    fn test_all_categories() {
        let categories = ErrorCategory::all();
        assert_eq!(categories.len(), 6);
        assert!(categories.contains(&ErrorCategory::ClientError));
        assert!(categories.contains(&ErrorCategory::ServerError));
    }

    #[test]
    fn test_categorize_status_code_names() {
        assert_eq!(categorize_status_code(200), "OK");
        assert_eq!(categorize_status_code(404), "Not Found");
        assert_eq!(categorize_status_code(500), "Internal Server Error");
        assert_eq!(categorize_status_code(503), "Service Unavailable");
    }
}
