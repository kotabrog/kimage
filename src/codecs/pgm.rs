use std::io::{Read, Write};

use crate::codecs::netpbm::HeaderParser;
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P5";
const MAX_VALUE: u32 = 255;

/// Decodes a binary PGM P5 image.
///
/// This initial implementation supports only 8-bit grayscale images with max value 255.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let magic = parser.next_token()?;

    if magic != MAGIC {
        return Err(ImageError::UnsupportedFormat);
    }

    let width = parser.next_u32("width")?;
    let height = parser.next_u32("height")?;
    let max_value = parser.next_u32("max value")?;

    if width == 0 || height == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "width and height must be greater than zero",
        });
    }

    if max_value != MAX_VALUE {
        return Err(ImageError::InvalidHeader {
            reason: "only max value 255 is supported",
        });
    }

    parser.consume_raster_separator()?;

    Image::new(
        width,
        height,
        PixelFormat::Gray8,
        data[parser.position()..].to_vec(),
    )
}

/// Encodes an image view as binary PGM P5.
///
/// This initial implementation supports only `PixelFormat::Gray8`.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    if image.pixel_format != PixelFormat::Gray8 {
        return Err(ImageError::UnsupportedPixelFormat {
            pixel_format: image.pixel_format,
        });
    }

    if image.width == 0 || image.height == 0 {
        return Err(ImageError::InvalidData {
            reason: "width and height must be greater than zero",
        });
    }

    let image = ImageView::new(
        image.width,
        image.height,
        image.pixel_format,
        image.stride,
        image.data,
    )?;

    writeln!(writer, "P5")?;
    writeln!(writer, "{} {}", image.width, image.height)?;
    writeln!(writer, "{MAX_VALUE}")?;

    let row_len = image.width as usize * PixelFormat::Gray8.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        writer.write_all(&image.data[start..end])?;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn decode_reads_pgm_p5_gray8_image() {
        let input = b"P5\n2 2\n255\n\x00\x55\xaa\xff";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 85, 170, 255]);
    }

    #[test]
    fn decode_accepts_comments_before_header_tokens() {
        let input = b"P5\n# comment\n2 # width\n1\n255\n\x10\x20";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [16, 32]);
    }

    #[test]
    fn decode_accepts_crlf_raster_separator() {
        let input = b"P5\r\n1 1\r\n255\r\n\x80";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [128]);
    }

    #[test]
    fn decode_rejects_non_pgm_p5_magic() {
        let input = b"P2\n1 1\n255\n0";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_zero_dimensions() {
        let input = b"P5\n0 1\n255\n";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "width and height must be greater than zero"
            }
        );
    }

    #[test]
    fn decode_rejects_unsupported_max_value() {
        let input = b"P5\n1 1\n65535\n\0";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "only max value 255 is supported"
            }
        );
    }

    #[test]
    fn decode_rejects_short_raster_data() {
        let input = b"P5\n2 1\n255\n\x01";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 2,
                actual: 1
            }
        );
    }

    #[test]
    fn encode_writes_pgm_p5_gray8_image() {
        let data = [0, 255];
        let image = ImageView::new(2, 1, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P5\n2 1\n255\n\0\xff");
    }

    #[test]
    fn encode_writes_only_pixel_bytes_from_strided_rows() {
        let data = [10, 99, 20, 88];
        let image = ImageView::new(1, 2, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P5\n1 2\n255\n\x0a\x14");
    }

    #[test]
    fn encode_rejects_non_gray8_pixel_format() {
        let data = [0, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let error = encode(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Rgb8
            }
        );
    }

    #[test]
    fn encode_rejects_zero_dimensions() {
        let image = ImageView {
            width: 0,
            height: 1,
            pixel_format: PixelFormat::Gray8,
            stride: 0,
            data: &[],
        };
        let error = encode(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "width and height must be greater than zero"
            }
        );
    }

    #[test]
    fn integration_decode_encode_roundtrip_preserves_pixels() {
        let input = b"P5\n2 1\n255\n\x10\x20";
        let image = decode(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image.as_view()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }
}
