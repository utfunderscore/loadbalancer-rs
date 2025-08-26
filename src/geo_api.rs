use redb::{
    Database, ReadableDatabase, TableDefinition,
    TransactionError, TableError, StorageError, CommitError
};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::path::Path;
use log::{debug, error};

// Custom error type for GeoCache operations
#[derive(Debug)]
pub enum GeoCacheError {
    Database(String),
    Network(String),
    Serialization(String),
    ApiResponse(String),
    InvalidIp(String),
    IoError(String),
}

impl fmt::Display for GeoCacheError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Database(msg) => write!(f, "Database error: {}", msg),
            Self::Network(msg) => write!(f, "Network error: {}", msg),
            Self::Serialization(msg) => write!(f, "Serialization error: {}", msg),
            Self::ApiResponse(msg) => write!(f, "API response error: {}", msg),
            Self::InvalidIp(msg) => write!(f, "Invalid IP address: {}", msg),
            Self::IoError(msg) => write!(f, "I/O error: {}", msg),
        }
    }
}

impl Error for GeoCacheError {}

// Implement From for common error types
impl From<redb::Error> for GeoCacheError {
    fn from(err: redb::Error) -> Self {
        GeoCacheError::Database(err.to_string())
    }
}

// Add implementations for specific redb error types
impl From<TransactionError> for GeoCacheError {
    fn from(err: TransactionError) -> Self {
        GeoCacheError::Database(format!("Transaction error: {}", err))
    }
}

impl From<TableError> for GeoCacheError {
    fn from(err: TableError) -> Self {
        GeoCacheError::Database(format!("Table error: {}", err))
    }
}

impl From<StorageError> for GeoCacheError {
    fn from(err: StorageError) -> Self {
        GeoCacheError::Database(format!("Storage error: {}", err))
    }
}

impl From<CommitError> for GeoCacheError {
    fn from(err: CommitError) -> Self {
        GeoCacheError::Database(format!("Commit error: {}", err))
    }
}

impl From<reqwest::Error> for GeoCacheError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            GeoCacheError::Network(format!("Request timed out: {}", err))
        } else if err.is_connect() {
            GeoCacheError::Network(format!("Connection error: {}", err))
        } else {
            GeoCacheError::Network(format!("Request error: {}", err))
        }
    }
}

impl From<serde_json::Error> for GeoCacheError {
    fn from(err: serde_json::Error) -> Self {
        GeoCacheError::Serialization(err.to_string())
    }
}

impl From<std::io::Error> for GeoCacheError {
    fn from(err: std::io::Error) -> Self {
        GeoCacheError::IoError(err.to_string())
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct IpInfo {
    pub ip: String,
    pub country_code: String,
    pub country: String,
    pub continent_code: String,
    pub continent: String,
}

const GEO_TABLE: TableDefinition<String, String> = TableDefinition::new("geo_cache");

pub struct GeoCache {
    client: Client,
    token: String,
    db: Database,
}

impl GeoCache {
    pub fn new(token: String) -> Result<Self, GeoCacheError> {
        std::fs::create_dir_all("cache").map_err(|e| {
            GeoCacheError::IoError(format!("Failed to create cache directory: {}", e))
        })?;

        let db = Database::create(Path::new("cache/geo.redb")).map_err(|e| {
            GeoCacheError::Database(format!("Failed to create geo cache database: {}", e))
        })?;

        Ok(GeoCache {
            client: Client::new(),
            token,
            db,
        })
    }

    pub async fn get_geo_data(&self, ip: &str) -> Result<IpInfo, GeoCacheError> {
        if ip.trim().is_empty() {
            return Err(GeoCacheError::InvalidIp("IP address is empty".to_string()));
        }

        debug!("Finding cached geo location data");

        match self.get_cached_ip_info(ip) {
            Ok(result) => {
                if let Some(info) = result {
                    return Ok(info);
                }
            }
            Err(err) => {
                error!("Error occurred using cache: {}", err)
            }
        }

        if let Ok(Some(info)) = self.get_cached_ip_info(ip) {
            debug!("Found cached ip address");
            return Ok(info);
        }

        debug!("Address is not cached, looking up...");
        let url = format!("https://api.ipinfo.io/lite/{}?token={}", ip, self.token);
        debug!("Calling - {}", url);
        let response = self.client.get(&url).send().await?;

        if !response.status().is_success() {
            debug!("API Error occurred");
            return Err(GeoCacheError::ApiResponse(format!(
                "API returned error status: {} - {}",
                response.status(),
                response.text().await.unwrap_or_default()
            )));
        }

        let ip_info: IpInfo = response.json().await?;
        self.cache_ip_info(&ip_info)?;
        Ok(ip_info)
    }

    fn cache_ip_info(&self, info: &IpInfo) -> Result<(), GeoCacheError> {
        let json = serde_json::to_string(info)?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(GEO_TABLE)?;
            table.insert(&info.ip, &json)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_cached_ip_info(&self, ip: &str) -> Result<Option<IpInfo>, GeoCacheError> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(GEO_TABLE)?;
        if let Some(json) = table.get(String::from(ip))? {
            let info: IpInfo = serde_json::from_str(&json.value())?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }

    fn is_local_ip() {

    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_ipinfo() -> IpInfo {
        IpInfo {
            ip: "1.2.3.4".to_string(),
            country_code: "US".to_string(),
            country: "United States".to_string(),
            continent_code: "NA".to_string(),
            continent: "North America".to_string(),
        }
    }

    #[test]
    fn test_cache_ip_info_and_get_cached_ip_info() {
        let dir = tempdir().unwrap();
        let db_path = dir.path().join("geo_test.redb");
        let db = Database::create(&db_path).unwrap();
        let cache = GeoCache {
            client: Client::new(),
            token: "dummy".to_string(),
            db,
        };

        let info = sample_ipinfo();
        cache.cache_ip_info(&info).unwrap();

        let retrieved = cache.get_cached_ip_info(&info.ip).unwrap();
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().ip, info.ip);
    }

    #[test]
    fn test_ipinfo_serialization() {
        let info = sample_ipinfo();
        let json = serde_json::to_string(&info).unwrap();
        let deserialized: IpInfo = serde_json::from_str(&json).unwrap();
        assert_eq!(info.ip, deserialized.ip);
        assert_eq!(info.country, deserialized.country);
    }
}
