pub mod config;
pub mod connection;
pub mod finder;

use config::Config;
use connection::Connection;
use log::info;
use std::error::Error;
use std::fs::write;
use std::path::Path;
use std::sync::{Arc};
use tokio::net::TcpListener;
use tokio::sync::Mutex;
use crate::finder::ServerFinder;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let config_path = "config.yaml";
    if !Path::new(config_path).exists() {
        // Write the default configuration to the file
        write(config_path, Config::default_config_str())?;
    }
    let config = Config::from_yaml_file(Path::new("config.yaml"))?;

    let server_finder: Arc<Mutex<Box<dyn ServerFinder>>> = Arc::new(Mutex::new(finder::get_server_finder(config)?));

    let listener = TcpListener::bind("0.0.0.0:25565").await?;

    loop {
        let (stream, addr) = listener.accept().await?;
        let server_finder = server_finder.clone();

        tokio::spawn(async move {
            let (read, write) = stream.into_split();
            info!("Accepted connection from {}", addr);

            let mut connection = Connection::new(read, write, server_finder);

            loop {
                if !connection.process_packets().await {
                    info!("Connection terminated");
                    break;
                }
            }
        });
    }
}
