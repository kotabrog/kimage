use std::io::{Read, Write};

use crate::codecs::netpbm::{
    HeaderParser, read_ascii_samples, read_sample_header, validate_image_view, write_packed_rows,
    write_sample_header,
};
use crate::{Image, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P5";
const ASCII_MAGIC: &[u8] = b"P2";

/// Decodes a binary PGM P5 image.
///
/// This initial implementation supports only 8-bit grayscale images with max value 255.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let dimensions = read_sample_header(&mut parser, MAGIC)?;

    parser.consume_raster_separator()?;

    Image::new(
        dimensions.width,
        dimensions.height,
        PixelFormat::Gray8,
        data[parser.position()..].to_vec(),
    )
}

/// Decodes an ASCII PGM P2 image.
///
/// This initial implementation supports only 8-bit grayscale images with max value 255.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let dimensions = read_sample_header(&mut parser, ASCII_MAGIC)?;

    let expected = dimensions.width as usize
        * dimensions.height as usize
        * PixelFormat::Gray8.bytes_per_pixel();
    let pixels = read_ascii_samples(&mut parser, expected)?;

    Image::new(
        dimensions.width,
        dimensions.height,
        PixelFormat::Gray8,
        pixels,
    )
}

/// Encodes an image view as binary PGM P5.
///
/// This initial implementation supports only `PixelFormat::Gray8`.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Gray8)?;

    write_sample_header(writer, "P5", image)?;
    write_packed_rows(writer, image)
}

/// Encodes an image view as ASCII PGM P2.
///
/// This initial implementation supports only `PixelFormat::Gray8`.
pub fn encode_ascii<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Gray8)?;

    write_sample_header(writer, "P2", image)?;

    let row_len = image.width as usize * PixelFormat::Gray8.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;

        for sample in &image.data[start..end] {
            writeln!(writer, "{sample}")?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::ImageError;

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
    fn decode_ascii_reads_pgm_p2_gray8_image() {
        let input = b"P2\n2 2\n255\n0 85\n170 255\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 85, 170, 255]);
    }

    #[test]
    fn decode_ascii_accepts_comments_and_multiple_whitespace() {
        let input = b"P2\r\n# comment\r\n1 2\r\n255\r\n128\t255\r\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 2);
        assert_eq!(image.data, [128, 255]);
    }

    #[test]
    fn decode_ascii_rejects_non_pgm_p2_magic() {
        let input = b"P5\n1 1\n255\n0";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_ascii_rejects_short_samples() {
        let input = b"P2\n2 1\n255\n0";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "unexpected end of header"
            }
        );
    }

    #[test]
    fn decode_ascii_rejects_too_many_samples() {
        let input = b"P2\n1 1\n255\n0 1";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "too many samples"
            }
        );
    }

    #[test]
    fn decode_ascii_rejects_sample_above_max_value() {
        let input = b"P2\n1 1\n255\n256";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "sample value exceeds max value"
            }
        );
    }

    #[test]
    fn encode_ascii_writes_pgm_p2_gray8_image() {
        let data = [0, 255];
        let image = ImageView::new(2, 1, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P2\n2 1\n255\n0\n255\n");
    }

    #[test]
    fn encode_ascii_writes_only_pixel_bytes_from_strided_rows() {
        let data = [10, 99, 20, 88];
        let image = ImageView::new(1, 2, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P2\n1 2\n255\n10\n20\n");
    }

    #[test]
    fn encode_ascii_rejects_non_gray8_pixel_format() {
        let data = [0, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let error = encode_ascii(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Rgb8
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

    #[test]
    fn integration_decode_encode_ascii_roundtrip_preserves_pixels() {
        let input = b"P2\n2 1\n255\n16 32\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image.as_view()).unwrap();
        let decoded = decode_ascii(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }
}
