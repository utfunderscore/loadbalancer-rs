use crate::backend::MinecraftServer;
use crate::config::{
    GeoConfig, LoadBalanceMode, LoadBalancerConfig, StaticAlgorithm, StaticConfig,
};
use crate::connection::Connection;
use crate::geo_api::GeoCache;
use async_trait::async_trait;
use futures::{StreamExt, future::join_all, stream};
use log::info;
use std::{collections::HashMap, error::Error, time::Duration};
use std::net::IpAddr;
use anyhow::anyhow;
use tokio::time::timeout;

#[async_trait]
pub trait ServerFinder: Send + Sync {
    async fn get_player_count(&self) -> u32;

    async fn find_server(
        &mut self,
        connection: &Connection,
    ) -> anyhow::Result<MinecraftServer>;
}

pub fn get_server_finder(
    config: LoadBalancerConfig,
) -> Result<Box<dyn ServerFinder>, Box<dyn Error>> {
    match config.mode {
        LoadBalanceMode::Static => match config.static_config {
            None => Err("Invalid static server find config.".into()),
            Some(config) => Ok(Box::new(StaticServerFiner::new(config))),
        },
        LoadBalanceMode::Geo => match config.geo_config {
            None => Err("Invalid geo location config".into()),
            Some(config) => {
                let finder = GeoServerFinder::new(config)?;
                Ok(Box::new(finder))
            }
        },
        LoadBalanceMode::Http => Err("TODO".into()),
    }
}

struct StaticServerFiner {
    servers: Vec<MinecraftServer>,
    mode: StaticAlgorithm,
    last_index: usize,
}

impl StaticServerFiner {
    pub fn new(config: StaticConfig) -> Self {
        let servers = config
            .servers
            .iter()
            .map(|x| MinecraftServer::new(x.address.clone()))
            .collect();
        StaticServerFiner {
            servers,
            mode: config.algorithm,
            last_index: 0,
        }
    }
}

#[async_trait]
impl ServerFinder for StaticServerFiner {
    async fn get_player_count(&self) -> u32 {
        let start_time = std::time::Instant::now();

        let futures: Vec<_> = self
            .servers
            .iter()
            .map(|x| async move {
                let result: Result<u32, Box<dyn Error>> =
                    timeout(Duration::from_secs(5), x.get_player_count())
                        .await
                        .map_err(|x| x.into())
                        .flatten();
                if result.is_err() {
                    info!(
                        "Error getting player count from server {}: {}",
                        x.address,
                        result.as_ref().err().unwrap()
                    );
                }
                result.unwrap_or(0)
            })
            .collect();

        let total = join_all(futures).await.iter().sum();
        let elapsed = start_time.elapsed();
        info!("Getting player counts took {:?}", elapsed);
        total
    }

    async fn find_server(&mut self, _: &Connection) -> anyhow::Result<MinecraftServer> {
        match self.mode {
            StaticAlgorithm::RoundRobin => {
                let index = self.last_index + 1;
                if index >= self.servers.len() {
                    self.last_index = 0;
                } else {
                    self.last_index = index;
                }

                let server = self
                    .servers
                    .get(self.last_index)
                    .ok_or(anyhow!("Couldn't find server"))?
                    .clone();

                Ok(server)
            }
            StaticAlgorithm::LowestPlayerCount => {
                let result: Vec<_> = stream::iter(self.servers.clone())
                    .map(|server| async move {
                        (
                            server.clone(),
                            server.get_player_count().await.unwrap_or(u32::MAX),
                        )
                    })
                    .buffer_unordered(5)
                    .collect()
                    .await;

                result
                    .into_iter()
                    .min_by_key(|(_, count)| *count)
                    .map(|x| x.0)
                    .ok_or(anyhow!("No servers available"))
            }
        }
    }
}

struct GeoServerFinder {
    pub regions: HashMap<String, MinecraftServer>,
    pub fallback: MinecraftServer,
    pub geo_cache: GeoCache,
}

impl GeoServerFinder {
    pub fn new(config: GeoConfig) -> Result<Self, Box<dyn Error>> {

        let regions: HashMap<String, MinecraftServer> = config
            .regions
            .into_iter()
            .map(|(key, server)| {
                // transform server to ServerInfo
                (key, MinecraftServer::new(server.address))
            })
            .collect();

        let fallback = MinecraftServer::new(config.fallback.address);
        let geo_cache = GeoCache::new(config.token)?;

        Ok(GeoServerFinder {
            regions,
            fallback,
            geo_cache,
        })
    }


}

#[async_trait]
impl ServerFinder for GeoServerFinder {
    async fn get_player_count(&self) -> u32 {
        let mut all_servers: Vec<MinecraftServer> = self.regions.values().cloned().collect();
        all_servers.push(self.fallback.clone());

        let result: Vec<u32> = stream::iter(all_servers)
            .map(async |x| x.get_player_count().await.unwrap_or(0))
            .buffer_unordered(8)
            .collect()
            .await;

        result.iter().sum()
    }

    async fn find_server(
        &mut self,
        connection: &Connection,
    ) -> anyhow::Result<MinecraftServer> {
        let is_local = match connection.addr.ip() {
            IpAddr::V4(ipv4) => {
                // Check for loopback, private, or link-local ranges
                ipv4.is_loopback() || ipv4.is_private() || ipv4.is_link_local()
            }
            IpAddr::V6(ipv6) => {
                // Check for loopback or unique local addresses
                ipv6.is_loopback() || ipv6.is_unique_local()
            }
        };
        let ip = if is_local {
            "129.58.155.11"
        } else {
            &connection.addr.ip().to_string()
        };

        let ip_info = self
            .geo_cache
            .get_geo_data(ip)
            .await?;
        if let Some(server) = self.regions.get(&ip_info.continent_code) {
            return Ok(server.clone());
        };
        if let Some(server) = self.regions.get(&ip_info.country_code) {
            return Ok(server.clone());
        }

        Ok(self.fallback.clone())
    }

}
