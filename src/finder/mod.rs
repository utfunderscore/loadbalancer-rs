use crate::config::Algorithm::RoundRobin;
use crate::config::{Algorithm, Config, Mode, StaticConfig};
use crate::finder::backend::MinecraftServer;
use std::error::Error;

pub mod backend;

pub trait ServerFinder: Send + Sync {
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

impl ServerFinder for StaticServerFiner {
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
