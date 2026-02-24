use tokio::io::AsyncReadExt;
use crate::protocol::connection_state::ConnectionState;
use crate::protocol::protocol_types::ProtocolType;

#[derive(Debug)]
pub struct C2SHandshake {
    pub protocol_version: i32,
    pub server_address: String,
    pub server_port: u16,
    pub intent: ConnectionState,
}

impl C2SHandshake {
    pub const ID: i32 = 0;

    pub async fn read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<C2SHandshake, std::io::Error> {
        let protocol_version = i32::mc_read(data).await?;
        let server_address = String::mc_read(data).await?;
        let server_port = data.read_u16().await?;
        let intent_id = i32::mc_read(data).await?;
        let intent = ConnectionState::from_intent(intent_id).ok_or(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!("Unknown intent: {}", intent_id),
        ))?;

        Ok(C2SHandshake {
            protocol_version,
            server_address,
            server_port,
            intent,
        })
    }

}
