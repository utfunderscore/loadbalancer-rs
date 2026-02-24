use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use thiserror::Error;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum LoadBalanceMode {
    Static,
    Geo,
    Http,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum StaticAlgorithm {
    RoundRobin,
    LowestPlayerCount,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Server {
    pub name: String,
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct StaticConfig {
    pub algorithm: StaticAlgorithm,
    pub servers: Vec<Server>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct RegionServer {
    pub address: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub port: Option<u16>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct GeoConfig {
    pub token: String,
    pub regions: HashMap<String, RegionServer>,
    pub fallback: RegionServer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HttpConfig {
    pub endpoint: String,
    pub request_method: String,
    pub headers: HashMap<String, String>,
    pub fallback: RegionServer,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoadBalancerConfig {
    pub mode: LoadBalanceMode,
    pub motd: String,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub static_config: Option<StaticConfig>,

    #[serde(rename = "static", skip_serializing_if = "Option::is_none")]
    pub static_config_raw: Option<StaticConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo_config: Option<GeoConfig>,

    #[serde(rename = "geo", skip_serializing_if = "Option::is_none")]
    pub geo_config_raw: Option<GeoConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_config: Option<HttpConfig>,

    #[serde(rename = "http", skip_serializing_if = "Option::is_none")]
    pub http_config_raw: Option<HttpConfig>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u32>,

    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<String>,
}

#[derive(Debug, Error)]
#[error("Error loading config because:")]
pub struct ConfigErrorReport {
    pub errors: Vec<ConfigFieldError>,
    pub source_path: Option<PathBuf>,
    pub source_content: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ConfigFieldError {
    pub field_path: String,
    pub message: String,
    pub line: Option<usize>,
    pub column: Option<usize>,
}

impl std::fmt::Display for ConfigFieldError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "    - '{}'", self.field_path)?;

        if let (Some(line), Some(column)) = (self.line, self.column) {
            write!(f, " at line {}, column {}: ", line, column)?;
        } else {
            write!(f, ": ")?;
        }

        write!(f, "{}", self.message)
    }
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    FileReadError(#[from] std::io::Error),

    #[error("Failed to parse YAML: {message}")]
    YamlParseError {
        line: usize,
        column: usize,
        message: String,
    },

    #[error("Missing configuration for {0} mode")]
    MissingModeConfig(String),

    #[error("{0}")]
    ValidationError(ConfigErrorReport),

    #[error("Failed to write config file: {0}")]
    FileWriteError(String),
}
impl LoadBalancerConfig {
    /// Loads the configuration from the specified file path
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let content = fs::read_to_string(path).map_err(ConfigError::FileReadError)?;

        let mut config: LoadBalancerConfig = match serde_yaml::from_str(&content) {
            Ok(config) => config,
            Err(err) => {
                if let Some(location) = err.location() {
                    return Err(ConfigError::YamlParseError {
                        line: location.line(),
                        column: location.column(),
                        message: err.to_string(),
                    });
                } else {
                    // Create a validation error report if we can't get line/column
                    let report = ConfigErrorReport {
                        errors: vec![ConfigFieldError {
                            field_path: "yaml_syntax".to_string(),
                            message: err.to_string(),
                            line: None,
                            column: None,
                        }],
                        source_path: Some(path.to_path_buf()),
                        source_content: Some(content),
                    };
                    return Err(ConfigError::ValidationError(report));
                }
            }
        };

        // Move raw configs to their proper fields
        if config.static_config_raw.is_some() {
            config.static_config = config.static_config_raw.take();
        }

        if config.geo_config_raw.is_some() {
            config.geo_config = config.geo_config_raw.take();
        }

        if config.http_config_raw.is_some() {
            config.http_config = config.http_config_raw.take();
        }

        // Validate the configuration and collect all errors
        let mut validation_errors = Vec::new();

        // Collect validation errors with enhanced position information
        match config.mode {
            LoadBalanceMode::Static => {
                if config.static_config.is_none() {
                    let line_col = find_field_position(&content, "mode");
                    validation_errors.push(ConfigFieldError {
                        field_path: "static".to_string(),
                        message: "Missing static configuration section".to_string(),
                        line: line_col.map(|(l, _)| l),
                        column: line_col.map(|(_, c)| c),
                    });
                } else if let Some(static_config) = &config.static_config {
                    if static_config.servers.is_empty() {
                        let line_col = find_field_position(&content, "static");
                        validation_errors.push(ConfigFieldError {
                            field_path: "static.servers".to_string(),
                            message: "Static mode requires at least one server".to_string(),
                            line: line_col.map(|(l, _)| l),
                            column: line_col.map(|(_, c)| c),
                        });
                    }

                    // Validate each server in the static config
                    for (i, server) in static_config.servers.iter().enumerate() {
                        if server.name.is_empty() {
                            validation_errors.push(ConfigFieldError {
                                field_path: format!("static.servers[{}].name", i),
                                message: "Server name cannot be empty".to_string(),
                                line: None, // We'd need more complex parsing to find exact positions
                                column: None,
                            });
                        }

                        if server.address.is_empty() {
                            validation_errors.push(ConfigFieldError {
                                field_path: format!("static.servers[{}].address", i),
                                message: "Server address cannot be empty".to_string(),
                                line: None,
                                column: None,
                            });
                        }
                    }
                }
            }
            LoadBalanceMode::Geo => {
                if config.geo_config.is_none() {
                    let line_col = find_field_position(&content, "mode");
                    validation_errors.push(ConfigFieldError {
                        field_path: "geo".to_string(),
                        message: "Missing geo configuration section".to_string(),
                        line: line_col.map(|(l, _)| l),
                        column: line_col.map(|(_, c)| c),
                    });
                } else if let Some(geo_config) = &config.geo_config {
                    if geo_config.regions.is_empty() {
                        let line_col = find_field_position(&content, "geo");
                        validation_errors.push(ConfigFieldError {
                            field_path: "geo.regions".to_string(),
                            message: "Geo mode requires at least one region".to_string(),
                            line: line_col.map(|(l, _)| l),
                            column: line_col.map(|(_, c)| c),
                        });
                    }

                    if geo_config.token.trim().is_empty() || geo_config.token == "YOUR-TOKEN" {
                        let line_col = find_field_position(&content, "token");
                        validation_errors.push(ConfigFieldError {
                            field_path: "geo.token".to_string(),
                            message: "Geo mode requires a valid API token".to_string(),
                            line: line_col.map(|(l, _)| l),
                            column: line_col.map(|(_, c)| c),
                        });
                    }
                }
            }
            LoadBalanceMode::Http => {
                if config.http_config.is_none() {
                    let line_col = find_field_position(&content, "mode");
                    validation_errors.push(ConfigFieldError {
                        field_path: "http".to_string(),
                        message: "Missing HTTP configuration section".to_string(),
                        line: line_col.map(|(l, _)| l),
                        column: line_col.map(|(_, c)| c),
                    });
                } else if let Some(http_config) = &config.http_config {
                    if !http_config.endpoint.starts_with("http") {
                        let line_col = find_field_position(&content, "endpoint");
                        validation_errors.push(ConfigFieldError {
                            field_path: "http.endpoint".to_string(),
                            message: "HTTP endpoint must start with http:// or https://"
                                .to_string(),
                            line: line_col.map(|(l, _)| l),
                            column: line_col.map(|(_, c)| c),
                        });
                    }

                    let method = http_config.request_method.to_uppercase();
                    if method != "GET" && method != "POST" {
                        let line_col = find_field_position(&content, "request_method");
                        validation_errors.push(ConfigFieldError {
                            field_path: "http.request_method".to_string(),
                            message: format!("Invalid HTTP request method: {}", method),
                            line: line_col.map(|(l, _)| l),
                            column: line_col.map(|(_, c)| c),
                        });
                    }
                }
            }
        }

        if !validation_errors.is_empty() {
            let report = ConfigErrorReport {
                errors: validation_errors,
                source_path: Some(path.to_path_buf()),
                source_content: Some(content),
            };
            return Err(ConfigError::ValidationError(report));
        }

        Ok(config)
    }

    /// Validates that the configuration is consistent
    pub fn validate(&self) -> Result<(), ConfigError> {
        let mut errors = Vec::new();

        match self.mode {
            LoadBalanceMode::Static => {
                if self.static_config.is_none() {
                    errors.push(ConfigFieldError {
                        field_path: "static".to_string(),
                        message: "Missing static configuration section".to_string(),
                        line: None,
                        column: None,
                    });
                } else if let Some(static_config) = &self.static_config
                    && static_config.servers.is_empty()
                {
                    errors.push(ConfigFieldError {
                        field_path: "static.servers".to_string(),
                        message: "Static mode requires at least one server".to_string(),
                        line: None,
                        column: None,
                    });
                }
            }
            LoadBalanceMode::Geo => {
                if self.geo_config.is_none() {
                    errors.push(ConfigFieldError {
                        field_path: "geo".to_string(),
                        message: "Missing geo configuration section".to_string(),
                        line: None,
                        column: None,
                    });
                } else if let Some(geo_config) = &self.geo_config {
                    if geo_config.regions.is_empty() {
                        errors.push(ConfigFieldError {
                            field_path: "geo.regions".to_string(),
                            message: "Geo mode requires at least one region".to_string(),
                            line: None,
                            column: None,
                        });
                    }

                    if geo_config.token.trim().is_empty() || geo_config.token == "YOUR-TOKEN" {
                        errors.push(ConfigFieldError {
                            field_path: "geo.token".to_string(),
                            message: "Geo mode requires a valid API token".to_string(),
                            line: None,
                            column: None,
                        });
                    }
                }
            }
            LoadBalanceMode::Http => {
                if self.http_config.is_none() {
                    errors.push(ConfigFieldError {
                        field_path: "http".to_string(),
                        message: "Missing HTTP configuration section".to_string(),
                        line: None,
                        column: None,
                    });
                } else if let Some(http_config) = &self.http_config {
                    if !http_config.endpoint.starts_with("http") {
                        errors.push(ConfigFieldError {
                            field_path: "http.endpoint".to_string(),
                            message: "HTTP endpoint must start with http:// or https://"
                                .to_string(),
                            line: None,
                            column: None,
                        });
                    }

                    let method = http_config.request_method.to_uppercase();
                    if method != "GET" && method != "POST" {
                        errors.push(ConfigFieldError {
                            field_path: "http.request_method".to_string(),
                            message: format!("Invalid HTTP request method: {}", method),
                            line: None,
                            column: None,
                        });
                    }
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(ConfigError::ValidationError(ConfigErrorReport {
                errors,
                source_path: None,
                source_content: None,
            }))
        }
    }

    /// Saves the configuration to the specified file path
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), ConfigError> {
        // Create a version of the config with the appropriate fields populated for serialization
        let mut output_config = self.clone();

        // Move the configs to their raw fields for proper YAML output
        match self.mode {
            LoadBalanceMode::Static => {
                output_config.static_config_raw = output_config.static_config.clone();
                output_config.static_config = None;
                output_config.geo_config_raw = None;
                output_config.http_config_raw = None;
            }
            LoadBalanceMode::Geo => {
                output_config.geo_config_raw = output_config.geo_config.clone();
                output_config.geo_config = None;
                output_config.static_config_raw = None;
                output_config.http_config_raw = None;
            }
            LoadBalanceMode::Http => {
                output_config.http_config_raw = output_config.http_config.clone();
                output_config.http_config = None;
                output_config.static_config_raw = None;
                output_config.geo_config_raw = None;
            }
        }

        // Add comments to the YAML
        let yaml = match serde_yaml::to_string(&output_config) {
            Ok(yaml) => yaml,
            Err(e) => return Err(ConfigError::FileWriteError(e.to_string())),
        };

        // Create the final output with comments
        let output = format!(
            "# Minecraft Server Load Balancer Configuration\n\
            # --------------------------------------------\n\
            # Select one of the modes below: 'static', 'geo', or 'http'\n\
            \n\
            {}",
            yaml
        );

        match fs::write(&path, output) {
            Ok(_) => Ok(()),
            Err(e) => Err(ConfigError::FileWriteError(e.to_string())),
        }
    }

    pub fn get_log_level(&self) -> log::Level {
        match self
            .log_level
            .as_deref()
            .unwrap_or("info")
            .to_lowercase()
            .as_str()
        {
            "error" => log::Level::Error,
            "warn" | "warning" => log::Level::Warn,
            "info" => log::Level::Info,
            "debug" => log::Level::Debug,
            "trace" => log::Level::Trace,
            _ => log::Level::Info,
        }
    }

    /// Creates a new config with default static configuration
    pub fn default_static() -> Self {
        LoadBalancerConfig {
            mode: LoadBalanceMode::Static,
            motd: "Minecraft Load Balancer".to_string(),
            static_config: Some(StaticConfig {
                algorithm: StaticAlgorithm::RoundRobin,
                servers: vec![
                    Server {
                        name: "Server 1".to_string(),
                        address: "mc1.example.com".to_string(),
                        port: None,
                    },
                    Server {
                        name: "Server 2".to_string(),
                        address: "mc2.example.com".to_string(),
                        port: None,
                    },
                ],
            }),
            static_config_raw: None,
            geo_config: None,
            geo_config_raw: None,
            http_config: None,
            http_config_raw: None,
            timeout_seconds: Some(5),
            log_level: Some("info".to_string()),
        }
    }

    /// Creates a new config with default geo configuration
    pub fn default_geo() -> Self {
        let mut regions = HashMap::new();
        regions.insert(
            "NA".to_string(),
            RegionServer {
                address: "us.example.com".to_string(),
                port: None,
            },
        );
        regions.insert(
            "EU".to_string(),
            RegionServer {
                address: "eu.example.com".to_string(),
                port: None,
            },
        );

        LoadBalancerConfig {
            mode: LoadBalanceMode::Geo,
            motd: "Minecraft Load Balancer".to_string(),
            static_config: None,
            static_config_raw: None,
            geo_config: Some(GeoConfig {
                token: "YOUR-GEOLOCATION-API-TOKEN".to_string(),
                regions,
                fallback: RegionServer {
                    address: "fallback.example.com".to_string(),
                    port: None,
                },
            }),
            geo_config_raw: None,
            http_config: None,
            http_config_raw: None,
            timeout_seconds: Some(5),
            log_level: Some("info".to_string()),
        }
    }

    /// Creates a new config with default http configuration
    pub fn default_http() -> Self {
        let mut headers = HashMap::new();
        headers.insert(
            "Authorization".to_string(),
            "Bearer YOUR_API_TOKEN".to_string(),
        );

        LoadBalancerConfig {
            mode: LoadBalanceMode::Http,
            motd: "Minecraft Load Balancer".to_string(),
            static_config: None,
            static_config_raw: None,
            geo_config: None,
            geo_config_raw: None,
            http_config: Some(HttpConfig {
                endpoint: "https://serverselector.example.com/getserver".to_string(),
                request_method: "GET".to_string(),
                headers,
                fallback: RegionServer {
                    address: "fallback.example.com".to_string(),
                    port: Some(25565),
                },
            }),
            http_config_raw: None,
            timeout_seconds: Some(5),
            log_level: Some("info".to_string()),
        }
    }
}

/// Helper function to find a field's position in the YAML content
fn find_field_position(content: &str, field_name: &str) -> Option<(usize, usize)> {
    for (line_num, line) in content.lines().enumerate() {
        if let Some(col) = line.find(&format!("{}:", field_name)) {
            return Some((line_num + 1, col + 1)); // 1-based line and column
        }
    }
    None
}

use std::fmt::Write;

/// Function to print a user-friendly error report to stderr
pub fn print_error_report(report: &ConfigErrorReport) {
    eprintln!("{}", format_concise_error_report(report));
}

/// Function to generate a more concise error report without source code context
pub fn format_concise_error_report(report: &ConfigErrorReport) -> String {
    let mut output = String::new();

    let _ = writeln!(output, "Error loading config because:");

    for error in &report.errors {
        let location = match (error.line, error.column) {
            (Some(line), Some(col)) => format!(" (line {}, column {})", line, col),
            (Some(line), None) => format!(" (line {})", line),
            _ => String::new(),
        };

        let _ = writeln!(
            output,
            "    - '{}'{}: {}",
            error.field_path, location, error.message
        );
    }

    if let Some(path) = &report.source_path {
        let _ = writeln!(output, "    in file: {}", path.display());
    }

    output
}
