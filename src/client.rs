use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::fs::File;
use std::io::Read;
use std::net::SocketAddr;
use std::str::FromStr;

use crate::connection_pool::PoolConfig;
use crate::utils::parse_headers_with_escapes;

/// Configuration for building the HTTP client.
pub struct ClientConfig {
    pub skip_tls_verify: bool,
    pub resolve_target_addr: Option<String>,
    pub client_cert_path: Option<String>,
    pub client_key_path: Option<String>,
    pub custom_headers: Option<String>,
    pub pool_config: Option<PoolConfig>,
}

/// Result of building the client, includes parsed headers for logging.
pub struct ClientBuildResult {
    pub client: reqwest::Client,
    pub parsed_headers: HeaderMap,
}

/// Builds a reqwest HTTP client with the specified configuration.
pub fn build_client(
    config: &ClientConfig,
) -> Result<ClientBuildResult, Box<dyn std::error::Error + Send + Sync>> {
    let mut client_builder = reqwest::Client::builder();

    // DNS Override Configuration
    if let Some(ref resolve_str) = config.resolve_target_addr {
        if !resolve_str.is_empty() {
            client_builder = configure_dns_override(client_builder, resolve_str)?;
        } else {
            println!("RESOLVE_TARGET_ADDR is set but empty, no DNS override will be applied.");
        }
    }

    // mTLS Configuration
    client_builder = configure_mtls(
        client_builder,
        config.client_cert_path.as_deref(),
        config.client_key_path.as_deref(),
    )?;

    // Custom Headers Configuration
    let parsed_headers = configure_custom_headers(config.custom_headers.as_deref())?;
    if !parsed_headers.is_empty() {
        client_builder = client_builder.default_headers(parsed_headers.clone());
        println!("Successfully configured custom default headers.");
    }

    // Connection Pool Configuration
    let pool_config = config.pool_config.clone().unwrap_or_default();
    client_builder = pool_config.apply_to_builder(client_builder);
    println!(
        "Connection pool configured: max_idle_per_host={}, idle_timeout={:?}",
        pool_config.max_idle_per_host, pool_config.idle_timeout
    );

    // Build client with TLS settings
    let client = if config.skip_tls_verify {
        println!("WARNING: Skipping TLS certificate verification.");
        client_builder
            .danger_accept_invalid_certs(true)
            .danger_accept_invalid_hostnames(true)
            .build()?
    } else {
        client_builder.build()?
    };

    Ok(ClientBuildResult {
        client,
        parsed_headers,
    })
}

fn configure_dns_override(
    mut client_builder: reqwest::ClientBuilder,
    resolve_str: &str,
) -> Result<reqwest::ClientBuilder, Box<dyn std::error::Error + Send + Sync>> {
    println!(
        "Attempting to apply DNS override from RESOLVE_TARGET_ADDR: {}",
        resolve_str
    );

    let parts: Vec<&str> = resolve_str.split(':').collect();
    if parts.len() != 3 {
        return Err(format!(
            "RESOLVE_TARGET_ADDR environment variable ('{}') is not in the expected format 'hostname:ip:port'",
            resolve_str
        ).into());
    }

    let hostname_to_override = parts[0].trim();
    let ip_to_resolve_to = parts[1].trim();
    let port_to_connect_to_str = parts[2].trim();

    if hostname_to_override.is_empty() {
        return Err(
            "RESOLVE_TARGET_ADDR: hostname part cannot be empty. Format: 'hostname:ip:port'".into(),
        );
    }
    if ip_to_resolve_to.is_empty() {
        return Err(
            "RESOLVE_TARGET_ADDR: IP address part cannot be empty. Format: 'hostname:ip:port'"
                .into(),
        );
    }
    if port_to_connect_to_str.is_empty() {
        return Err(
            "RESOLVE_TARGET_ADDR: port part cannot be empty. Format: 'hostname:ip:port'".into(),
        );
    }

    let port_to_connect_to: u16 = port_to_connect_to_str.parse().map_err(|e| {
        format!(
            "Failed to parse port '{}' in RESOLVE_TARGET_ADDR: {}. Must be a valid u16. Format: 'hostname:ip:port'",
            port_to_connect_to_str, e
        )
    })?;

    let socket_addr_str = format!("{}:{}", ip_to_resolve_to, port_to_connect_to);
    let socket_addr: SocketAddr = socket_addr_str.parse().map_err(|e| {
        format!(
            "Failed to parse IP/Port '{}' into SocketAddr for RESOLVE_TARGET_ADDR: {}. Ensure IP and port are valid. Format: 'hostname:ip:port'",
            socket_addr_str, e
        )
    })?;

    client_builder = client_builder.resolve(hostname_to_override, socket_addr);
    println!(
        "Successfully configured DNS override: '{}' will resolve to {}",
        hostname_to_override, socket_addr
    );

    Ok(client_builder)
}

