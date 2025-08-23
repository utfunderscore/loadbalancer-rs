use crate::connection::Connection;
use log::info;
use pumpkin_protocol::{
    codec::var_int::VarInt, java::client::status::CStatusResponse, java::packet_decoder::TCPNetworkDecoder, java::packet_encoder::TCPNetworkEncoder, java::server::handshake::SHandShake,
    java::server::status::SStatusRequest, ClientPacket,
    ConnectionState, RawPacket,
    ServerPacket,
};
use serde_json::Value;
use std::error::Error;
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::OwnedWriteHalf;
use tokio::net::TcpStream;

#[derive(Debug, Clone)]
pub struct MinecraftServer {
    pub address: String,
}

impl MinecraftServer {
    pub fn new(address: String) -> MinecraftServer {
        MinecraftServer { address }
    }

    pub async fn get_player_count(&self) -> Result<u32, Box<dyn Error>> {
        println!("Getting player count from {}", self.address);

        let stream = TcpStream::connect("")
            .await?;

        println!("Connected to server");

        let (reader, writer) = stream.into_split();

        let mut stream_writer = TCPNetworkEncoder::new(BufWriter::new(writer));
        let mut stream_reader = TCPNetworkDecoder::new(BufReader::new(reader));

        let (hostname, port) = self.get_hostname_and_port();

        let handshake_packet = SHandShake {
            protocol_version: VarInt(772),
            server_address: hostname,
            server_port: port,
            next_state: ConnectionState::Status,
        };

        info!("Sending handshake packet");
        Self::send_packet(&mut stream_writer, &handshake_packet)
            .await?;

        info!("Sending status packet");
        Self::send_packet(&mut stream_writer, &SStatusRequest)
            .await?;

        info!("Waiting for response");

        let packet: RawPacket = stream_reader.get_raw_packet().await?;

        let bytebuf = &packet.payload[..];
        let packet = CStatusResponse::read(bytebuf)?;

        info!("Yup!");

        let response = serde_json::from_str::<'_, Value>(&packet.json_response)?;

        let players = response.get("players").ok_or("Response did not contain 'players' field")?;

        let online_field = players.get("online").ok_or("Response did not contain 'online' field")?;

        let online = online_field.as_u64().ok_or("'online' field is not a u64")? as u32;
        Ok(online)
    }

    async fn send_packet<PACKET>(
        stream_writer: &mut TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
        packet: &PACKET,
    ) -> Result<(), Box<dyn Error>>
    where
        PACKET: ClientPacket,
    {
        let mut buffer = Vec::new();
        Connection::write_packet(packet, &mut buffer)?;

        stream_writer.write_packet(buffer.into()).await?;
        Ok(())
    }

    pub fn get_hostname_and_port(&self) -> (String, u16) {
        let parts: Vec<&str> = self.address.split(':').collect();
        let hostname = parts[0].to_string();
        let port = if parts.len() > 1 {
            parts[1].parse::<u16>().unwrap_or(25565)
        } else {
            25565
        };
        (hostname, port)
    }

}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_new() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
        println!("Logger initialized");
        //
        let backend = MinecraftServer::new(String::from("hypixel.net"));
        let result = backend.get_player_count().await;

        println!("{:?}", result);

        assert_eq!(result.is_ok(), true);
        println!("Player count: {:?}", result);
    }
}
