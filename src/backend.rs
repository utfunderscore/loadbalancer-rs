use crate::address_resolver::resolve_host_port;
use crate::connection::Connection;
use log::{debug};
use pumpkin_protocol::{
    ClientPacket, ConnectionState, RawPacket, ServerPacket, codec::var_int::VarInt,
    java::client::status::CStatusResponse, java::packet_decoder::TCPNetworkDecoder,
    java::packet_encoder::TCPNetworkEncoder, java::server::handshake::SHandShake,
    java::server::status::SStatusRequest,
};
use serde_json::Value;
use std::error::Error;
use tokio::io::{BufReader, BufWriter};
use tokio::net::TcpStream;
use tokio::net::tcp::OwnedWriteHalf;

#[derive(Debug, Clone)]
pub struct MinecraftServer {
    pub address: String,
}

impl MinecraftServer {
    pub fn parse(address: String) -> Result<Self, Box<dyn Error>> {
        Ok(MinecraftServer { address })
    }

    pub async fn get_player_count(&self) -> Result<u32, Box<dyn Error>> {
        debug!("Getting player count from {}", self.address);

        let (hostname, port) = self.get_host_and_port().await?;

        debug!("{}:{}", hostname, port);

        let stream = TcpStream::connect((hostname.clone(), port)).await?;

        debug!("Connected to server");

        let (reader, writer) = stream.into_split();

        let mut stream_writer = TCPNetworkEncoder::new(BufWriter::new(writer));
        let mut stream_reader = TCPNetworkDecoder::new(BufReader::new(reader));

        let handshake_packet = SHandShake {
            protocol_version: VarInt(772),
            server_address: hostname.to_string(),
            server_port: port,
            next_state: ConnectionState::Status,
        };

        debug!("Sending handshake packet");
        Self::send_packet(&mut stream_writer, &handshake_packet).await?;

        debug!("Sending status packet");
        Self::send_packet(&mut stream_writer, &SStatusRequest).await?;

        debug!("Waiting for response");

        let packet: RawPacket = stream_reader.get_raw_packet().await?;

        let bytebuf = &packet.payload[..];
        let packet = CStatusResponse::read(bytebuf)?;
        
        let response = serde_json::from_str::<'_, Value>(&packet.json_response)?;

        let players = response
            .get("players")
            .ok_or("Response did not contain 'players' field")?;

        let online_field = players
            .get("online")
            .ok_or("Response did not contain 'online' field")?;

        let online = online_field.as_u64().ok_or("'online' field is not a u64")? as u32;
        Ok(online)
    }

    pub async fn get_host_and_port(&self) -> Result<(String, u16), Box<dyn Error>> {
        let result = resolve_host_port(&self.address, "minecraft", "tcp", 25565).await?;

        Ok((result.ip.to_string(), result.port))
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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_backend_new() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
        //
        let backend = MinecraftServer::parse(String::from("hypixel.net")).unwrap();
        let result = backend.get_player_count().await;

        println!("{:?}", result);

        assert_eq!(result.is_ok(), true);
        println!("Player count: {:?}", result);
    }

    #[tokio::test]
    async fn test_get_host_port() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();
        println!("Logger initialized");
        //
        let backend = MinecraftServer::parse(String::from("hypixel.net")).unwrap();
        let (host, port) = backend.get_host_and_port().await.unwrap();

        println!("{} {}", host, port)
    }


}
