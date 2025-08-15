use crate::connection::Connection;
use log::info;
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
    pub hostname: String,
    pub port: u16,
}

impl MinecraftServer {
    pub fn new(hostname: String, port: u16) -> MinecraftServer {
        MinecraftServer { hostname, port }
    }

    pub async fn get_player_count(&self) -> Option<u64> {
        let stream = TcpStream::connect((self.hostname.clone(), self.port))
            .await
            .ok()?;

        let (reader, writer) = stream.into_split();

        let mut stream_writer = TCPNetworkEncoder::new(BufWriter::new(writer));
        let mut stream_reader = TCPNetworkDecoder::new(BufReader::new(reader));

        let handshake_packet = SHandShake {
            protocol_version: VarInt(772),
            server_address: self.hostname.to_string(),
            server_port: self.port,
            next_state: ConnectionState::Status,
        };

        info!("Sending handshake packet");
        Self::send_packet(&mut stream_writer, &handshake_packet)
            .await
            .ok()?;

        info!("Sending status packet");
        Self::send_packet(&mut stream_writer, &SStatusRequest)
            .await
            .ok()?;

        info!("Waiting for response");

        let packet: RawPacket = stream_reader.get_raw_packet().await.ok()?;

        let bytebuf = &packet.payload[..];
        let packet = CStatusResponse::read(bytebuf).ok()?;

        info!("Yup!");

        let response = serde_json::from_str::<'_, Value>(&packet.json_response).ok()?;

        let players = response.get("players")?;

        players.get("online")?.as_u64()
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
    use std::time::Duration;
    use tokio::time::timeout;

    #[tokio::test]
    async fn test_backend_new() {
        simple_logger::init_with_level(log::Level::Debug).unwrap();

        let backend = MinecraftServer::new(String::from("localhost"), 25565);

        let result = timeout(Duration::from_secs(5), async {
            backend.get_player_count().await
        })
        .await
        .unwrap();

        println!("{:?}", result);
    }
}
