use crate::protocol::varint::{read_var_int, write_var_int};
use std::io::Error;
use futures::AsyncWrite;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub async fn read_string<W: AsyncReadExt + Unpin>(stream: &mut W) -> Result<String, Error> {
    let length = read_var_int(stream).await?;
    let mut data = vec![0; length as usize];
    stream.read_exact(&mut data).await?;

    Ok(String::from_utf8(data).unwrap())
}

pub async fn write_string<W: AsyncWriteExt + Unpin>(output: &mut W, string: String) -> Result<(), Error> {
    let string_bytes = string.as_bytes();

    let length = string_bytes.len() as i32;
    write_var_int(output, length).await?;
    output.write_all(string_bytes).await?;
    Ok(())
}