use crate::connection::Connection;
use pumpkin_protocol::{
    codec::var_int::VarInt,
    ConnectionState,
    ClientPacket,
    java::client::status::CStatusResponse,
    java::packet_decoder::TCPNetworkDecoder,
    java::packet_encoder::TCPNetworkEncoder,
    java::server::handshake::SHandShake,
    java::server::status::SStatusRequest,
    packet::Packet
};
use std::error::Error;
use tokio::{
    io::{BufReader, BufWriter},
    net::TcpStream,
    net::tcp::OwnedWriteHalf
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

    pub async fn test_connection(&self) -> bool {
        let stream = TcpStream::connect((self.hostname.clone(), self.port)).await;
        let Ok(stream) = stream else {
            return false;
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
            return false;
        };

        let Ok(()) = Self::send_packet(&mut stream_writer, &SStatusRequest).await else {
            return false;
        };

        let packet = stream_reader.get_raw_packet().await.ok();
        let Some(packet) = packet else {
            return false;
        };

        if packet.id != CStatusResponse::PACKET_ID {
            return false;
        }

        true
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
            backend.test_connection().await
        })
        .await
        .unwrap();

        println!("{}", result);
    }
}
