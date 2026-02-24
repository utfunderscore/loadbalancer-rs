
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod c2s_handshake;
pub mod c2s_legacy_ping;
pub mod c2s_status_request;
pub mod s2c_status_response;
pub mod c2s_ping_request;
pub mod s2c_ping_response;
pub mod c2s_login_start;
pub mod s2c_login_success;
pub mod c2s_login_aknowledged;
pub mod s2c_transfer;

pub trait Packet {
    const ID: i32;
}

pub trait WritablePacket: Packet {
    async fn write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), std::io::Error>;
}

pub trait ReadablePacket: Packet + Sized {
    async fn read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, std::io::Error>;
}