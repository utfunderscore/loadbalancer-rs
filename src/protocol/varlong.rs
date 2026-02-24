use std::io;
use std::io::ErrorKind::InvalidData;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const SEGMENT_BITS: u8 = 0x7F;
const CONTINUE_BIT: u8 = 0x80;

pub async fn read_var_long<W>(buf_reader: &mut W) -> Result<i64, io::Error>
where
    W: AsyncReadExt + Unpin,
{
    let mut value: i64 = 0;
    let mut position: i32 = 0;

    loop {
        let current_byte = buf_reader.read_u8().await?;
        value |= ((current_byte & SEGMENT_BITS) as i64) << position;

        if current_byte & CONTINUE_BIT == 0 {
            break;
        }
        position += 7;
        if position > 64 {
            return Err(io::Error::new(InvalidData, "VarLong is too big"))
        }
    }

    Ok(value)
}

pub async fn write_var_long<W>(writer: &mut W, value: i64) -> Result<(), io::Error>
where
    W: AsyncWriteExt + Unpin,
{
    let mut val = value as u64;

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
    use std::io::Cursor;
    use tokio::io::AsyncWriteExt;

    /// Test data from the VarLong specification
    const VARLONG_SAMPLES: &[(i64, &[u8])] = &[
        (0, &[0x00]),
        (1, &[0x01]),
        (2, &[0x02]),
        (127, &[0x7f]),
        (128, &[0x80, 0x01]),
        (255, &[0xff, 0x01]),
        (2147483647, &[0xff, 0xff, 0xff, 0xff, 0x07]),
        (9223372036854775807, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x7f]),
        (-1, &[0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x01]),
        (-2147483648, &[0x80, 0x80, 0x80, 0x80, 0xf8, 0xff, 0xff, 0xff, 0xff, 0x01]),
        (-9223372036854775808, &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01]),
    ];

    /// Helper function to write a VarLong for testing purposes only
    /// Note: This uses raw two's complement encoding (not ZigZag) to match read_var_long
    async fn write_var_long<W>(writer: &mut W, value: i64) -> Result<(), io::Error>
    where
        W: AsyncWriteExt + Unpin,
    {
        let mut unsigned_val = value as u64;
        loop {
            let mut byte = (unsigned_val & 0x7F) as u8;
            unsigned_val >>= 7;
            if unsigned_val != 0 {
                byte |= CONTINUE_BIT;
            }
            writer.write_u8(byte).await?;
            if unsigned_val == 0 {
                break;
            }
        }
        Ok(())
    }

    #[tokio::test]
    async fn test_read_var_long_samples() {
        for (expected_value, bytes) in VARLONG_SAMPLES {
            let mut cursor = Cursor::new(bytes);
            let decoded = read_var_long(&mut cursor).await.unwrap();
            assert_eq!(
                decoded, *expected_value,
                "Decoding failed for bytes {:?}, expected {}, got {}",
                bytes, expected_value, decoded
            );
        }
    }

    #[tokio::test]
    async fn test_write_var_long_samples() {
        for (value, expected_bytes) in VARLONG_SAMPLES {
            let mut buffer = Vec::new();
            write_var_long(&mut buffer, *value).await.unwrap();
            assert_eq!(
                &buffer, expected_bytes,
                "Encoding failed for value {}, expected {:?}, got {:?}",
                value, expected_bytes, buffer
            );
        }
    }

    #[tokio::test]
    async fn test_roundtrip_all_samples() {
        for (original_value, _) in VARLONG_SAMPLES {
            // Encode
            let mut buffer = Vec::new();
            write_var_long(&mut buffer, *original_value).await.unwrap();

            // Decode
            let mut cursor = Cursor::new(buffer);
            let decoded = read_var_long(&mut cursor).await.unwrap();

            assert_eq!(
                decoded, *original_value,
                "Roundtrip failed for {}",
                original_value
            );
        }
    }

    #[tokio::test]
    async fn test_var_long_too_large() {
        // A malformed VarLong with 10 bytes all having continue bit set
        // This forces position to exceed 64 bits (max valid is 10 bytes with 10th byte having no continue bit)
        let malformed = vec![0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80];
        let mut cursor = Cursor::new(malformed);

        let result = read_var_long(&mut cursor).await;
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn test_var_long_incomplete() {
        // Test with truncated data - VarLong that doesn't terminate
        let incomplete = vec![0x80, 0x80, 0x80];
        let mut cursor = Cursor::new(incomplete);

        let result = read_var_long(&mut cursor).await;
        // Should get an UnexpectedEof error since we run out of bytes mid-encoding
        assert!(result.is_err());
        assert_eq!(result.unwrap_err().kind(), std::io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn test_var_long_max_valid_size() {
        // Ensure i64::MIN (10 bytes) decodes correctly - this is the maximum valid size
        let bytes = &[0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x01];
        let mut cursor = Cursor::new(bytes);
        let decoded = read_var_long(&mut cursor).await.unwrap();
        assert_eq!(decoded, -9223372036854775808);
    }
}