use redb::{Database, ReadableDatabase, TableDefinition};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct IpInfo {
    pub ip: String,
    pub asn: String,
    pub as_name: String,
    pub as_domain: String,
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
    pub fn new(token: String) -> Result<Self, Box<dyn Error>> {
        let db = Database::create(Path::new("cache/geo.redb"))?;
        Ok(GeoCache {
            client: Client::new(),
            token,
            db,
        })
    }

    pub async fn get_geo_data(&self, ip: &str) -> Result<IpInfo, Box<dyn Error>> {
        if let Some(info) = self.get_cached_ip_info(ip)? {
            return Ok(info);
        }

        let url = format!("https://api.ipinfo.io/lite/{}?token={}", ip, self.token);
        let response = self.client.get(&url).send().await?;
        let ip_info: IpInfo = response.json().await?;
        self.cache_ip_info(&ip_info)?;
        Ok(ip_info)
    }

    fn cache_ip_info(&self, info: &IpInfo) -> Result<(), Box<dyn Error>> {
        let json = serde_json::to_string(info)?;
        let tx = self.db.begin_write()?;
        {
            let mut table = tx.open_table(GEO_TABLE)?;
            table.insert(&info.ip, &json)?;
        }
        tx.commit()?;
        Ok(())
    }

    fn get_cached_ip_info(&self, ip: &str) -> Result<Option<IpInfo>, Box<dyn Error>> {
        let tx = self.db.begin_read()?;
        let table = tx.open_table(GEO_TABLE)?;
        if let Some(json) = table.get(String::from(ip))? {
            let info: IpInfo = serde_json::from_str(&json.value())?;
            Ok(Some(info))
        } else {
            Ok(None)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn sample_ipinfo() -> IpInfo {
        IpInfo {
            ip: "1.2.3.4".to_string(),
            asn: "AS1234".to_string(),
            as_name: "Test ASN".to_string(),
            as_domain: "test.com".to_string(),
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
