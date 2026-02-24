use std::io::ErrorKind::InvalidData;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

pub async fn read_var_int<W>(buf_reader: &mut W) -> Result<i32, io::Error>
where
    W: AsyncReadExt + Unpin,
{
    let mut value = 0;
    let mut position = 0;

    loop {
        let current_byte = buf_reader.read_u8().await?;
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

pub async fn write_var_int<W>(writer: &mut W, value: i32) -> Result<(), io::Error>
where
    W: AsyncWriteExt + Unpin,
{
    let mut val = value as u32;

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

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::BufReader;

    const VARINT_SAMPLES: &[(i32, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (2, &[0x02]),
        (127, &[0x7F]),
        (128, &[0x80, 0x01]),
        (255, &[0xFF, 0x01]),
        (25565, &[0xDD, 0xC7, 0x01]),
        (2097151, &[0xFF, 0xFF, 0x7F]),
        (2147483647, &[0xFF, 0xFF, 0xFF, 0xFF, 0x07]),
        (-1, &[0xFF, 0xFF, 0xFF, 0xFF, 0x0F]),
        (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0x08]),
    ];

    #[tokio::test]
    async fn test_read_var_int() {
        for (expected_value, bytes) in VARINT_SAMPLES {
            let mut reader = BufReader::new(*bytes);
            let result = read_var_int(&mut reader).await.expect("Failed to read VarInt");
            assert_eq!(result, *expected_value, "Mismatch for bytes {:?}", bytes);
        }
    }

    #[tokio::test]
    async fn test_write_var_int() {
        for (value, expected_bytes) in VARINT_SAMPLES {
            let mut buf = Vec::new();
            write_var_int(&mut buf, *value).await.expect("Failed to write VarInt");
            assert_eq!(&buf, expected_bytes, "Mismatch for value {}", value);
        }
        }

    #[tokio::test]
    async fn test123() {
        let mut buf = Vec::new();
        write_var_int(&mut buf, 173).await;
        println!("{:?}", buf);
    }

    #[tokio::test]
    async fn test_read_write_roundtrip() {
        for (value, _) in VARINT_SAMPLES {
            let mut buf = Vec::new();
            write_var_int(&mut buf, *value).await.expect("Failed to write VarInt");

            let mut reader = BufReader::new(buf.as_slice());
            let result = read_var_int(&mut reader).await.expect("Failed to read VarInt");

            assert_eq!(result, *value, "Roundtrip failed for value {}", value);
        }
    }

    #[tokio::test]
    async fn test_read_var_int_too_big() {
        // 5 bytes all with the continue bit set — exceeds 32-bit VarInt limit
        let bytes: &[u8] = &[0x80, 0x80, 0x80, 0x80, 0x80];
        let mut reader = BufReader::new(bytes);
        let result = read_var_int(&mut reader).await;
        assert!(result.is_err(), "Expected error for oversized VarInt");
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn test_read_var_int_unexpected_eof() {
        // Byte with continue bit set but no following byte
        let bytes: &[u8] = &[0x80];
        let mut reader = BufReader::new(bytes);
        let result = read_var_int(&mut reader).await;
        assert!(result.is_err(), "Expected error on unexpected EOF");
    }
}