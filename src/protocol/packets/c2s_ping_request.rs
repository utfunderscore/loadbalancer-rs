use crate::protocol::packets::{Packet, ReadablePacket};
use std::io::Error;
use tokio::io::AsyncReadExt;
use crate::protocol::protocol_types::ProtocolType;

#[derive(Debug)]
pub struct PingRequest {
    pub timestamp: i64,
}

impl PingRequest {
    pub const ID: i32 = 0x01;
}

impl Packet for PingRequest {
    const ID: i32 = Self::ID;
}

impl ReadablePacket for PingRequest {
    async fn read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let timestamp = data.read_i64().await?;
        Ok(Self { timestamp })
    }
}
