use crate::protocol::protocol_types::ProtocolType;
use std::io::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

#[derive(Debug)]
pub struct GameProfile {
    pub(crate) player_id: Uuid,
    pub(crate) name: String,
    pub(crate) properties: Vec<Property>,
}

#[derive(Debug)]
pub struct Property {
    name: String,
    value: String,
    signature: Option<String>,
}

impl ProtocolType for Property {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let name = String::mc_read(data).await?;
        let value = String::mc_read(data).await?;
        let signature = Option::<String>::mc_read(data).await?;

        Ok(Property {
            name,
            value,
            signature,
        })
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        self.name.mc_write(writer).await?;
        self.value.mc_write(writer).await?;
        self.signature.mc_write(writer).await?;
        Ok(())
    }
}

impl ProtocolType for GameProfile {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let player_id = Uuid::mc_read(data).await?;
        let name = String::mc_read(data).await?;
        let properties = Vec::<Property>::mc_read(data).await?;
        Ok(GameProfile {
            player_id,
            name,
            properties,
        })
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        self.player_id.mc_write(writer).await?;
        self.name.mc_write(writer).await?;
        self.properties.mc_write(writer).await?;
        Ok(())
    }
}
