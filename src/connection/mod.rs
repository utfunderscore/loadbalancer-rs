use crate::finder::backend::MinecraftServer;
use ConnectionState::{Config, Status};
use log::{debug, info};
use pumpkin_protocol::ConnectionState::{HandShake, Login};
use pumpkin_protocol::codec::var_int::VarInt;
use pumpkin_protocol::java::client::config::CTransfer;
use pumpkin_protocol::java::client::login::CLoginSuccess;
use pumpkin_protocol::java::client::status::{CPingResponse, CStatusResponse};
use pumpkin_protocol::java::packet_decoder::TCPNetworkDecoder;
use pumpkin_protocol::java::packet_encoder::TCPNetworkEncoder;
use pumpkin_protocol::java::server::handshake::SHandShake;
use pumpkin_protocol::java::server::login::{SLoginAcknowledged, SLoginStart};
use pumpkin_protocol::java::server::status::{SStatusPingRequest, SStatusRequest};
use pumpkin_protocol::packet::Packet;
use pumpkin_protocol::ser::{NetworkWriteExt, WritingError};
use pumpkin_protocol::{
    ClientPacket, ConnectionState, Players, RawPacket, ServerPacket, StatusResponse, Version,
};
use std::cmp::max;
use std::error::Error;
use std::io::Write;
use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering::SeqCst;
use tokio::io::{BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};

pub struct Connection {
    state: ConnectionState,
    network_writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    network_reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
    server: MinecraftServer,
    context_id: usize,
    protocol_version: i32,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Connection {
    pub fn new(
        owned_read_half: OwnedReadHalf,
        owned_write_half: OwnedWriteHalf,
        server: MinecraftServer,
    ) -> Connection {
        Connection {
            state: HandShake,
            server,
            context_id: COUNTER.fetch_add(1, SeqCst),
            network_writer: TCPNetworkEncoder::new(BufWriter::new(owned_write_half)),
            network_reader: TCPNetworkDecoder::new(BufReader::new(owned_read_half)),
            protocol_version: 0,
        }
    }

    pub async fn process_packets(&mut self) -> bool {
        let packet = self.get_packet().await;

        let Some(mut packet) = packet else {
            // Connection dropped
            return false;
        };

        if let Err(error) = self.handle_packet(&mut packet).await {
            log::error!(
                "({}) Failed to read incoming packet with id {} (State: {:?}): {}",
                self.context_id,
                packet.id,
                self.state,
                error
            );
            return false;
        };
        true
    }

    async fn handle_packet(&mut self, packet: &mut RawPacket) -> Result<(), Box<dyn Error>> {
        match self.state {
            HandShake => {
                self.handle_handshake_packet(packet).await?;
            }
            Status => {
                info!("({}) Handling status packet", self.context_id);
                self.handle_status_packet(packet).await?;
            }
            Config => {
                let _ = self.handle_config_packet().await;
            }
            Login => {
                self.handle_login_packet(packet).await?;
            }
            _ => todo!(),
        }
        Ok(())
    }

    async fn handle_handshake_packet(
        &mut self,
        packet: &mut RawPacket,
    ) -> Result<(), Box<dyn Error>> {
        let bytebuf = &packet.payload[..];
        match packet.id {
            SHandShake::PACKET_ID => {
                let result = SHandShake::read(bytebuf)?;
                debug!(
                    "({}) Switched from {:?} to {:?}",
                    self.context_id, self.state, result.next_state
                );
                self.state = result.next_state;
                self.protocol_version = result.protocol_version.0;
            }
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
                let protocol = max(766, self.protocol_version) as u32;

                let response = StatusResponse {
                    version: Some(Version {
                        name: "1.21.8".to_owned(),
                        protocol,
                    }),
                    players: Some(Players {
                        max: 1000,
                        online: 0,
                        sample: Vec::new(),
                    }),
                    description: "Pumpkin load finder".to_string(),
                    favicon: None,
                    enforce_secure_chat: false,
                };

                let response = serde_json::to_string(&response)?;
                return self.send_packet(&CStatusResponse::new(&response)).await;
            }
            SStatusPingRequest::PACKET_ID => {
                let payload = SStatusPingRequest::read(bytebuf)?.payload;
                return self.send_packet(&CPingResponse::new(payload)).await;
            }
            _ => {
                println!("Received unknown packet with id: {}", packet.id);
                Err("Unknown packet id")?
            }
        }
        Ok(())
    }

    async fn handle_login_packet(&mut self, packet: &mut RawPacket) -> Result<(), Box<dyn Error>> {
        let bytebuf = &packet.payload[..];
        match packet.id {
            SLoginStart::PACKET_ID => {
                info!("Received login start packet");
                let login = SLoginStart::read(bytebuf)?;
                self.send_packet(&CLoginSuccess::new(&login.uuid, &login.name, &[]))
                    .await?;
                Ok(())
            }
            SLoginAcknowledged::PACKET_ID => {
                info!("Received login acknowledged packet");
                self.state = Config;
                Ok(())
            }
            _ => Err("Unknown packet id".into()),
        }
    }

    async fn handle_config_packet(&mut self) -> Result<(), Box<dyn Error>> {
        let hostname = self.server.hostname.clone();
        self.send_packet(&CTransfer::new(&hostname, &VarInt(self.server.port as i32)))
            .await
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

    pub fn write_packet<PACKET: ClientPacket>(
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
