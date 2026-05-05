use std::io::{Read, Write};

use crate::codecs::netpbm::{
    HeaderParser, NetpbmImage, gray_image_to_pgm_native, raster_slice, read_any_sample_header,
    read_ascii_sample_bytes_with_max_value, validate_image_view, write_packed_rows,
    write_sample_header_with_max_value,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P5";
const ASCII_MAGIC: &[u8] = b"P2";

/// Decodes a binary PGM P5 image.
///
/// This implementation normalizes PGM samples to `PixelFormat::Gray8` or `PixelFormat::Gray16`.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_native(reader)?)
}

/// Decodes a binary PGM P5 image while preserving its max value.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    decode_one_native(&data, &mut parser)
}

/// Decodes all binary PGM P5 images from a multi-image stream.
pub fn decode_all_native<R: Read>(reader: &mut R) -> Result<Vec<NetpbmImage>> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let mut images = Vec::new();

    while parser.has_more_tokens() {
        images.push(decode_one_native(&data, &mut parser)?);
    }

    Ok(images)
}

fn decode_one_native(data: &[u8], parser: &mut HeaderParser<'_>) -> Result<NetpbmImage> {
    let (dimensions, maxval) = read_any_sample_header(parser, MAGIC)?;
    parser.consume_raster_separator()?;
    let pixel_format = pgm_pixel_format_for_maxval(maxval);
    let raster = raster_slice(data, parser, dimensions, pixel_format)?;
    let next_position = parser.position() + raster.len();
    let data = if maxval < 256 {
        raster.to_vec()
    } else {
        be_samples_to_le_bytes(raster)
    };
    parser.set_position(next_position);

    Ok(NetpbmImage::Pgm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data,
    })
}

/// Decodes an ASCII PGM P2 image.
///
/// This implementation normalizes PGM samples to `PixelFormat::Gray8` or `PixelFormat::Gray16`.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_ascii_native(reader)?)
}

/// Decodes an ASCII PGM P2 image while preserving its max value.
pub fn decode_ascii_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let (dimensions, maxval) = read_any_sample_header(&mut parser, ASCII_MAGIC)?;
    let expected =
        dimensions.width as usize * dimensions.height as usize * PixelFormat::Gray8.channels();
    let pixels = read_ascii_sample_bytes_with_max_value(&mut parser, expected, maxval)?;

    Ok(NetpbmImage::Pgm {
        width: dimensions.width,
        height: dimensions.height,
        maxval,
        data: pixels,
    })
}

/// Encodes an image view as binary PGM P5.
///
/// This implementation supports `PixelFormat::Gray8` and `PixelFormat::Gray16`.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let maxval = image_maxval(image.pixel_format, PixelFormat::Gray8, PixelFormat::Gray16)?;
    let image = validate_image_view(image, image.pixel_format)?;

    write_sample_header_with_max_value(writer, "P5", image.width, image.height, maxval)?;

    if image.pixel_format == PixelFormat::Gray8 {
        write_packed_rows(writer, image)
    } else {
        write_16_bit_rows_as_be(writer, image)
    }
}

/// Encodes an image view as ASCII PGM P2.
///
/// This implementation supports `PixelFormat::Gray8` and `PixelFormat::Gray16`.
pub fn encode_ascii<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let maxval = image_maxval(image.pixel_format, PixelFormat::Gray8, PixelFormat::Gray16)?;
    let image = validate_image_view(image, image.pixel_format)?;

    write_sample_header_with_max_value(writer, "P2", image.width, image.height, maxval)?;

    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;

        if image.pixel_format == PixelFormat::Gray8 {
            for sample in &image.data[start..end] {
                writeln!(writer, "{sample}")?;
            }
        } else {
            for sample in image.data[start..end].chunks_exact(2) {
                writeln!(writer, "{}", u16::from_le_bytes([sample[0], sample[1]]))?;
            }
        }
    }

    Ok(())
}

/// Encodes a native PGM image as binary PGM P5.
pub fn encode_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_pgm_image(image)?;

    write_sample_header_with_max_value(writer, "P5", width, height, maxval)?;
    if maxval < 256 {
        writer.write_all(data)?;
    } else {
        write_le_sample_bytes_as_be(writer, data)?;
    }

    Ok(())
}

