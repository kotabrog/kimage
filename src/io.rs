use std::io::{Read, Write};

/// Reads a little-endian `u16` from the reader.
pub fn read_u16_le<R: Read>(reader: &mut R) -> std::io::Result<u16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

/// Reads a little-endian `u32` from the reader.
pub fn read_u32_le<R: Read>(reader: &mut R) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

/// Reads a big-endian `u16` from the reader.
pub fn read_u16_be<R: Read>(reader: &mut R) -> std::io::Result<u16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_be_bytes(bytes))
}

/// Reads a big-endian `u32` from the reader.
pub fn read_u32_be<R: Read>(reader: &mut R) -> std::io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_be_bytes(bytes))
}

/// Writes a little-endian `u16` to the writer.
pub fn write_u16_le<W: Write>(writer: &mut W, value: u16) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

/// Writes a little-endian `u32` to the writer.
pub fn write_u32_le<W: Write>(writer: &mut W, value: u32) -> std::io::Result<()> {
    writer.write_all(&value.to_le_bytes())
}

/// Writes a big-endian `u16` to the writer.
pub fn write_u16_be<W: Write>(writer: &mut W, value: u16) -> std::io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

/// Writes a big-endian `u32` to the writer.
pub fn write_u32_be<W: Write>(writer: &mut W, value: u32) -> std::io::Result<()> {
    writer.write_all(&value.to_be_bytes())
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, ErrorKind};

    use super::*;

    #[test]
    fn read_u16_le_reads_little_endian_value() {
        let mut reader = Cursor::new([0x34, 0x12]);

        assert_eq!(read_u16_le(&mut reader).unwrap(), 0x1234);
    }

    #[test]
    fn read_u32_le_reads_little_endian_value() {
        let mut reader = Cursor::new([0x78, 0x56, 0x34, 0x12]);

        assert_eq!(read_u32_le(&mut reader).unwrap(), 0x1234_5678);
    }

    #[test]
    fn read_u16_be_reads_big_endian_value() {
        let mut reader = Cursor::new([0x12, 0x34]);

        assert_eq!(read_u16_be(&mut reader).unwrap(), 0x1234);
    }

    #[test]
    fn read_u32_be_reads_big_endian_value() {
        let mut reader = Cursor::new([0x12, 0x34, 0x56, 0x78]);

        assert_eq!(read_u32_be(&mut reader).unwrap(), 0x1234_5678);
    }

    #[test]
    fn write_u16_le_writes_little_endian_value() {
        let mut writer = Vec::new();

        write_u16_le(&mut writer, 0x1234).unwrap();

        assert_eq!(writer, [0x34, 0x12]);
    }

    #[test]
    fn write_u32_le_writes_little_endian_value() {
        let mut writer = Vec::new();

        write_u32_le(&mut writer, 0x1234_5678).unwrap();

        assert_eq!(writer, [0x78, 0x56, 0x34, 0x12]);
    }

    #[test]
    fn write_u16_be_writes_big_endian_value() {
        let mut writer = Vec::new();

        write_u16_be(&mut writer, 0x1234).unwrap();

        assert_eq!(writer, [0x12, 0x34]);
    }

    #[test]
    fn write_u32_be_writes_big_endian_value() {
        let mut writer = Vec::new();

        write_u32_be(&mut writer, 0x1234_5678).unwrap();

        assert_eq!(writer, [0x12, 0x34, 0x56, 0x78]);
    }

    #[test]
    fn read_u16_le_returns_unexpected_eof_for_short_input() {
        let mut reader = Cursor::new([0x34]);
        let error = read_u16_le(&mut reader).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
    }

    #[test]
    fn read_u32_be_returns_unexpected_eof_for_short_input() {
        let mut reader = Cursor::new([0x12, 0x34, 0x56]);
        let error = read_u32_be(&mut reader).unwrap_err();

        assert_eq!(error.kind(), ErrorKind::UnexpectedEof);
    }
}
