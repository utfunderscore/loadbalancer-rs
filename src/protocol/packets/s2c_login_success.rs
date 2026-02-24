use crate::protocol::game_profile::GameProfile;
use crate::protocol::packets::{Packet, WritablePacket};
use crate::protocol::protocol_types::ProtocolType;
use std::io::Error;
use tokio::io::AsyncWriteExt;
use uuid::Uuid;

pub struct LoginSuccess {
    game_profile: GameProfile,
}

impl LoginSuccess {
    pub fn by_name(username: String, player_id: Uuid) -> Self{
        Self {
           game_profile: GameProfile {
               name: username,
               player_id,
               properties: Vec::new(),
           }
        }
    }

    pub fn new(game_profile: GameProfile) -> Self {
        Self { game_profile }
    }
}

impl Packet for LoginSuccess {
    const ID: i32 = 0x02;
}

impl WritablePacket for LoginSuccess {
    async fn write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        self.game_profile.mc_write(writer).await
    }
}