/// Encodes image views as a binary PGM P5 multi-image stream.
pub fn encode_all<W: Write>(writer: &mut W, images: &[ImageView<'_>]) -> Result<()> {
    let images = images
        .iter()
        .map(|image| gray_image_to_pgm_native(*image))
        .collect::<Result<Vec<_>>>()?;

    encode_all_native(writer, &images)
}

/// Encodes native PGM images as a binary PGM P5 multi-image stream.
pub fn encode_all_native<W: Write>(writer: &mut W, images: &[NetpbmImage]) -> Result<()> {
    for image in images {
        encode_native(writer, image)?;
    }

    Ok(())
}

/// Encodes a native PGM image as ASCII PGM P2.
pub fn encode_ascii_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, maxval, data) = validate_native_pgm_image(image)?;

    write_sample_header_with_max_value(writer, "P2", width, height, maxval)?;

    if maxval < 256 {
        for sample in data {
            writeln!(writer, "{sample}")?;
        }
    } else {
        for sample in data.chunks_exact(2) {
            writeln!(writer, "{}", u16::from_le_bytes([sample[0], sample[1]]))?;
        }
    }

    Ok(())
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

    validate_native_sample_image(
        *width,
        *height,
        *maxval,
        data,
        PixelFormat::Gray8.channels(),
    )?;
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

