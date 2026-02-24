use std::io::Error;
use tokio::io::AsyncWriteExt;
use crate::protocol::packets::{Packet, WritablePacket};
use crate::protocol::protocol_types::ProtocolType;

pub struct TransferPacket {
    host: String,
    port: i32,
}

impl TransferPacket {
    pub fn new(host: String, port: i32) -> Self {
        Self { host, port }
    }
}

impl Packet for TransferPacket {
    const ID: i32 = 0x0B;
}

impl WritablePacket for TransferPacket {
    async fn write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        self.host.mc_write(writer).await?;
        self.port.mc_write(writer).await
    }
}