//! CSV data source for data-driven testing.
//!
//! This module provides functionality to load test data from CSV files and
//! distribute rows across virtual users. Each virtual user gets its own row
//! of data, enabling realistic data-driven load testing.
//!
//! # Features
//! - Load CSV files with headers
//! - Round-robin row distribution to virtual users
//! - Thread-safe access with Arc<Mutex<>>
//! - Automatic variable substitution in scenarios
//! - Support for user credentials, product IDs, etc.

use std::collections::HashMap;
use std::fs::File;
use std::path::Path;
use std::sync::{Arc, Mutex};
use thiserror::Error;
use tracing::{debug, info, warn};

/// Errors that can occur when loading or using CSV data.
#[derive(Error, Debug)]
pub enum DataSourceError {
    #[error("Failed to read CSV file: {0}")]
    CsvReadError(#[from] csv::Error),

    #[error("Failed to open file: {0}")]
    IoError(#[from] std::io::Error),

    #[error("CSV file is empty or has no data rows")]
    EmptyData,

    #[error("CSV file has no headers")]
    NoHeaders,

    #[error("No data available (all rows consumed)")]
    NoDataAvailable,
}

/// A single row of CSV data as a map of column name -> value.
pub type DataRow = HashMap<String, String>;

/// CSV data source for data-driven testing.
///
/// Loads CSV files and provides round-robin access to rows for virtual users.
/// Each virtual user gets a unique row of data to use in their scenario.
///
/// # Example CSV File
/// ```csv
/// username,password,email
/// user1,pass123,user1@example.com
/// user2,pass456,user2@example.com
/// user3,pass789,user3@example.com
/// ```
///
/// # Example Usage
/// ```rust
/// use rust_loadtest::data_source::CsvDataSource;
///
/// let data_source = CsvDataSource::from_file("users.csv").unwrap();
/// let row = data_source.next_row().unwrap();
/// println!("Username: {}", row.get("username").unwrap());
/// ```
#[derive(Clone)]
pub struct CsvDataSource {
    /// All data rows from the CSV file
    rows: Arc<Mutex<Vec<DataRow>>>,

    /// Current index for round-robin distribution
    current_index: Arc<Mutex<usize>>,

    /// Column headers from the CSV
    headers: Vec<String>,
}

impl CsvDataSource {
    /// Load a CSV file from the given path.
    ///
    /// # Arguments
    /// * `path` - Path to the CSV file
    ///
    /// # Returns
    /// A CsvDataSource instance with all rows loaded
    ///
    /// # Errors
    /// Returns error if file cannot be read, has no headers, or is empty
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, DataSourceError> {
        let path_ref = path.as_ref();
        info!(path = ?path_ref, "Loading CSV data file");

        let file = File::open(path_ref)?;
        let mut reader = csv::Reader::from_reader(file);

        // Get headers
        let headers = reader
            .headers()?
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>();

        if headers.is_empty() {
            return Err(DataSourceError::NoHeaders);
        }

        debug!(headers = ?headers, "CSV headers loaded");

        // Read all rows
        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            let mut row = HashMap::new();

            for (i, header) in headers.iter().enumerate() {
                if let Some(value) = record.get(i) {
                    row.insert(header.clone(), value.to_string());
                }
            }

            rows.push(row);
        }

        if rows.is_empty() {
            return Err(DataSourceError::EmptyData);
        }

        info!(
            path = ?path_ref,
            rows = rows.len(),
            columns = headers.len(),
            "CSV data loaded successfully"
        );

        Ok(Self {
            rows: Arc::new(Mutex::new(rows)),
            current_index: Arc::new(Mutex::new(0)),
            headers,
        })
    }

    /// Create a data source from raw CSV string (useful for testing).
    ///
    /// # Arguments
    /// * `csv_content` - CSV content as a string with headers
    ///
    /// # Returns
    /// A CsvDataSource instance
    pub fn from_string(csv_content: &str) -> Result<Self, DataSourceError> {
        let mut reader = csv::Reader::from_reader(csv_content.as_bytes());

        // Get headers
        let headers = reader
            .headers()?
            .iter()
            .map(|h| h.to_string())
            .collect::<Vec<_>>();

        if headers.is_empty() {
            return Err(DataSourceError::NoHeaders);
        }

        // Read all rows
        let mut rows = Vec::new();
        for result in reader.records() {
            let record = result?;
            let mut row = HashMap::new();

            for (i, header) in headers.iter().enumerate() {
                if let Some(value) = record.get(i) {
                    row.insert(header.clone(), value.to_string());
                }
            }

            rows.push(row);
        }

        if rows.is_empty() {
            return Err(DataSourceError::EmptyData);
        }

        Ok(Self {
            rows: Arc::new(Mutex::new(rows)),
            current_index: Arc::new(Mutex::new(0)),
            headers,
        })
    }

    /// Get the next row in round-robin fashion.
    ///
    /// Returns rows in sequence, wrapping back to the first row after the last.
    /// Thread-safe for concurrent access by multiple virtual users.
    ///
    /// # Returns
    /// A clone of the next data row
    pub fn next_row(&self) -> Result<DataRow, DataSourceError> {
        let rows = self.rows.lock().unwrap();
        let mut index = self.current_index.lock().unwrap();

        if rows.is_empty() {
            return Err(DataSourceError::NoDataAvailable);
        }

        let row = rows[*index % rows.len()].clone();
        *index += 1;

        debug!(
            index = *index - 1,
            row_count = rows.len(),
            "Retrieved data row"
        );

        Ok(row)
    }

    /// Get a specific row by index.
    ///
    /// # Arguments
    /// * `index` - Zero-based row index
    ///
    /// # Returns
    /// A clone of the requested row, or None if index is out of bounds
    pub fn get_row(&self, index: usize) -> Option<DataRow> {
        let rows = self.rows.lock().unwrap();
        rows.get(index).cloned()
    }

    /// Get the total number of data rows.
    pub fn row_count(&self) -> usize {
        let rows = self.rows.lock().unwrap();
        rows.len()
    }

    /// Get the column headers.
    pub fn headers(&self) -> &[String] {
        &self.headers
    }

    /// Reset the row index to start from the beginning.
    pub fn reset(&self) {
        let mut index = self.current_index.lock().unwrap();
        *index = 0;
        debug!("Data source index reset to 0");
    }

    /// Get all rows (useful for inspection/debugging).
    pub fn all_rows(&self) -> Vec<DataRow> {
        let rows = self.rows.lock().unwrap();
        rows.clone()
    }

    /// Apply data from a row to a variable map.
    ///
    /// This copies all values from the data row into the provided map,
    /// making them available for variable substitution in scenarios.
    ///
    /// # Arguments
    /// * `row` - Data row to extract values from
    /// * `variables` - Target variable map to populate
    pub fn apply_row_to_variables(row: &DataRow, variables: &mut HashMap<String, String>) {
        for (key, value) in row {
            variables.insert(key.clone(), value.clone());
        }
    }
}

/// Builder for creating CSV data sources with options.
pub struct CsvDataSourceBuilder {
    path: Option<String>,
    content: Option<String>,
}

impl CsvDataSourceBuilder {
    /// Create a new builder.
    pub fn new() -> Self {
        Self {
            path: None,
            content: None,
        }
    }

