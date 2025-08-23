use crate::backend::MinecraftServer;
use crate::config::Algorithm::RoundRobin;
use crate::config::{Algorithm, Config, Mode, StaticConfig};
use async_trait::async_trait;
use futures::future::join_all;
use log::info;
use std::error::Error;
use std::time::Duration;
use tokio::time::timeout;

#[async_trait]
pub trait ServerFinder: Send + Sync {
    async fn get_player_count(&self) -> u32;

    fn find_server(&mut self) -> Result<MinecraftServer, Box<dyn Error>>;
}

pub fn get_server_finder(config: Config) -> Result<Box<dyn ServerFinder>, Box<dyn Error>> {
    match config.mode {
        Mode::Static => match config.static_cfg {
            None => Err("Invalid static server find config.".into()),
            Some(config) => Ok(Box::new(StaticServerFiner::new(config))),
        },
        Mode::Geo => Err("TODO".into()),
        Mode::Http => Err("TODO".into()),
    }
}

struct StaticServerFiner {
    servers: Vec<MinecraftServer>,
    mode: Algorithm,
    last_index: usize,
}

impl StaticServerFiner {
    pub fn new(config: StaticConfig) -> Self {

        let mut servers = Vec::new();

        for server in config.servers {
            let parsed = MinecraftServer::parse(server.address.clone());
            if let Err(e) = parsed {
                info!("Error parsing server address {}: {}", server.address, e);
            } else {
                servers.push(parsed.unwrap());
            }
        }

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
        info!("Getting player count from {} servers", self.servers.len());

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

        join_all(futures).await.iter().sum()
    }

    fn find_server(&mut self) -> Result<MinecraftServer, Box<dyn Error>> {
        match self.mode {
            RoundRobin => {
                println!("before: {}", self.last_index);
                let index = self.last_index + 1;
                if index >= self.servers.len() {
                    self.last_index = 0;
                } else {
                    self.last_index = index;
                }
                println!("after: {}", self.last_index);
                let server = self
                    .servers
                    .get(index)
                    .ok_or("Couldn't find server")?
                    .clone();

                Ok(server)
            }
            Algorithm::LowestPlayerCount => {
                todo!("No player count tracking yet")
            }
        }
    }
}
