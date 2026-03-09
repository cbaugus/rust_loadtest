use std::str::FromStr;
use tokio::time::Duration;

/// Parses a duration string in the format "30s", "10m", "5h", "3d".
///
/// Supported units:
/// - `s` for seconds
/// - `m` for minutes
/// - `h` for hours
/// - `d` for days
pub fn parse_duration_string(s: &str) -> Result<Duration, String> {
    let s = s.trim();

    if s.is_empty() {
        return Err("Duration string cannot be empty".to_string());
    }

    let unit_char = s.chars().last().unwrap();
    let value_str = &s[0..s.len() - 1];

    let value = match u64::from_str(value_str) {
        Ok(v) => v,
        Err(_) => {
            return Err(format!(
                "Invalid numeric value in duration: '{}'",
                value_str
            ))
        }
    };

    match unit_char {
        's' => Ok(Duration::from_secs(value)),
        'm' => Ok(Duration::from_secs(value * 60)),
        'h' => Ok(Duration::from_secs(value * 60 * 60)),
        'd' => Ok(Duration::from_secs(value * 24 * 60 * 60)),
        _ => Err(format!(
            "Unknown duration unit: '{}'. Use 's', 'm', 'h', or 'd'.",
            unit_char
        )),
    }
}

/// Parses a body size string like "512B", "512KB", or "1MB" into bytes.
///
/// Supported units:
/// - `B`  — bytes
/// - `KB` — kibibytes (× 1 024)
/// - `MB` — mebibytes (× 1 048 576)
pub fn parse_body_size(s: &str) -> Result<usize, String> {
    let s = s.trim();

    if s.is_empty() {
        return Err("Body size string cannot be empty".to_string());
    }

    let (value_str, multiplier) = if let Some(v) = s.strip_suffix("MB") {
        (v, 1024 * 1024usize)
    } else if let Some(v) = s.strip_suffix("KB") {
        (v, 1024usize)
    } else if let Some(v) = s.strip_suffix('B') {
        (v, 1usize)
    } else {
        return Err(format!(
            "Unknown body size unit in '{}'. Use 'B', 'KB', or 'MB'.",
            s
        ));
    };

    let value: usize = value_str
        .trim()
        .parse()
        .map_err(|_| format!("Invalid numeric value in body size: '{}'", value_str.trim()))?;

    if value == 0 {
        return Err("Body size must be greater than zero".to_string());
    }

    Ok(value * multiplier)
}