    /// Set the file path to load.
    pub fn path<P: AsRef<Path>>(mut self, path: P) -> Self {
        self.path = Some(path.as_ref().to_string_lossy().to_string());
        self
    }

    /// Set CSV content directly (for testing).
    pub fn content(mut self, content: &str) -> Self {
        self.content = Some(content.to_string());
        self
    }

    /// Build the data source.
    pub fn build(self) -> Result<CsvDataSource, DataSourceError> {
        if let Some(content) = self.content {
            CsvDataSource::from_string(&content)
        } else if let Some(path) = self.path {
            CsvDataSource::from_file(path)
        } else {
            Err(DataSourceError::EmptyData)
        }
    }
}

impl Default for CsvDataSourceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TEST_CSV: &str = r#"username,password,email
user1,pass123,user1@example.com
user2,pass456,user2@example.com
user3,pass789,user3@example.com"#;

    #[test]
    fn test_from_string() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();
        assert_eq!(ds.row_count(), 3);
        assert_eq!(ds.headers(), &["username", "password", "email"]);
    }

    #[test]
    fn test_next_row_round_robin() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();

        let row1 = ds.next_row().unwrap();
        assert_eq!(row1.get("username").unwrap(), "user1");

        let row2 = ds.next_row().unwrap();
        assert_eq!(row2.get("username").unwrap(), "user2");

        let row3 = ds.next_row().unwrap();
        assert_eq!(row3.get("username").unwrap(), "user3");

        // Should wrap back to first row
        let row4 = ds.next_row().unwrap();
        assert_eq!(row4.get("username").unwrap(), "user1");
    }

    #[test]
    fn test_get_row_by_index() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();

        let row = ds.get_row(1).unwrap();
        assert_eq!(row.get("username").unwrap(), "user2");

        assert!(ds.get_row(999).is_none());
    }

    #[test]
    fn test_reset() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();

        ds.next_row().unwrap();
        ds.next_row().unwrap();

        ds.reset();

        let row = ds.next_row().unwrap();
        assert_eq!(row.get("username").unwrap(), "user1");
    }

    #[test]
    fn test_apply_row_to_variables() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();
        let row = ds.next_row().unwrap();

        let mut variables = HashMap::new();
        CsvDataSource::apply_row_to_variables(&row, &mut variables);

        assert_eq!(variables.get("username").unwrap(), "user1");
        assert_eq!(variables.get("password").unwrap(), "pass123");
        assert_eq!(variables.get("email").unwrap(), "user1@example.com");
    }

    #[test]
    fn test_empty_csv() {
        let empty_csv = "username,password\n";
        let result = CsvDataSource::from_string(empty_csv);
        assert!(result.is_err());
    }

    #[test]
    fn test_no_headers() {
        let no_headers = "";
        let result = CsvDataSource::from_string(no_headers);
        assert!(result.is_err());
    }

    #[test]
    fn test_concurrent_access() {
        use std::sync::Arc;
        use std::thread;

        let ds = Arc::new(CsvDataSource::from_string(TEST_CSV).unwrap());
        let mut handles = vec![];

        // Spawn 10 threads, each getting 5 rows
        for _ in 0..10 {
            let ds_clone = Arc::clone(&ds);
            let handle = thread::spawn(move || {
                for _ in 0..5 {
                    let row = ds_clone.next_row().unwrap();
                    assert!(row.contains_key("username"));
                }
            });
            handles.push(handle);
        }

        for handle in handles {
            handle.join().unwrap();
        }

        // Should have distributed 50 rows total across 3 users
        // Index should be at 50
        let rows = ds.all_rows();
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn test_builder_with_content() {
        let ds = CsvDataSourceBuilder::new()
            .content(TEST_CSV)
            .build()
            .unwrap();

        assert_eq!(ds.row_count(), 3);
    }

    #[test]
    fn test_all_rows() {
        let ds = CsvDataSource::from_string(TEST_CSV).unwrap();
        let rows = ds.all_rows();

        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].get("username").unwrap(), "user1");
        assert_eq!(rows[1].get("username").unwrap(), "user2");
        assert_eq!(rows[2].get("username").unwrap(), "user3");
    }
}
