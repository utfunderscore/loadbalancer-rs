use crate::finder::ServerFinder;
use crate::status::StatusCache;
use log::debug;

use crate::protocol::connection_state::ConnectionState;
use crate::protocol::packets::c2s_handshake::C2SHandshake;
use crate::protocol::packets::c2s_legacy_ping::C2SLegacyPing;
use crate::protocol::packets::c2s_ping_request::PingRequest;
use crate::protocol::packets::c2s_status_request::C2SStatusRequest;
use crate::protocol::packets::s2c_ping_response::PongResponse;
use crate::protocol::packets::{ReadablePacket, WritablePacket};
use crate::protocol::varint::write_var_int;
use crate::protocol::{PacketData, PacketReader, packets};
use anyhow::anyhow;
use std::cmp::max;
use std::net::SocketAddr;
use std::{error::Error, sync::Arc, sync::atomic::AtomicUsize, sync::atomic::Ordering::SeqCst};
use tokio::io::AsyncWriteExt;
use tokio::{
    io::{BufReader, BufWriter},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::Mutex,
};

pub struct Connection {
    state: ConnectionState,
    network_writer: BufWriter<OwnedWriteHalf>,
    network_reader: PacketReader,
    server_finder: Arc<Mutex<Box<dyn ServerFinder>>>,
    status_cache: Arc<Mutex<StatusCache>>,
    motd: String,
    pub addr: SocketAddr,
    context_id: usize,
    protocol_version: i32,
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

impl Connection {
    pub fn new(
        owned_read_half: OwnedReadHalf,
        owned_write_half: OwnedWriteHalf,
        server_finder: Arc<Mutex<Box<dyn ServerFinder>>>,
        status_cache: Arc<Mutex<StatusCache>>,
        addr: SocketAddr,
        motd: String,
    ) -> Connection {
        Connection {
            state: ConnectionState::HandShake,
            server_finder,
            context_id: COUNTER.fetch_add(1, SeqCst),
            network_writer: BufWriter::new(owned_write_half),
            network_reader: PacketReader::new(BufReader::new(owned_read_half)),
            protocol_version: 0,
            status_cache,
            addr,
            motd,
        }
    }

    pub async fn process_packets(&mut self) -> bool {
        let packet = self.network_reader.get_next_packet().await;

        let Ok(mut packet) = packet else {
            println!("Failed to read next packet.");
            return false;
        };

        if let Err(error) = self.handle_packet(&mut packet).await {
            log::error!("{}", error);
            return false;
        };
        true
    }

    async fn handle_packet(&mut self, packet: &mut PacketData) -> Result<(), Box<dyn Error>> {
        match &self.state {
            ConnectionState::HandShake => {
                self.handle_handshake_packet(packet).await?;
            }
            ConnectionState::Status => {
                // debug!("({}) Handling status packet", self.context_id);
                self.handle_status_packet(packet).await?;
            }
            // Config => {
            //     self.handle_config_packet().await?;
            //     return Err("Disconnect".into());
            // }
            // Login => {
            //     self.handle_login_packet(packet).await?;
            // }
            _ => {}
        }
        Ok(())
    }

    async fn handle_handshake_packet(
        &mut self,
        packet: &mut PacketData,
    ) -> Result<(), Box<dyn Error>> {
        if packet.id == C2SHandshake::ID {
            let handshake = C2SHandshake::read(&mut packet.data).await?;

            println!("{:?}", &handshake);
            self.state = handshake.intent;

            println!("Switching to {:?} state", self.state);

            return Ok(());
        } else if packet.id == C2SLegacyPing::ID {
            println!("Received legacy ping");
            return Ok(());
        }

        Err("Incompatible handshake packet received".into())
    }

    async fn handle_status_packet(&mut self, packet: &mut PacketData) -> anyhow::Result<()> {
        match packet.id {
            C2SStatusRequest::ID => {
                let protocol = max(766, self.protocol_version) as u32;

                println!("Received status packet with protocol {}", protocol);

                let status = self
                    .status_cache
                    .lock()
                    .await
                    .get_status_response(
                        self.motd.clone(),
                        protocol,
                        self.server_finder.lock().await,
                    )
                    .await?;

                println!("{:?}", status);
                self.send_packet(status).await?;
                println!("Sent status response packet");

                return Ok(());
            }
            PingRequest::ID => {
                let payload = PingRequest::read(&mut packet.data).await?;
                return self.send_packet(PongResponse::new(payload.timestamp)).await;
            }
            _ => {
                return Err(anyhow!("Incompatible status packet received"));
            }
        }
        Ok(())
    }
    //
    // async fn handle_login_packet(&mut self, packet: &mut RawPacket) -> Result<(), Box<dyn Error>> {
    //     let bytebuf = &packet.payload[..];
    //     match packet.id {
    //         SLoginStart::PACKET_ID => {
    //             debug!("Received login start packet");
    //             let login = SLoginStart::read(bytebuf)?;
    //             self.send_packet(&CLoginSuccess::new(&login.uuid, &login.name, &[]))
    //                 .await?;
    //             Ok(())
    //         }
    //         SLoginAcknowledged::PACKET_ID => {
    //             debug!("Received login acknowledged packet");
    //             self.state = Config;
    //             Ok(())
    //         }
    //         _ => Err("Unknown packet id".into()),
    //     }
    // }
    //
    // async fn handle_config_packet(&mut self) -> Result<(), Box<dyn Error>> {
    //     let mut finder = self
    //         .server_finder
    //         .lock()
    //         .await;
    //
    //     let server =finder.find_server(self).await?;
    //     drop(finder);
    //
    //     let (hostname, port) = server.get_host_and_port().await?;
    //
    //     info!("Transferring to {}:{}", hostname, port);
    //
    //     self.send_packet(&CTransfer::new(&hostname, &VarInt(port as i32)))
    //         .await
    // }
    //
    async fn send_packet<PACKET>(&mut self, packet: PACKET) -> anyhow::Result<()>
    where
        PACKET: WritablePacket,
    {
        Self::write_packet(packet, &mut self.network_writer).await?;
        self.network_writer.flush().await?;
        Ok(())
    }
    //
    pub async fn write_packet<PACKET: WritablePacket, W: AsyncWriteExt + Unpin + Send>(
        packet: PACKET,
        mut write: &mut W,
    ) -> anyhow::Result<()> {
        let mut data: Vec<u8> = Vec::new();
        write_var_int(&mut data, PACKET::ID).await?;
        packet.write(&mut data).await?;

        write_var_int(&mut write, data.len() as i32).await?;
        write.write_all(&data).await?;
        Ok(())
    }
    //
    // async fn get_packet(&mut self) -> Option<RawPacket> {
    //     self.network_reader.get_raw_packet().await.ok()
    // }
}
