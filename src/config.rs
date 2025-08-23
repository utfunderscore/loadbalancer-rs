// Single-file configuration loader for the Minecraft load balancer.
// This ONLY defines data structures, parsing, and validation.
// No load balancing logic is included.
//
// Dependencies you need in Cargo.toml:
//
// [dependencies]
// serde = { version = "1.0", features = ["derive"] }
// serde_yaml = "0.9"
// serde_json = "1.0"          # (only if you also want JSON support; optional)
// thiserror = "1.0"
//
// Usage:
// let yaml = std::fs::read_to_string("config.yaml")?;
// let cfg = Config::from_yaml_str(&yaml)?;
// cfg.validate()?; // (already called inside from_yaml_* helpers)
// println!("Mode: {:?}", cfg.mode);

use serde::{Deserialize, Serialize};
use std::{collections::HashMap, fs, path::Path};
use thiserror::Error;

/* ---------------- Errors ---------------- */

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("IO error reading config: {0}")]
    Io(#[from] std::io::Error),
    #[error("YAML parse error: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Invalid configuration: {0}")]
    Invalid(String),
}

/* ---------------- Basic Types ---------------- */

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    Static,
    Geo,
    Http,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Algorithm {
    RoundRobin,
    LowestPlayerCount,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum HttpMethod {
    #[default]
    GET,
    POST,
}

fn default_port() -> u16 {
    25565
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Server {
    pub name: Option<String>,
    pub address: String,
    #[serde(default = "default_port")]
    pub port: u16,
}

/* ---------------- Section Structures ---------------- */

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct StaticConfig {
    pub algorithm: Algorithm,
    pub servers: Vec<Server>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct GeoConfig {
    pub token: String,
    pub regions: HashMap<String, Server>, // keys like "NA", "EU"
    pub fallback: Server,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct HttpConfig {
    pub endpoint: String,
    #[serde(default)]
    pub request_method: HttpMethod,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub fallback: Server,
}

/* ---------------- Root Config ---------------- */

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub mode: Mode,

    // "static" and "http" are reserved words in Rust, so use rename.
    #[serde(rename = "static")]
    pub static_cfg: Option<StaticConfig>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub geo: Option<GeoConfig>,
    #[serde(rename = "http")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub http_cfg: Option<HttpConfig>,

    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub timeout_seconds: Option<u64>,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub log_level: Option<LogLevel>,
}

impl Config {
    // Load from a YAML file path (blocking).
    pub fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let raw = fs::read_to_string(path)?;
        Self::from_yaml_str(&raw)
    }

    // Parse from a YAML string.
    pub fn from_yaml_str(s: &str) -> Result<Self, ConfigError> {
        let cfg: Config = serde_yaml::from_str(s)?;
        cfg.validate()?;
        Ok(cfg)
    }

    // (Optional) JSON loader if you ever want it.
    #[allow(dead_code)]
    pub fn from_json_str(s: &str) -> Result<Self, ConfigError> {
        let cfg: Config = serde_json::from_str(s)?;
        cfg.validate()?;
        Ok(cfg)
    }

    // Validate internal consistency.
    pub fn validate(&self) -> Result<(), ConfigError> {
        match self.mode {
            Mode::Static => {
                let sc = self.static_cfg.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("mode 'static' requires a 'static' section".into())
                })?;
                if sc.servers.is_empty() {
                    return Err(ConfigError::Invalid(
                        "static.servers must contain at least one server".into(),
                    ));
                }
            }
            Mode::Geo => {
                let gc = self.geo.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("mode 'geo' requires a 'geo' section".into())
                })?;
                if gc.regions.is_empty() {
                    return Err(ConfigError::Invalid(
                        "geo.regions must contain at least one region entry".into(),
                    ));
                }
            }
            Mode::Http => {
                let hc = self.http_cfg.as_ref().ok_or_else(|| {
                    ConfigError::Invalid("mode 'http' requires an 'http' section".into())
                })?;
                if hc.endpoint.trim().is_empty() {
                    return Err(ConfigError::Invalid("http.endpoint cannot be empty".into()));
                }
            }
        }
        Ok(())
    }

    pub fn timeout(&self) -> u64 {
        self.timeout_seconds.unwrap_or(5)
    }

    pub fn log_level(&self) -> LogLevel {
        self.log_level.unwrap_or_default()
    }

    pub fn default_config_str() -> &'static str {
        r#"# Minecraft Server Load Balancer Configuration
# --------------------------------------------
# Select one of the modes below: 'static', 'geo', or 'http'

mode: static           # Options: static, geo, http

# 1. Static Mode - Predefined list of servers with load balancing algorithm
static:
  algorithm: round_robin   # Options: round_robin, lowest_player_count
  servers:
    - name: "US-East"
      address: "useast.example.com"
      port: 25565
    - name: "EU-West"
      address: "euwest.example.com"
      port: 25565
    - name: "Asia"
      address: "asia.example.com"
      port: 25565

# 2. Geo Mode - Select server based on user's region (using a geo-location API)
geo:
  token: "YOUR-TOKEN"   # Your geolocation API endpoint
  regions:
    NA:
      address: "us.example.com"
      port: 25565
    EU:
      address: "eu.example.com"
      port: 25565
    ASIA:
      address: "asia.example.com"
      port: 25565
  fallback:
    address: "fallback.example.com"
    port: 25565

# 3. HTTP Mode - Server address is fetched from a remote HTTP endpoint
http:
  endpoint: "https://serverselector.example.com/getserver"
  request_method: GET      # Typically GET or POST
  headers:
    Authorization: "Bearer YOUR_API_TOKEN"
  fallback:
    address: "fallback.example.com"
    port: 25565

# Advanced options (optional)
timeout_seconds: 5         # Maximum time to wait for server selection
log_level: info            # Options: info, debug, warn, error
"#
    }
}

/* ---------------- Minimal Tests (can remove) ---------------- */

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_static_ok() {
        let yaml = r#"
        mode: static
static:
  algorithm: round_robin
  servers:
    - name: "A"
      address: "a.example.com"
    - address: "b.example.com"
timeout_seconds: 10
log_level: debug
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.mode, Mode::Static);
        assert_eq!(cfg.static_cfg.as_ref().unwrap().servers.len(), 2);
        assert_eq!(cfg.timeout(), 10);
    }

    #[test]
    fn invalid_missing_section() {
        let yaml = r#"
mode: http
timeout_seconds: 3
"#;
        let err = Config::from_yaml_str(yaml).unwrap_err();
        assert!(matches!(err, ConfigError::Invalid(_)));
    }

    #[test]
    fn http_ok() {
        let yaml = r#"
mode: http
http:
  endpoint: "https://example.com/api"
  request_method: GET
  fallback:
    address: "fallback.example.com"
    port: 25565
"#;
        let cfg = Config::from_yaml_str(yaml).unwrap();
        assert_eq!(cfg.mode, Mode::Http);
        assert!(cfg.http_cfg.is_some());
    }
}
