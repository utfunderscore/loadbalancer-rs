use std::io::Error;
use tokio::io::AsyncWriteExt;
use crate::protocol::packets::{Packet, WritablePacket};
use crate::protocol::varlong::write_var_long;

pub struct PongResponse {
    timestamp: i64
}

impl PongResponse {
    pub fn new(timestamp: i64) -> Self {
        Self { timestamp }
    }
}

impl Packet for PongResponse { const ID: i32 = 0x01; }

impl WritablePacket for PongResponse {
    async fn write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let timestamp = self.timestamp;
        write_var_long(writer, timestamp).await
    }
}