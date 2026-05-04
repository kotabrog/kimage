use std::io::{Read, Write};

use crate::codecs::netpbm::{
    HeaderParser, NetpbmImage, normalize_sample_to_u8, raster_slice, read_any_sample_header,
    read_ascii_samples_with_max_value, validate_image_view, write_packed_rows, write_sample_header,
    write_sample_header_with_max_value,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P6";
const ASCII_MAGIC: &[u8] = b"P3";

/// Decodes a binary PPM P6 image.
///
/// This implementation normalizes 8-bit PPM samples to `PixelFormat::Rgb8`.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    ppm_native_to_image(decode_native(reader)?)
}

/// Decodes a binary PPM P6 image while preserving its max value.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, MAGIC)?;
    reject_unsupported_native_maxval(maxval)?;

    parser.consume_raster_separator()?;
    let raster = raster_slice(&data, &parser, dimensions, PixelFormat::Rgb8)?;

    Ok(NetpbmImage::Ppm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: raster.to_vec(),
    })
}

/// Decodes an ASCII PPM P3 image.
///
/// This implementation normalizes 8-bit PPM samples to `PixelFormat::Rgb8`.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    ppm_native_to_image(decode_ascii_native(reader)?)
}

/// Decodes an ASCII PPM P3 image while preserving its max value.
pub fn decode_ascii_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, ASCII_MAGIC)?;
    reject_unsupported_native_maxval(maxval)?;

    let expected = dimensions.width as usize
        * dimensions.height as usize
        * PixelFormat::Rgb8.bytes_per_pixel();
    let pixels = read_ascii_samples_with_max_value(&mut parser, expected, maxval as u32)?;

    Ok(NetpbmImage::Ppm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: pixels,
    })
}

/// Encodes an image view as binary PPM P6.
///
/// This initial implementation supports only `PixelFormat::Rgb8`.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Rgb8)?;

    write_sample_header(writer, "P6", image)?;
    write_packed_rows(writer, image)
}

/// Encodes an image view as ASCII PPM P3.
///
/// This initial implementation supports only `PixelFormat::Rgb8`.
pub fn encode_ascii<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Rgb8)?;

    write_sample_header(writer, "P3", image)?;

    let row_len = image.width as usize * PixelFormat::Rgb8.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        let row_data = &image.data[start..end];

        for pixel in row_data.chunks_exact(3) {
            writeln!(writer, "{} {} {}", pixel[0], pixel[1], pixel[2])?;
        }
    }

    Ok(())
}

/// Encodes a native PPM image as binary PPM P6.
pub fn encode_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_ppm_image(image)?;

    write_sample_header_with_max_value(writer, "P6", width, height, maxval)?;
    writer.write_all(data)?;

    Ok(())
}

/// Encodes a native PPM image as ASCII PPM P3.
pub fn encode_ascii_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_ppm_image(image)?;

    write_sample_header_with_max_value(writer, "P3", width, height, maxval)?;

    for pixel in data.chunks_exact(3) {
        writeln!(writer, "{} {} {}", pixel[0], pixel[1], pixel[2])?;
    }

    Ok(())
}

fn ppm_native_to_image(image: NetpbmImage) -> Result<Image> {
    let NetpbmImage::Ppm {
        width,
        height,
        maxval,
        data,
    } = image
    else {
        return Err(ImageError::UnsupportedFormat);
    };

    if maxval == u16::from(u8::MAX) {
        return Image::new(width, height, PixelFormat::Rgb8, data);
    }

    let data = data
        .into_iter()
        .map(|sample| normalize_sample_to_u8(sample, maxval))
        .collect();

    Image::new(width, height, PixelFormat::Rgb8, data)
}

