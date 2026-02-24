use std::io::{Cursor, ErrorKind};
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, BufWriter};
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use varint::read_var_int;

pub mod connection_state;
pub mod mc_string;
pub mod packets;
pub mod varint;
pub mod varlong;

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

        let length = read_var_int(&mut self.source).await?;

        let mut read = vec![0; length as usize];
        let _ = self.source.read(&mut read).await;

        let mut cursor = Cursor::new(&read);

        let id = read_var_int(&mut cursor).await?;
        println!("id {}", id);

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
}