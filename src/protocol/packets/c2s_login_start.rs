use std::io::Error;
use tokio::io::AsyncReadExt;
use uuid::Uuid;
use crate::protocol::packets::{Packet, ReadablePacket};
use crate::protocol::protocol_types::ProtocolType;

pub struct LoginStart {
    pub name: String,
    pub player_id: Uuid
}

impl ReadablePacket for LoginStart {
    async fn read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let name = String::mc_read(data).await?;
        let most_sig = data.read_u64().await?;
        let least_sig = data.read_u64().await?;

        let uuid = Uuid::from_u64_pair(most_sig, least_sig);
        Ok(LoginStart {
            name,
            player_id: uuid,
        })
    }
}

impl Packet for LoginStart {
    const ID: i32 = 0;
}