use std::io;
use std::io::Error;
use std::io::ErrorKind::InvalidData;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

pub trait ProtocolType: Sized {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error>;

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(
        self,
        writer: &mut W,
    ) -> Result<(), Error>;
}

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

impl ProtocolType for i32 {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let mut value = 0;
        let mut position = 0;

        loop {
            let current_byte = data.read_u8().await?;
            value |= ((current_byte & SEGMENT_BITS) as i32) << position;

            if current_byte & CONTINUE_BIT == 0 {
                break;
            }

            position += 7;
            if position >= 32 {
                return Err(io::Error::new(InvalidData, "VarInt is too big"));
            }
        }
        Ok(value)
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let mut val = self as u32;

        for _ in 0..5 {
            let byte = (val & 0x7F) as u8;
            val >>= 7; // Now this is a logical shift on u32

            writer
                .write_u8(if val == 0 { byte } else { byte | 0x80 })
                .await?;

            if val == 0 {
                return Ok(());
            }
        }

        Err(Error::new(
            io::ErrorKind::InvalidData,
            "VarInt encoding exceeded maximum size",
        ))
    }
}

impl ProtocolType for i64 {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let mut value: i64 = 0;
        let mut position: i32 = 0;

        loop {
            let current_byte = data.read_u8().await?;
            value |= ((current_byte & SEGMENT_BITS) as i64) << position;

            if current_byte & CONTINUE_BIT == 0 {
                break;
            }
            position += 7;
            if position > 64 {
                return Err(io::Error::new(InvalidData, "VarLong is too big"));
            }
        }

        Ok(value)
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let mut val = self as u64;

        for _ in 0..5 {
            let byte = (val & 0x7F) as u8;
            val >>= 7; // Now this is a logical shift on u32

            writer
                .write_u8(if val == 0 { byte } else { byte | 0x80 })
                .await?;

            if val == 0 {
                return Ok(());
            }
        }

        Err(io::Error::new(
            io::ErrorKind::InvalidData,
            "VarInt encoding exceeded maximum size",
        ))
    }
}

impl ProtocolType for bool {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        Ok(data.read_u8().await? == 0x01)
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        writer.write_u8(self.into()).await
    }
}

impl ProtocolType for String {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let length = i32::mc_read(data).await?;
        let mut packet_data = vec![0; length as usize];
        data.read_exact(&mut packet_data).await?;

        Ok(String::from_utf8(packet_data).unwrap())
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let string_bytes = self.as_bytes();

        let length = string_bytes.len() as i32;
        length.mc_write(writer).await?;
        writer.write_all(string_bytes).await?;
        Ok(())
    }
}

impl ProtocolType for Uuid {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let high = data.read_u64().await?;
        let low = data.read_u64().await?;

        Ok(Uuid::from_u64_pair(high, low))
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let (high, low) = self.as_u64_pair();

        writer.write_u64(high).await?;
        writer.write_u64(low).await?;

        Ok(())
    }
}

impl<T> ProtocolType for Vec<T>
where
    T: ProtocolType,
{
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Vec<T>, Error> {
        let length = i32::mc_read(data).await?;

        let mut items: Vec<T> = Vec::with_capacity(length as usize);
        for _ in 0..length {
            let item = T::mc_read(data).await?;
            items.push(item);
        }

        Ok(items)
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        let length = self.len() as i32;

        length.mc_write(writer).await?;
        for item in self {
            item.mc_write(writer).await?;
        }

        Ok(())
    }
}


impl<T> ProtocolType for Option<T> where T: ProtocolType {
    async fn mc_read<W: AsyncReadExt + Unpin>(data: &mut W) -> Result<Self, Error> {
        let is_present = bool::mc_read(data).await?;
        if is_present {
            T::mc_read(data).await.map(Some)
        } else {
            Ok(None)
        }
    }

    async fn mc_write<W: AsyncWriteExt + Unpin + Send>(self, writer: &mut W) -> Result<(), Error> {
        if let Some(item) = self {
            bool::mc_write(true, writer).await?;
            item.mc_write(writer).await?;
        } else {
            bool::mc_write(false, writer).await?;
        }
        Ok(())
    }
}