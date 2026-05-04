use std::io::{Read, Write};

use crate::codecs::netpbm::{
    HeaderParser, NetpbmImage, raster_slice, read_any_sample_header,
    read_ascii_samples_with_max_value, validate_image_view, write_packed_rows, write_sample_header,
    write_sample_header_with_max_value,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P5";
const ASCII_MAGIC: &[u8] = b"P2";

/// Decodes a binary PGM P5 image.
///
/// This initial implementation supports only 8-bit grayscale images with max value 255.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    pgm_native_to_image(decode_native(reader)?)
}

/// Decodes a binary PGM P5 image while preserving its max value.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, MAGIC)?;
    reject_unsupported_native_maxval(maxval)?;

    parser.consume_raster_separator()?;
    let raster = raster_slice(&data, &parser, dimensions, PixelFormat::Gray8)?;

    Ok(NetpbmImage::Pgm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: raster.to_vec(),
    })
}

/// Decodes an ASCII PGM P2 image.
///
/// This initial implementation supports only 8-bit grayscale images with max value 255.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    pgm_native_to_image(decode_ascii_native(reader)?)
}

/// Decodes an ASCII PGM P2 image while preserving its max value.
pub fn decode_ascii_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, ASCII_MAGIC)?;
    reject_unsupported_native_maxval(maxval)?;

    let expected = dimensions.width as usize
        * dimensions.height as usize
        * PixelFormat::Gray8.bytes_per_pixel();
    let pixels = read_ascii_samples_with_max_value(&mut parser, expected, maxval as u32)?;

    Ok(NetpbmImage::Pgm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: pixels,
    })
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

/// Encodes a native PGM image as binary PGM P5.
pub fn encode_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_pgm_image(image)?;

    write_sample_header_with_max_value(writer, "P5", width, height, maxval)?;
    writer.write_all(data)?;

    Ok(())
}

/// Encodes a native PGM image as ASCII PGM P2.
pub fn encode_ascii_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_pgm_image(image)?;

    write_sample_header_with_max_value(writer, "P2", width, height, maxval)?;

    for sample in data {
        writeln!(writer, "{sample}")?;
    }

    Ok(())
}

fn pgm_native_to_image(image: NetpbmImage) -> Result<Image> {
    let NetpbmImage::Pgm {
        width,
        height,
        maxval,
        data,
    } = image
    else {
        return Err(ImageError::UnsupportedFormat);
    };

    if maxval != 255 {
        return Err(ImageError::InvalidHeader {
            reason: "only max value 255 is supported",
        });
    }

    Image::new(width, height, PixelFormat::Gray8, data)
}

fn validate_native_pgm_image(image: &NetpbmImage) -> Result<(u32, u32, u16, &[u8])> {
    let NetpbmImage::Pgm {
        width,
        height,
        maxval,
        data,
    } = image
    else {
        return Err(ImageError::UnsupportedFormat);
    };

    validate_native_sample_image(*width, *height, *maxval, data, PixelFormat::Gray8)?;
    Ok((*width, *height, *maxval, data))
}

fn reject_unsupported_native_maxval(maxval: u16) -> Result<()> {
    if maxval == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "max value must be between 1 and 65535",
        });
    }

    if maxval > 255 {
        return Err(ImageError::InvalidHeader {
            reason: "only max value 255 is supported",
        });
    }

    Ok(())
}

fn validate_native_sample_image(
    width: u32,
    height: u32,
    maxval: u16,
    data: &[u8],
    pixel_format: PixelFormat,
) -> Result<()> {
    reject_unsupported_native_maxval(maxval)?;

    if width == 0 || height == 0 {
        return Err(ImageError::InvalidData {
            reason: "width and height must be greater than zero",
        });
    }

    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(pixel_format.bytes_per_pixel()))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: pixel_format.bytes_per_pixel(),
        })?;

    if data.len() != expected {
        return Err(ImageError::InvalidBufferLength {
            expected,
            actual: data.len(),
        });
    }

    if data.iter().any(|sample| *sample as u16 > maxval) {
        return Err(ImageError::InvalidData {
            reason: "sample value exceeds max value",
        });
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
    fn decode_native_reads_pgm_p5_with_max_value() {
        let input = b"P5\n2 1\n15\n\x00\x0f";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![0, 15]
            }
        );
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
    fn encode_native_writes_pgm_p5_with_max_value() {
        let image = NetpbmImage::Pgm {
            width: 2,
            height: 1,
            maxval: 15,
            data: vec![0, 15],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P5\n2 1\n15\n\0\x0f");
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
    fn decode_ascii_native_reads_pgm_p2_with_max_value() {
        let input = b"P2\n2 1\n15\n0 15\n";
        let image = decode_ascii_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![0, 15]
            }
        );
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
    fn encode_ascii_native_writes_pgm_p2_with_max_value() {
        let image = NetpbmImage::Pgm {
            width: 2,
            height: 1,
            maxval: 15,
            data: vec![0, 15],
        };
        let mut output = Vec::new();

        encode_ascii_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P2\n2 1\n15\n0\n15\n");
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
