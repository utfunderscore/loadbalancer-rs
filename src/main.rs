mod connection;

use std::error::Error;
use log::info;
use tokio::net::TcpListener;
use connection::Connection;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {

    let listener = TcpListener::bind("0.0.0.0:25565").await?;

    simple_logger::init_with_level(log::Level::Info).unwrap();

    loop {
        let (stream, addr) = listener.accept().await?;

        tokio::spawn(async move {

            let (read, write) = stream.into_split();
            println!("Connection from {}", addr);

            let mut connection = Connection::new(read, write);

            loop {
                let result = connection.process_packets().await;
                if result == false {
                    info!("Connection terminated");
                    break;
                }
            }
        });

    }


    Ok(())
}
