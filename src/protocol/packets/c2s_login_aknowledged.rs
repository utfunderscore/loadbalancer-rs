use std::io::Error;
use tokio::io::AsyncReadExt;
use crate::protocol::packets::{Packet, ReadablePacket};

pub struct LoginAcknowledged;

impl Packet for LoginAcknowledged { const ID: i32 = 0x03; }

impl ReadablePacket for LoginAcknowledged {
    async fn read<W: AsyncReadExt + Unpin>(_data: &mut W) -> Result<Self, Error> {
        Ok(LoginAcknowledged {})
    }
}