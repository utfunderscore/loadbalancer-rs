#[derive(Debug)]
pub struct StatusResponse {
    json: String,
}

impl StatusResponse {
    pub fn new(server_status: ServerStatus) -> anyhow::Result<StatusResponse> {
        let result = serde_json::to_string(&server_status)?;
        Ok(StatusResponse { json: result })
    }
}

impl StatusResponse {
    pub const ID: i32 = 0x00;
}

impl Packet for StatusResponse {
    const ID: i32 = Self::ID;
}

impl WritablePacket for StatusResponse {
    async fn write<W: AsyncWriteExt + Unpin + Send>(
        self,
        writer: &mut W,
    ) -> Result<(), std::io::Error> {
        write_string(writer, self.json).await
    }
}

use crate::protocol::mc_string::write_string;
use crate::protocol::packets::{Packet, WritablePacket};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ServerStatus {
    pub version: Option<Version>,
    pub players: Option<Players>,
    pub description: Description,
    pub favicon: Option<String>,
    #[serde(rename = "enforcesSecureChat")]
    pub enforces_secure_chat: bool,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Version {
    pub name: String,
    pub protocol: u32,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Players {
    pub max: u32,
    pub online: u32,
    pub sample: Vec<PlayerSample>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PlayerSample {
    pub name: String,
    pub id: String,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Description {
    pub text: String,
}