fn validate_native_ppm_image(image: &NetpbmImage) -> Result<(u32, u32, u16, &[u8])> {
    let NetpbmImage::Ppm {
        width,
        height,
        maxval,
        data,
    } = image
    else {
        return Err(ImageError::UnsupportedFormat);
    };

    validate_native_sample_image(*width, *height, *maxval, data, PixelFormat::Rgb8)?;
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
    fn decode_reads_ppm_p6_rgb8_image() {
        let input = b"P6\n2 2\n255\n\xff\0\0\0\xff\0\0\0\xff\xff\xff\xff";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255]);
    }

    #[test]
    fn decode_native_reads_ppm_p6_with_max_value() {
        let input = b"P6\n2 1\n15\n\x0f\0\0\0\x0f\0";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![15, 0, 0, 0, 15, 0]
            }
        );
    }

    #[test]
    fn decode_normalizes_ppm_p6_max_value_to_rgb8() {
        let input = b"P6\n2 1\n15\n\x0f\0\x05\0\x0a\x0f";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 85, 0, 170, 255]);
    }

    #[test]
    fn decode_normalizes_ppm_p6_max_value_with_nearest_rounding() {
        let input = b"P6\n1 1\n10\n\x05\0\x0a";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.data, [128, 0, 255]);
    }

    #[test]
    fn decode_accepts_comments_before_header_tokens() {
        let input = b"P6\n# comment\n2 # width\n1\n255\n\x01\x02\x03\x04\x05\x06";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [1, 2, 3, 4, 5, 6]);
    }

    #[test]
    fn decode_accepts_crlf_raster_separator() {
        let input = b"P6\r\n1 1\r\n255\r\n\x01\x02\x03";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [1, 2, 3]);
    }

    #[test]
    fn decode_rejects_non_ppm_p6_magic() {
        let input = b"P3\n1 1\n255\n0 0 0";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_zero_dimensions() {
        let input = b"P6\n0 1\n255\n";
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
        let input = b"P6\n1 1\n65535\n\0\0\0";
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
        let input = b"P6\n2 1\n255\n\x01\x02\x03";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 6,
                actual: 3
            }
        );
    }

    #[test]
    fn encode_writes_ppm_p6_rgb8_image() {
        let data = [255, 0, 0, 0, 255, 0];
        let image = ImageView::new(2, 1, PixelFormat::Rgb8, 6, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P6\n2 1\n255\n\xff\0\0\0\xff\0");
    }

    #[test]
    fn encode_native_writes_ppm_p6_with_max_value() {
        let image = NetpbmImage::Ppm {
            width: 2,
            height: 1,
            maxval: 15,
            data: vec![15, 0, 0, 0, 15, 0],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P6\n2 1\n15\n\x0f\0\0\0\x0f\0");
    }

    #[test]
    fn encode_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 0, 0, 99, 0, 255, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Rgb8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P6\n1 2\n255\n\xff\0\0\0\xff\0");
    }

    #[test]
    fn encode_rejects_non_rgb8_pixel_format() {
        let data = [0, 0, 0, 255];
        let image = ImageView::new(1, 1, PixelFormat::Rgba8, 4, &data).unwrap();
        let error = encode(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Rgba8
            }
        );
    }

    #[test]
    fn encode_rejects_zero_dimensions() {
        let image = ImageView {
            width: 0,
            height: 1,
            pixel_format: PixelFormat::Rgb8,
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
    fn decode_ascii_reads_ppm_p3_rgb8_image() {
        let input = b"P3\n2 1\n255\n255 0 0\n0 255 0\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn decode_ascii_native_reads_ppm_p3_with_max_value() {
        let input = b"P3\n2 1\n15\n15 0 0\n0 15 0\n";
        let image = decode_ascii_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![15, 0, 0, 0, 15, 0]
            }
        );
    }

    #[test]
    fn decode_ascii_normalizes_ppm_p3_max_value_to_rgb8() {
        let input = b"P3\n2 1\n15\n15 0 5\n0 10 15\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 85, 0, 170, 255]);
    }

    #[test]
    fn decode_ascii_accepts_comments_and_multiple_whitespace() {
        let input = b"P3\r\n# comment\r\n1 2\r\n255\r\n255\t0 0\n\n0 0 255\r\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 2);
        assert_eq!(image.data, [255, 0, 0, 0, 0, 255]);
    }

    #[test]
    fn decode_ascii_rejects_non_ppm_p3_magic() {
        let input = b"P6\n1 1\n255\n0 0 0";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_ascii_rejects_short_samples() {
        let input = b"P3\n1 1\n255\n0 0";
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
        let input = b"P3\n1 1\n255\n0 0 0 1";
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
        let input = b"P3\n1 1\n255\n256 0 0";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "sample value exceeds max value"
            }
        );
    }

    #[test]
    fn encode_ascii_writes_ppm_p3_rgb8_image() {
        let data = [255, 0, 0, 0, 255, 0];
        let image = ImageView::new(2, 1, PixelFormat::Rgb8, 6, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P3\n2 1\n255\n255 0 0\n0 255 0\n");
    }

    #[test]
    fn encode_ascii_native_writes_ppm_p3_with_max_value() {
        let image = NetpbmImage::Ppm {
            width: 2,
            height: 1,
            maxval: 15,
            data: vec![15, 0, 0, 0, 15, 0],
        };
        let mut output = Vec::new();

        encode_ascii_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P3\n2 1\n15\n15 0 0\n0 15 0\n");
    }

    #[test]
    fn encode_ascii_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 0, 0, 99, 0, 255, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Rgb8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P3\n1 2\n255\n255 0 0\n0 255 0\n");
    }

    #[test]
    fn encode_ascii_rejects_non_rgb8_pixel_format() {
        let data = [0];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let error = encode_ascii(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Gray8
            }
        );
    }

    #[test]
    fn integration_decode_encode_roundtrip_preserves_pixels() {
        let input = b"P6\n2 1\n255\n\x10\x20\x30\x40\x50\x60";
        let image = decode(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image.as_view()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }

    #[test]
    fn integration_decode_encode_ascii_roundtrip_preserves_pixels() {
        let input = b"P3\n2 1\n255\n16 32 48\n64 80 96\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image.as_view()).unwrap();
        let decoded = decode_ascii(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }
}
