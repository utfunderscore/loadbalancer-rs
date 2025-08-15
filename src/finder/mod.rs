use crate::config::Algorithm::RoundRobin;
use crate::config::{Algorithm, Config, Mode, StaticConfig};
use crate::finder::backend::MinecraftServer;
use async_trait::async_trait;
use std::error::Error;
use std::future;
use std::time::Duration;
use futures::future::join_all;
use tokio::time::error::Elapsed;
use tokio::time::timeout;

pub mod backend;

#[async_trait]
pub trait ServerFinder: Send + Sync {
    async fn get_player_count(&self) -> u64;

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
        StaticServerFiner {
            servers: config
                .servers
                .iter()
                .map(|x| MinecraftServer::new(x.address.clone(), x.port))
                .collect(),
            mode: config.algorithm,
            last_index: 0,
        }
    }
}

#[async_trait]
impl ServerFinder for StaticServerFiner {
    async fn get_player_count(&self) -> u64 {
        let futures: Vec<_> = self.servers.iter().map(|x| async move {
            timeout(Duration::from_secs(5), x.get_player_count()).await.ok().flatten().unwrap_or(0)
        }).collect();

        join_all(futures).await.iter().sum()
    }

    fn find_server(&mut self) -> Result<MinecraftServer, Box<dyn Error>> {
        match self.mode {
            RoundRobin => {
                let index = self.last_index + 1;
                if index >= self.servers.len() {
                    self.last_index = 0;
                } else {
                    self.last_index = index;
                }
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
