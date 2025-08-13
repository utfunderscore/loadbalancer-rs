use crate::connection::Connection;
use pumpkin_protocol::{
    ClientPacket, ConnectionState, ServerPacket, StatusResponse, codec::var_int::VarInt,
    java::client::status::CStatusResponse, java::packet_decoder::TCPNetworkDecoder,
    java::packet_encoder::TCPNetworkEncoder, java::server::handshake::SHandShake,
    java::server::status::SStatusRequest, packet::Packet,
};
use std::error::Error;
use tokio::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    net::tcp::OwnedWriteHalf,
};

#[derive(Debug, Clone)]
pub struct MinecraftServer {
    pub hostname: String,
    pub port: u16,
}

impl MinecraftServer {
    pub fn new(hostname: String, port: u16) -> MinecraftServer {
        MinecraftServer { hostname, port }
    }

    pub async fn get_playercount(&self) -> Option<u16> {
        let stream = TcpStream::connect((self.hostname.clone(), self.port)).await;
        let Ok(stream) = stream else {
            return None;
        };

        let (reader, writer) = stream.into_split();

        let mut stream_writer = TCPNetworkEncoder::new(BufWriter::new(writer));
        let mut stream_reader = TCPNetworkDecoder::new(BufReader::new(reader));

        let handshake_packet = SHandShake {
            protocol_version: VarInt(772),
            server_address: self.hostname.to_string(),
            server_port: self.port,
            next_state: ConnectionState::Status,
        };

        let Ok(()) = Self::send_packet(&mut stream_writer, &handshake_packet).await else {
            return None;
        };

        let Ok(()) = Self::send_packet(&mut stream_writer, &SStatusRequest).await else {
            return None;
        };

        let packet = stream_reader.get_raw_packet().await.ok();
        let Some(packet) = packet else {
            return None;
        };

        if packet.id != CStatusResponse::PACKET_ID {
            return None;
        }
        let buffer = &packet.payload[..];

        let packet = CStatusResponse::read(buffer);
        let Ok(packet) = packet else {
            return None;
        };

        // let response = serde_json::from_str(packet.json_response);

        // Waiting on PR to be merged
        None
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
        let backend = MinecraftServer::new("localhost".to_string(), 25565);

        let result = timeout(Duration::from_secs(5), async {
            backend.get_playercount().await
        })
        .await
        .unwrap();
    }
}
