use std::io::{Read, Write};

use crate::codecs::netpbm::{
    HeaderParser, NetpbmImage, normalize_sample_to_u8, normalize_sample_to_u16, raster_slice,
    read_any_sample_header, read_ascii_sample_bytes_with_max_value, validate_image_view,
    write_packed_rows, write_sample_header_with_max_value,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P6";
const ASCII_MAGIC: &[u8] = b"P3";

/// Decodes a binary PPM P6 image.
///
/// This implementation normalizes PPM samples to `PixelFormat::Rgb8` or `PixelFormat::Rgb16`.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    ppm_native_to_image(decode_native(reader)?)
}

/// Decodes a binary PPM P6 image while preserving its max value.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, MAGIC)?;
    parser.consume_raster_separator()?;
    let pixel_format = ppm_pixel_format_for_maxval(maxval);
    let raster = raster_slice(&data, &parser, dimensions, pixel_format)?;
    let data = if maxval < 256 {
        raster.to_vec()
    } else {
        be_samples_to_le_bytes(raster)
    };

    Ok(NetpbmImage::Ppm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data,
    })
}

/// Decodes an ASCII PPM P3 image.
///
/// This implementation normalizes PPM samples to `PixelFormat::Rgb8` or `PixelFormat::Rgb16`.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    ppm_native_to_image(decode_ascii_native(reader)?)
}

/// Decodes an ASCII PPM P3 image while preserving its max value.
pub fn decode_ascii_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, ASCII_MAGIC)?;
    let expected =
        dimensions.width as usize * dimensions.height as usize * PixelFormat::Rgb8.channels();
    let pixels = read_ascii_sample_bytes_with_max_value(&mut parser, expected, maxval)?;

    Ok(NetpbmImage::Ppm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: pixels,
    })
}

/// Encodes an image view as binary PPM P6.
///
/// This implementation supports `PixelFormat::Rgb8` and `PixelFormat::Rgb16`.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let maxval = image_maxval(image.pixel_format, PixelFormat::Rgb8, PixelFormat::Rgb16)?;
    let image = validate_image_view(image, image.pixel_format)?;

    write_sample_header_with_max_value(writer, "P6", image.width, image.height, maxval)?;

    if image.pixel_format == PixelFormat::Rgb8 {
        write_packed_rows(writer, image)
    } else {
        write_16_bit_rows_as_be(writer, image)
    }
}

/// Encodes an image view as ASCII PPM P3.
///
/// This implementation supports `PixelFormat::Rgb8` and `PixelFormat::Rgb16`.
pub fn encode_ascii<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let maxval = image_maxval(image.pixel_format, PixelFormat::Rgb8, PixelFormat::Rgb16)?;
    let image = validate_image_view(image, image.pixel_format)?;

    write_sample_header_with_max_value(writer, "P3", image.width, image.height, maxval)?;

    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        let row_data = &image.data[start..end];

        if image.pixel_format == PixelFormat::Rgb8 {
            for pixel in row_data.chunks_exact(3) {
                writeln!(writer, "{} {} {}", pixel[0], pixel[1], pixel[2])?;
            }
        } else {
            for pixel in row_data.chunks_exact(6) {
                let red = u16::from_le_bytes([pixel[0], pixel[1]]);
                let green = u16::from_le_bytes([pixel[2], pixel[3]]);
                let blue = u16::from_le_bytes([pixel[4], pixel[5]]);
                writeln!(writer, "{red} {green} {blue}")?;
            }
        }
    }

    Ok(())
}

/// Encodes a native PPM image as binary PPM P6.
pub fn encode_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_ppm_image(image)?;

    write_sample_header_with_max_value(writer, "P6", width, height, maxval)?;
    if maxval < 256 {
        writer.write_all(data)?;
    } else {
        write_le_sample_bytes_as_be(writer, data)?;
    }

    Ok(())
}

/// Encodes a native PPM image as ASCII PPM P3.
pub fn encode_ascii_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_ppm_image(image)?;

    write_sample_header_with_max_value(writer, "P3", width, height, maxval)?;

    if maxval < 256 {
        for pixel in data.chunks_exact(3) {
            writeln!(writer, "{} {} {}", pixel[0], pixel[1], pixel[2])?;
        }
    } else {
        for pixel in data.chunks_exact(6) {
            let red = u16::from_le_bytes([pixel[0], pixel[1]]);
            let green = u16::from_le_bytes([pixel[2], pixel[3]]);
            let blue = u16::from_le_bytes([pixel[4], pixel[5]]);
            writeln!(writer, "{red} {green} {blue}")?;
        }
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

    if maxval < 256 {
        let data = data
            .into_iter()
            .map(|sample| normalize_sample_to_u8(sample, maxval))
            .collect();

        return Image::new(width, height, PixelFormat::Rgb8, data);
    }

    let mut normalized = Vec::with_capacity(data.len());
    for sample in data.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        normalized.extend_from_slice(&normalize_sample_to_u16(sample, maxval).to_le_bytes());
    }

    Image::new(width, height, PixelFormat::Rgb16, normalized)
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

    validate_native_sample_image(*width, *height, *maxval, data, PixelFormat::Rgb8.channels())?;
    Ok((*width, *height, *maxval, data))
}

