use ConnectionState::{Config, Status};
use log::info;
use pumpkin_protocol::ConnectionState::HandShake;
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::packet_decoder::TCPNetworkDecoder;
use pumpkin_protocol::java::packet_encoder::TCPNetworkEncoder;
use pumpkin_protocol::java::server::handshake::SHandShake;
use pumpkin_protocol::java::server::status::SStatusRequest;
use pumpkin_protocol::packet::Packet;
use pumpkin_protocol::ser::{NetworkWriteExt, WritingError};
use pumpkin_protocol::{ClientPacket, ConnectionState, RawPacket, ServerPacket};
use std::error::Error;
use std::io::Write;
use pumpkin_protocol::java::client::status::CStatusResponse;
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub struct Connection {
    state: ConnectionState,
    network_writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    network_reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
}

impl Connection {
    pub fn new(owned_read_half: OwnedReadHalf, owned_write_half: OwnedWriteHalf) -> Connection {
        Connection {
            state: HandShake,
            network_writer: TCPNetworkEncoder::new(BufWriter::new(owned_write_half)),
            network_reader: TCPNetworkDecoder::new(BufReader::new(owned_read_half)),
        }
    }

    pub async fn process_packets(&mut self) -> bool {
        let packet = self.get_packet().await;

        let Some(mut packet) = packet else {
            info!("Failed to get packet");
            return false;
        };

        if let Err(error) = self.handle_packet(&mut packet).await {
            log::error!(
                "Failed to read incoming packet with id {}: {}",
                packet.id,
                error
            );
            return false;
        };
        true
    }

    async fn handle_packet(&mut self, packet: &mut RawPacket) -> Result<(), Box<dyn Error>> {
        match self.state {
            Handshake => {
                let result = self.handle_handshake_packet(packet).await;
            }
            Status => {}
            Config => {}
        }
        Ok(())
    }

    async fn handle_handshake_packet(
        &mut self,
        packet: &mut RawPacket,
    ) -> Result<(), Box<dyn Error>> {
        let bytebuf = &packet.payload[..];
        match packet.id {
            SHandShake::PACKET_ID => {}
            _ => {
                println!("Received unknown packet with id: {}", packet.id);
            }
        }
        Ok(())
    }

    async fn handle_status_packet(&mut self, packet: &mut RawPacket) -> Result<(), Box<dyn Error>> {
        let bytebuf = &packet.payload[..];
        match packet.id {
            SStatusRequest::PACKET_ID => {
                return self.send_packet(&CStatusResponse::new("")).await;
            }
            _ => {
                println!("Received unknown packet with id: {}", packet.id);
                Err("Unknown packet id")?
            }
        }
        Ok(())
    }

    async fn send_packet<PACKET>(&mut self, packet: &PACKET) -> Result<(), Box<dyn Error>>
    where
        PACKET: ClientPacket,
    {
        let mut buffer = Vec::new();
        Self::write_packet(packet, &mut buffer)?;

        self.network_writer.write_packet(buffer.into()).await?;
        Ok(())
    }

    fn write_packet<PACKET: ClientPacket>(
        packet: &PACKET,
        mut write: impl Write,
    ) -> Result<(), WritingError> {
        write.write_var_int(&VarInt(PACKET::PACKET_ID))?;
        packet.write_packet_data(write)
    }

    async fn get_packet(&mut self) -> Option<RawPacket> {
        self.network_reader.get_raw_packet().await.ok()
    }
}
