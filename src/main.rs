pub mod address_resolver;
pub mod backend;
pub mod config;
pub mod connection;
pub mod finder;
mod geo_api;
pub mod status;

use crate::config::{
    ConfigError, LoadBalancerConfig, format_concise_error_report, print_error_report,
};
use crate::connection::Connection;
use crate::finder::ServerFinder;
use log::info;
use std::error::Error;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::Mutex;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    let config = LoadBalancerConfig::load("config.yml");
    if let Err(err) = config {
        match err {
            ConfigError::ValidationError(report) => {
                // Print the detailed error report
                print_error_report(&report);

                // You can also use the concise format
                println!("\nConcise error report:");
                println!("{}", format_concise_error_report(&report));
            }
            _ => eprintln!("Error: {}", err),
        }
    } else if let Ok(config) = config {
        let motd = config.motd.clone();
        let server_finder: Arc<Mutex<Box<dyn ServerFinder>>> =
            Arc::new(Mutex::new(finder::get_server_finder(config)?));

        let listener = TcpListener::bind("0.0.0.0:25565").await?;
        let status_cache = Arc::new(Mutex::new(status::StatusCache::new()));

        loop {
            let (stream, addr) = listener.accept().await?;
            let server_finder = server_finder.clone();

            let status_cache = status_cache.clone();
            let motd = motd.clone();

            tokio::spawn(async move {
                let (read, write) = stream.into_split();
                info!("Accepted connection from {}", addr);

                let mut connection =
                    Connection::new(read, write, server_finder, status_cache, addr, motd.clone());

                loop {
                    if !connection.process_packets().await {
                        info!("Connection terminated");
                        break;
                    }
                }
            });
        }
    }
    Ok(())
}