fn reject_unsupported_native_maxval(maxval: u16) -> Result<()> {
    if maxval == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "max value must be between 1 and 65535",
        });
    }

    Ok(())
}

fn validate_native_sample_image(
    width: u32,
    height: u32,
    maxval: u16,
    data: &[u8],
    channels: usize,
) -> Result<()> {
    reject_unsupported_native_maxval(maxval)?;

    if width == 0 || height == 0 {
        return Err(ImageError::InvalidData {
            reason: "width and height must be greater than zero",
        });
    }

    let expected = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(channels))
        .and_then(|samples| samples.checked_mul(bytes_per_sample(maxval)))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: channels * bytes_per_sample(maxval),
        })?;

    if data.len() != expected {
        return Err(ImageError::InvalidBufferLength {
            expected,
            actual: data.len(),
        });
    }

    if maxval < 256 && data.iter().any(|sample| *sample as u16 > maxval) {
        return Err(ImageError::InvalidData {
            reason: "sample value exceeds max value",
        });
    }

    if maxval >= 256
        && data
            .chunks_exact(2)
            .any(|sample| u16::from_le_bytes([sample[0], sample[1]]) > maxval)
    {
        return Err(ImageError::InvalidData {
            reason: "sample value exceeds max value",
        });
    }

    Ok(())
}

fn ppm_pixel_format_for_maxval(maxval: u16) -> PixelFormat {
    if maxval < 256 {
        PixelFormat::Rgb8
    } else {
        PixelFormat::Rgb16
    }
}

fn bytes_per_sample(maxval: u16) -> usize {
    if maxval < 256 { 1 } else { 2 }
}

fn image_maxval(
    actual: PixelFormat,
    eight_bit: PixelFormat,
    sixteen_bit: PixelFormat,
) -> Result<u16> {
    match actual {
        format if format == eight_bit => Ok(u16::from(u8::MAX)),
        format if format == sixteen_bit => Ok(u16::MAX),
        pixel_format => Err(ImageError::UnsupportedPixelFormat { pixel_format }),
    }
}

fn be_samples_to_le_bytes(data: &[u8]) -> Vec<u8> {
    let mut output = Vec::with_capacity(data.len());

    for sample in data.chunks_exact(2) {
        output.extend_from_slice(&u16::from_be_bytes([sample[0], sample[1]]).to_le_bytes());
    }

    output
}

fn write_le_sample_bytes_as_be<W: Write>(writer: &mut W, data: &[u8]) -> Result<()> {
    for sample in data.chunks_exact(2) {
        writer.write_all(&u16::from_le_bytes([sample[0], sample[1]]).to_be_bytes())?;
    }

    Ok(())
}

fn write_16_bit_rows_as_be<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        write_le_sample_bytes_as_be(writer, &image.data[start..end])?;
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
    fn decode_native_reads_ppm_p6_16_bit_samples_as_little_endian() {
        let input = b"P6\n1 1\n65535\n\x12\x34\x56\x78\xff\xff";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff]
            }
        );
    }

    #[test]
    fn decode_reads_ppm_p6_16_bit_as_rgb16_image() {
        let input = b"P6\n1 1\n65535\n\x12\x34\x56\x78\xff\xff";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb16);
        assert_eq!(image.data, [0x34, 0x12, 0x78, 0x56, 0xff, 0xff]);
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
    fn decode_rejects_short_16_bit_raster_data() {
        let input = b"P6\n1 1\n65535\n\0\0\0";
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
    fn encode_native_writes_ppm_p6_16_bit_samples_as_big_endian() {
        let image = NetpbmImage::Ppm {
            width: 1,
            height: 1,
            maxval: 65535,
            data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P6\n1 1\n65535\n\x12\x34\x56\x78\xff\xff");
    }

    #[test]
    fn encode_writes_ppm_p6_rgb16_image_as_big_endian() {
        let data = [0x34, 0x12, 0x78, 0x56, 0xff, 0xff];
        let image = ImageView::new(1, 1, PixelFormat::Rgb16, 6, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P6\n1 1\n65535\n\x12\x34\x56\x78\xff\xff");
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
    fn decode_ascii_native_reads_ppm_p3_16_bit_samples_as_little_endian() {
        let input = b"P3\n1 1\n65535\n4660 22136 65535\n";
        let image = decode_ascii_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff]
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
    fn decode_ascii_reads_ppm_p3_16_bit_as_rgb16_image() {
        let input = b"P3\n1 1\n65535\n4660 22136 65535\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb16);
        assert_eq!(image.data, [0x34, 0x12, 0x78, 0x56, 0xff, 0xff]);
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
    fn encode_ascii_native_writes_ppm_p3_16_bit_samples() {
        let image = NetpbmImage::Ppm {
            width: 1,
            height: 1,
            maxval: 65535,
            data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff],
        };
        let mut output = Vec::new();

        encode_ascii_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P3\n1 1\n65535\n4660 22136 65535\n");
    }

    #[test]
    fn encode_ascii_writes_ppm_p3_rgb16_image() {
        let data = [0x34, 0x12, 0x78, 0x56, 0xff, 0xff];
        let image = ImageView::new(1, 1, PixelFormat::Rgb16, 6, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P3\n1 1\n65535\n4660 22136 65535\n");
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
