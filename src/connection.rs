use crate::finder::ServerFinder;
use crate::status::StatusCache;
use ConnectionState::{Config, Status};
use log::{debug, info};
use pumpkin_protocol::{
    ClientPacket, ConnectionState,
    ConnectionState::{HandShake, Login},
    RawPacket, ServerPacket,
    codec::var_int::VarInt,
    java::client::config::CTransfer,
    java::client::login::CLoginSuccess,
    java::client::status::CPingResponse,
    java::packet_decoder::TCPNetworkDecoder,
    java::packet_encoder::TCPNetworkEncoder,
    java::server::handshake::SHandShake,
    java::server::login::{SLoginAcknowledged, SLoginStart},
    java::server::status::{SStatusPingRequest, SStatusRequest},
    packet::Packet,
    ser::{NetworkWriteExt, WritingError},
};
use std::{
    cmp::max, error::Error, io::Write, sync::Arc, sync::atomic::AtomicUsize,
    sync::atomic::Ordering::SeqCst,
};
use tokio::{
    io::{BufReader, BufWriter},
    net::tcp::{OwnedReadHalf, OwnedWriteHalf},
    sync::Mutex,
};

pub struct Connection {
    state: ConnectionState,
    network_writer: TCPNetworkEncoder<BufWriter<OwnedWriteHalf>>,
    network_reader: TCPNetworkDecoder<BufReader<OwnedReadHalf>>,
    server_finder: Arc<Mutex<Box<dyn ServerFinder>>>,
    status_cache: Arc<Mutex<StatusCache>>,
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
    ) -> Connection {
        Connection {
            state: HandShake,
            server_finder,
            context_id: COUNTER.fetch_add(1, SeqCst),
            network_writer: TCPNetworkEncoder::new(BufWriter::new(owned_write_half)),
            network_reader: TCPNetworkDecoder::new(BufReader::new(owned_read_half)),
            protocol_version: 0,
            status_cache,
        }
    }

    pub async fn process_packets(&mut self) -> bool {
        let packet = self.get_packet().await;

        let Some(mut packet) = packet else {
            debug!("Failed to read next packet.");
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
                debug!("({}) Handling status packet", self.context_id);
                self.handle_status_packet(packet).await?;
            }
            Config => {
                self.handle_config_packet().await?;
                return Err("Disconnect".into());
            }
            Login => {
                self.handle_login_packet(packet).await?;
            }
            _ => {}
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
        debug!("Handling status packet with id {}", packet.id);

        match packet.id {
            SStatusRequest::PACKET_ID => {
                let protocol = max(766, self.protocol_version) as u32;

                let status = self
                    .status_cache
                    .lock()
                    .await
                    .get_status_response(
                        String::from("test"),
                        protocol,
                        self.server_finder.lock().await,
                    )
                    .await;
                return self.send_packet(&status).await;
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
                debug!("Received login start packet");
                let login = SLoginStart::read(bytebuf)?;
                self.send_packet(&CLoginSuccess::new(&login.uuid, &login.name, &[]))
                    .await?;
                Ok(())
            }
            SLoginAcknowledged::PACKET_ID => {
                debug!("Received login acknowledged packet");
                self.state = Config;
                Ok(())
            }
            _ => Err("Unknown packet id".into()),
        }
    }

    async fn handle_config_packet(&mut self) -> Result<(), Box<dyn Error>> {
        let mut finder = self
            .server_finder
            .lock()
            .await;

        let server =finder.find_server()?;
        drop(finder);

        let (hostname, port) = server.get_host_and_port().await?;

        info!("Transferring to {}:{}", hostname, port);

        self.send_packet(&CTransfer::new(&hostname, &VarInt(port as i32)))
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