/// Parses a comma-separated header string with support for escaped commas.
///
/// Use `\,` to include a literal comma in a header value.
/// Example: "Connection:keep-alive,Keep-Alive:timeout=5\,max=200"
pub fn parse_headers_with_escapes(headers_str: &str) -> Vec<String> {
    let mut headers = Vec::new();
    let mut current_header = String::new();
    let mut chars = headers_str.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '\\' => {
                // Check if the next character is a comma
                if chars.peek() == Some(&',') {
                    // This is an escaped comma, add it to the current header
                    current_header.push(',');
                    chars.next(); // Consume the comma
                } else {
                    // Not escaping a comma, keep the backslash
                    current_header.push('\\');
                }
            }
            ',' => {
                // This is a header separator
                if !current_header.trim().is_empty() {
                    headers.push(current_header.clone());
                }
                current_header.clear();
            }
            _ => {
                current_header.push(ch);
            }
        }
    }

    // Don't forget the last header
    if !current_header.trim().is_empty() {
        headers.push(current_header);
    }

    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- parse_duration_string tests ---

    mod duration {
        use super::*;

        #[test]
        fn parse_minutes() {
            assert_eq!(
                parse_duration_string("10m").unwrap(),
                Duration::from_secs(600)
            );
        }

        #[test]
        fn parse_hours() {
            assert_eq!(
                parse_duration_string("5h").unwrap(),
                Duration::from_secs(18000)
            );
        }

        #[test]
        fn parse_days() {
            assert_eq!(
                parse_duration_string("3d").unwrap(),
                Duration::from_secs(259200)
            );
        }

        #[test]
        fn parse_one_minute() {
            assert_eq!(
                parse_duration_string("1m").unwrap(),
                Duration::from_secs(60)
            );
        }

        #[test]
        fn parse_zero_minutes() {
            assert_eq!(parse_duration_string("0m").unwrap(), Duration::from_secs(0));
        }

        #[test]
        fn parse_large_value() {
            assert_eq!(
                parse_duration_string("365d").unwrap(),
                Duration::from_secs(365 * 86400)
            );
        }

        #[test]
        fn trims_whitespace() {
            assert_eq!(
                parse_duration_string("  10m  ").unwrap(),
                Duration::from_secs(600)
            );
        }

        #[test]
        fn empty_string_errors() {
            let err = parse_duration_string("").unwrap_err();
            assert!(err.contains("empty"), "error was: {}", err);
        }

        #[test]
        fn whitespace_only_errors() {
            let err = parse_duration_string("   ").unwrap_err();
            assert!(err.contains("empty"), "error was: {}", err);
        }

        #[test]
        fn unknown_suffix_errors() {
            let err = parse_duration_string("10x").unwrap_err();
            assert!(err.contains("Unknown duration unit"), "error was: {}", err);
        }

        #[test]
        fn parse_seconds() {
            assert_eq!(
                parse_duration_string("30s").unwrap(),
                Duration::from_secs(30)
            );
        }

        #[test]
        fn no_suffix_errors() {
            let err = parse_duration_string("10").unwrap_err();
            assert!(err.contains("Unknown duration unit"), "error was: {}", err);
        }

        #[test]
        fn no_number_errors() {
            let err = parse_duration_string("m").unwrap_err();
            assert!(err.contains("Invalid numeric"), "error was: {}", err);
        }

        #[test]
        fn fractional_number_errors() {
            let err = parse_duration_string("5.5h").unwrap_err();
            assert!(err.contains("Invalid numeric"), "error was: {}", err);
        }

        #[test]
        fn negative_number_errors() {
            let err = parse_duration_string("-5m").unwrap_err();
            assert!(err.contains("Invalid numeric"), "error was: {}", err);
        }

        #[test]
        fn letters_as_number_errors() {
            let err = parse_duration_string("abcm").unwrap_err();
            assert!(err.contains("Invalid numeric"), "error was: {}", err);
        }
    }

    // --- parse_headers_with_escapes tests ---

    #[test]
    fn test_parse_headers_simple() {
        let headers_str = "Content-Type:application/json,Authorization:Bearer token";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Content-Type:application/json");
        assert_eq!(result[1], "Authorization:Bearer token");
    }

    #[test]
    fn test_parse_headers_with_escaped_comma() {
        let headers_str = "Connection:keep-alive,Keep-Alive:timeout=5\\,max=200";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Connection:keep-alive");
        assert_eq!(result[1], "Keep-Alive:timeout=5,max=200");
    }

    #[test]
    fn test_parse_headers_multiple_escaped_commas() {
        let headers_str =
            "Accept:text/html\\,application/xml\\,application/json,User-Agent:Mozilla/5.0";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(
            result[0],
            "Accept:text/html,application/xml,application/json"
        );
        assert_eq!(result[1], "User-Agent:Mozilla/5.0");
    }

    #[test]
    fn test_parse_headers_backslash_not_before_comma() {
        let headers_str = "Path:C:\\Users\\test,Host:example.com";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Path:C:\\Users\\test");
        assert_eq!(result[1], "Host:example.com");
    }

    #[test]
    fn test_parse_headers_empty_and_whitespace() {
        let headers_str = "  Header1:value1  ,  ,  Header2:value2  ";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "  Header1:value1  ");
        assert_eq!(result[1], "  Header2:value2  ");
    }

    #[test]
    fn test_parse_headers_trailing_comma() {
        let headers_str = "Header1:value1,Header2:value2,";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Header1:value1");
        assert_eq!(result[1], "Header2:value2");
    }

    #[test]
    fn test_parse_headers_complex_keep_alive() {
        let headers_str =
            "Connection:keep-alive\\,close,Keep-Alive:timeout=5\\,max=1000\\,custom=value";
        let result = parse_headers_with_escapes(headers_str);

        assert_eq!(result.len(), 2);
        assert_eq!(result[0], "Connection:keep-alive,close");
        assert_eq!(result[1], "Keep-Alive:timeout=5,max=1000,custom=value");
    }

    // --- parse_body_size tests ---

    mod body_size {
        use super::*;

        #[test]
        fn parse_bytes() {
            assert_eq!(parse_body_size("128B").unwrap(), 128);
        }

        #[test]
        fn parse_kilobytes() {
            assert_eq!(parse_body_size("512KB").unwrap(), 512 * 1024);
        }

        #[test]
        fn parse_megabytes() {
            assert_eq!(parse_body_size("1MB").unwrap(), 1024 * 1024);
        }

        #[test]
        fn parse_large_mb() {
            assert_eq!(parse_body_size("10MB").unwrap(), 10 * 1024 * 1024);
        }

        #[test]
        fn trims_whitespace() {
            assert_eq!(parse_body_size("  256KB  ").unwrap(), 256 * 1024);
        }

        #[test]
        fn zero_errors() {
            let err = parse_body_size("0KB").unwrap_err();
            assert!(err.contains("greater than zero"), "error was: {}", err);
        }

        #[test]
        fn empty_errors() {
            let err = parse_body_size("").unwrap_err();
            assert!(err.contains("empty"), "error was: {}", err);
        }

        #[test]
        fn unknown_unit_errors() {
            let err = parse_body_size("1GB").unwrap_err();
            assert!(err.contains("Unknown body size unit"), "error was: {}", err);
        }

        #[test]
        fn invalid_number_errors() {
            let err = parse_body_size("abcKB").unwrap_err();
            assert!(err.contains("Invalid numeric"), "error was: {}", err);
        }
    }
}
