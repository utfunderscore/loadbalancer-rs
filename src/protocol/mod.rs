use std::io::{Cursor, ErrorKind};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use crate::protocol::packets::WritablePacket;
use crate::protocol::protocol_types::ProtocolType;

pub mod connection_state;
pub mod packets;
pub mod game_profile;
pub mod protocol_types;

#[derive(Debug)]
pub struct PacketData {
    pub id: i32,
    pub data: Cursor<Vec<u8>>,
}
pub struct PacketReader {
    source: BufReader<OwnedReadHalf>,
}

impl PacketReader {
    pub fn new(source: BufReader<OwnedReadHalf>) -> PacketReader {
        Self { source }
    }

    pub async fn get_next_packet(&mut self) -> Result<PacketData, std::io::Error> {
        println!("Reading packet...");

        let length = i32::mc_read(&mut self.source).await?;

        let mut read = vec![0; length as usize];
        let _ = self.source.read(&mut read).await;

        let mut cursor = Cursor::new(&read);
        let id = i32::mc_read(&mut cursor).await?;

        let mut data = Vec::new();
        let _ = cursor.read_to_end(&mut data).await?;

        println!("id: {} length: {}, data: {:?}", id, data.len(), &data);

        Ok(PacketData {
            id,
            data: Cursor::new(data),
        })
    }

    async fn peek_u8(&mut self) -> Result<u8, std::io::Error> {
        let buffer = self.source.fill_buf().await?;

        buffer.first().copied().ok_or(std::io::Error::new(
            ErrorKind::UnexpectedEof,
            "Failed to read 1 byte",
        ))
    }
}

pub struct PacketWriter {
    output: BufWriter<OwnedWriteHalf>,
}

impl PacketWriter {
    pub fn new(output: BufWriter<OwnedWriteHalf>) -> PacketWriter {
        Self { output }
    }

    pub async fn write_packet<W: WritablePacket>(&mut self, packet: W) -> Result<(), std::io::Error> {
        let mut data: Vec<u8> = Vec::new();
        let packet_id = W::ID;

        packet_id.mc_write(&mut data).await?;
        packet.write(&mut data).await?;

        let length = data.len() as i32;

        length.mc_write(&mut self.output).await?;
        self.output.write_all(&data).await?;

        self.output.flush().await?;

        Ok(())
    }

}