fn configure_mtls(
    mut client_builder: reqwest::ClientBuilder,
    cert_path: Option<&str>,
    key_path: Option<&str>,
) -> Result<reqwest::ClientBuilder, Box<dyn std::error::Error + Send + Sync>> {
    match (cert_path, key_path) {
        (Some(cert_path), Some(key_path)) => {
            println!("Attempting to load mTLS certificate from: {}", cert_path);
            println!("Attempting to load mTLS private key from: {}", key_path);

            let mut cert_file = File::open(cert_path).map_err(|e| {
                format!(
                    "Failed to open client certificate file '{}': {}",
                    cert_path, e
                )
            })?;
            let mut cert_pem_buf = Vec::new();
            cert_file.read_to_end(&mut cert_pem_buf).map_err(|e| {
                format!(
                    "Failed to read client certificate file '{}': {}",
                    cert_path, e
                )
            })?;

            let mut key_file = File::open(key_path)
                .map_err(|e| format!("Failed to open client key file '{}': {}", key_path, e))?;
            let mut key_pem_buf = Vec::new();
            key_file
                .read_to_end(&mut key_pem_buf)
                .map_err(|e| format!("Failed to read client key file '{}': {}", key_path, e))?;

            // Validate certificate PEM
            let mut cert_pem_cursor = std::io::Cursor::new(cert_pem_buf.as_slice());
            let certs_result: Vec<_> = rustls_pemfile::certs(&mut cert_pem_cursor).collect();
            if certs_result.is_empty() {
                return Err(format!("No PEM certificates found in {}", cert_path).into());
            }
            for cert in certs_result {
                if let Err(e) = cert {
                    return Err(format!(
                        "Failed to parse PEM certificates from '{}': {}",
                        cert_path, e
                    )
                    .into());
                }
            }

            // Validate private key PEM (must be PKCS#8)
            let mut key_pem_cursor = std::io::Cursor::new(key_pem_buf.as_slice());
            let keys_result: Vec<_> =
                rustls_pemfile::pkcs8_private_keys(&mut key_pem_cursor).collect();
            if keys_result.is_empty() {
                return Err(format!(
                    "No PKCS#8 private keys found in '{}'. Ensure the file contains a valid PEM-encoded PKCS#8 private key.",
                    key_path
                ).into());
            }
            for key in keys_result {
                if let Err(e) = key {
                    return Err(format!(
                        "Failed to parse private key from '{}' as PKCS#8: {}. Please ensure the key is PEM-encoded and in PKCS#8 format.",
                        key_path, e
                    ).into());
                }
            }

            // Combine certificate PEM and key PEM into one buffer
            let mut combined_pem_buf = Vec::new();
            combined_pem_buf.extend_from_slice(&cert_pem_buf);
            if !cert_pem_buf.ends_with(b"\n") && !key_pem_buf.starts_with(b"\n") {
                combined_pem_buf.push(b'\n');
            }
            combined_pem_buf.extend_from_slice(&key_pem_buf);

            let identity = reqwest::Identity::from_pem(&combined_pem_buf)
                .map_err(|e| format!(
                    "Failed to create reqwest::Identity from PEM (cert+key): {}. Ensure the key is PKCS#8 and the certificate is valid.",
                    e
                ))?;

            client_builder = client_builder.identity(identity);
            println!("Successfully configured mTLS with client certificate and key.");
        }
        (Some(_), None) => {
            return Err("CLIENT_CERT_PATH is set, but CLIENT_KEY_PATH is missing for mTLS.".into());
        }
        (None, Some(_)) => {
            return Err("CLIENT_KEY_PATH is set, but CLIENT_CERT_PATH is missing for mTLS.".into());
        }
        (None, None) => {
            // No mTLS configured
        }
    }

    Ok(client_builder)
}

fn configure_custom_headers(
    custom_headers_str: Option<&str>,
) -> Result<HeaderMap, Box<dyn std::error::Error + Send + Sync>> {
    let mut parsed_headers = HeaderMap::new();

    let headers_str = match custom_headers_str {
        Some(s) if !s.is_empty() => s,
        _ => return Ok(parsed_headers),
    };

    println!("Attempting to parse CUSTOM_HEADERS: {}", headers_str);

    let header_pairs = parse_headers_with_escapes(headers_str);

    for header_pair_str in header_pairs {
        let header_pair_str_trimmed = header_pair_str.trim();
        if header_pair_str_trimmed.is_empty() {
            continue;
        }

        let parts: Vec<&str> = header_pair_str_trimmed.splitn(2, ':').collect();
        if parts.len() != 2 {
            return Err(format!(
                "Invalid header format in CUSTOM_HEADERS: '{}'. Expected 'Name:Value'.",
                header_pair_str_trimmed
            )
            .into());
        }

        let name_str = parts[0].trim();
        let value_str = parts[1].trim();

        if name_str.is_empty() {
            return Err(format!(
                "Invalid header format: Header name cannot be empty in '{}'.",
                header_pair_str_trimmed
            )
            .into());
        }

        let unescaped_value = value_str.replace("\\,", ",");

        let header_name = HeaderName::from_str(name_str)
            .map_err(|e| format!("Invalid header name: {}. Name: '{}'", e, name_str))?;
        let header_value = HeaderValue::from_str(&unescaped_value).map_err(|e| {
            format!(
                "Invalid header value for '{}': {}. Value: '{}'",
                name_str, e, unescaped_value
            )
        })?;

        parsed_headers.insert(header_name, header_value);
    }

    Ok(parsed_headers)
}