fn pgm_pixel_format_for_maxval(maxval: u16) -> PixelFormat {
    if maxval < 256 {
        PixelFormat::Gray8
    } else {
        PixelFormat::Gray16
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
    fn decode_all_native_returns_empty_vec_for_empty_input() {
        let images = decode_all_native(&mut Cursor::new([])).unwrap();

        assert!(images.is_empty());
    }

    #[test]
    fn decode_all_native_reads_single_pgm_p5_image() {
        let input = b"P5\n2 1\n15\n\x00\x0f";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![0, 15]
            }]
        );
    }

    #[test]
    fn decode_all_native_reads_concatenated_pgm_p5_images() {
        let input = b"P5\n2 1\n15\n\x00\x0fP5\n1 1\n255\n\x80";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pgm {
                    width: 2,
                    height: 1,
                    maxval: 15,
                    data: vec![0, 15]
                },
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![128]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_preserves_16_bit_samples() {
        let input = b"P5\n1 1\n65535\n\x12\x34P5\n1 1\n256\n\x01\0";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 65535,
                    data: vec![0x34, 0x12]
                },
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 256,
                    data: vec![0x00, 0x01]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_accepts_whitespace_and_comments_between_images() {
        let input = b"P5\n1 1\n255\n\x00\n# next image\nP5\n1 1\n255\n\xff";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![0]
                },
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![255]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_rejects_mixed_magic() {
        let input = b"P5\n1 1\n255\n\0P6\n1 1\n255\n\0\0\0";
        let error = decode_all_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_rejects_broken_second_image() {
        let input = b"P5\n1 1\n255\n\0P5\n2 1\n255\n\x80";
        let error = decode_all_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 2,
                actual: 1
            }
        );
    }

    #[test]
    fn decode_native_reads_pgm_p5_16_bit_samples_as_little_endian() {
        let input = b"P5\n2 1\n65535\n\x12\x34\xff\xff";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0xff, 0xff]
            }
        );
    }

    #[test]
    fn decode_reads_pgm_p5_16_bit_as_gray16_image() {
        let input = b"P5\n2 1\n65535\n\x12\x34\xff\xff";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Gray16);
        assert_eq!(image.data, [0x34, 0x12, 0xff, 0xff]);
    }

    #[test]
    fn decode_normalizes_pgm_p5_max_value_to_gray8() {
        let input = b"P5\n4 1\n15\n\x00\x05\x0a\x0f";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 4);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 85, 170, 255]);
    }

    #[test]
    fn decode_normalizes_pgm_p5_max_value_with_nearest_rounding() {
        let input = b"P5\n1 1\n10\n\x05";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.data, [128]);
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
    fn decode_rejects_short_16_bit_raster_data() {
        let input = b"P5\n1 1\n65535\n\0";
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
    fn encode_native_writes_pgm_p5_16_bit_samples_as_big_endian() {
        let image = NetpbmImage::Pgm {
            width: 2,
            height: 1,
            maxval: 65535,
            data: vec![0x34, 0x12, 0xff, 0xff],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P5\n2 1\n65535\n\x12\x34\xff\xff");
    }

    #[test]
    fn encode_all_native_writes_empty_stream_for_empty_slice() {
        let mut output = Vec::new();

        encode_all_native(&mut output, &[]).unwrap();

        assert!(output.is_empty());
    }

    #[test]
    fn encode_all_writes_empty_stream_for_empty_slice() {
        let mut output = Vec::new();

        encode_all(&mut output, &[]).unwrap();

        assert!(output.is_empty());
    }

    #[test]
    fn encode_all_writes_pgm_p5_multi_image_stream() {
        let first_data = [0, 255];
        let second_data = [0x34, 0x12, 0xff, 0xff];
        let images = [
            ImageView::new(2, 1, PixelFormat::Gray8, 2, &first_data).unwrap(),
            ImageView::new(2, 1, PixelFormat::Gray16, 4, &second_data).unwrap(),
        ];
        let expected = [
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 255,
                data: vec![0, 255],
            },
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0xff, 0xff],
            },
        ];
        let mut output = Vec::new();

        encode_all(&mut output, &images).unwrap();

        assert_eq!(
            decode_all_native(&mut Cursor::new(output)).unwrap(),
            expected
        );
    }

    #[test]
    fn encode_all_rejects_unsupported_pixel_format_without_writing() {
        let data = [0, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let mut output = Vec::new();
        let error = encode_all(&mut output, &[image]).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Rgb8
            }
        );
        assert!(output.is_empty());
    }

    #[test]
    fn encode_all_native_writes_pgm_p5_multi_image_stream() {
        let images = [
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![0, 15],
            },
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0xff, 0xff],
            },
        ];
        let mut output = Vec::new();

        encode_all_native(&mut output, &images).unwrap();

        assert_eq!(decode_all_native(&mut Cursor::new(output)).unwrap(), images);
    }

    #[test]
    fn encode_all_native_rejects_mixed_format() {
        let images = [
            NetpbmImage::Pgm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![0],
            },
            NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![0, 0, 0],
            },
        ];
        let error = encode_all_native(&mut Vec::new(), &images).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn encode_writes_pgm_p5_gray16_image_as_big_endian() {
        let data = [0x34, 0x12, 0xff, 0xff];
        let image = ImageView::new(2, 1, PixelFormat::Gray16, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P5\n2 1\n65535\n\x12\x34\xff\xff");
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
    fn decode_ascii_native_reads_pgm_p2_16_bit_samples_as_little_endian() {
        let input = b"P2\n2 1\n65535\n4660 65535\n";
        let image = decode_ascii_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 65535,
                data: vec![0x34, 0x12, 0xff, 0xff]
            }
        );
    }

    #[test]
    fn decode_ascii_normalizes_pgm_p2_max_value_to_gray8() {
        let input = b"P2\n4 1\n15\n0 5 10 15\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 4);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 85, 170, 255]);
    }

    #[test]
    fn decode_ascii_reads_pgm_p2_16_bit_as_gray16_image() {
        let input = b"P2\n2 1\n65535\n4660 65535\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Gray16);
        assert_eq!(image.data, [0x34, 0x12, 0xff, 0xff]);
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
    fn encode_ascii_native_writes_pgm_p2_16_bit_samples() {
        let image = NetpbmImage::Pgm {
            width: 2,
            height: 1,
            maxval: 65535,
            data: vec![0x34, 0x12, 0xff, 0xff],
        };
        let mut output = Vec::new();

        encode_ascii_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P2\n2 1\n65535\n4660\n65535\n");
    }

    #[test]
    fn encode_ascii_writes_pgm_p2_gray16_image() {
        let data = [0x34, 0x12, 0xff, 0xff];
        let image = ImageView::new(2, 1, PixelFormat::Gray16, 4, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P2\n2 1\n65535\n4660\n65535\n");
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
