use std::io::{Read, Write};

use crate::codecs::netpbm::{
    Dimensions, HeaderParser, NetpbmImage, gray_image_to_pbm_native, read_bitmap_header,
    reject_trailing_tokens, validate_image_view, write_bitmap_header,
    write_bitmap_header_dimensions,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const ASCII_MAGIC: &[u8] = b"P1";
const BINARY_MAGIC: &[u8] = b"P4";
/// Decodes an ASCII PBM P1 image.
///
/// PBM samples are expanded to `PixelFormat::Gray8`, where `0` is black and
/// `255` is white.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_ascii_native(reader)?)
}

/// Decodes an ASCII PBM P1 image without normalizing PBM sample values.
pub fn decode_ascii_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let dimensions = read_bitmap_header(&mut parser, ASCII_MAGIC)?;

    let expected = dimensions.width as usize * dimensions.height as usize;
    let mut pixels = Vec::with_capacity(expected);

    for _ in 0..expected {
        let sample = parser.next_u32("bitmap sample")?;
        pixels.push(validate_pbm_sample(sample)?);
    }

    reject_trailing_tokens(&mut parser)?;

    Ok(NetpbmImage::Pbm {
        width: dimensions.width,
        height: dimensions.height,
        data: pixels,
    })
}

/// Decodes a binary PBM P4 image.
///
/// PBM bits are expanded to `PixelFormat::Gray8`, where `0` is black and `255`
/// is white.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_native(reader)?)
}

/// Decodes a binary PBM P4 image without normalizing PBM bit values.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    decode_one_native(&data, &mut parser)
}

/// Decodes all binary PBM P4 images from a multi-image stream.
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

/// Decodes all binary PBM P4 images from a multi-image stream.
///
/// PBM bits are expanded to `PixelFormat::Gray8`, where `0` is black and `255`
/// is white.
pub fn decode_all<R: Read>(reader: &mut R) -> Result<Vec<Image>> {
    decode_all_native(reader)?
        .into_iter()
        .map(Image::try_from)
        .collect()
}

fn decode_one_native(data: &[u8], parser: &mut HeaderParser<'_>) -> Result<NetpbmImage> {
    let dimensions = read_bitmap_header(parser, BINARY_MAGIC)?;
    parser.consume_raster_separator()?;

    let row_bytes = pbm_row_bytes(dimensions)?;
    let raster = pbm_raster_slice(data, parser, dimensions, row_bytes)?;
    let next_position = parser.position() + raster.len();
    let mut pixels = Vec::with_capacity(dimensions.width as usize * dimensions.height as usize);

    for row in 0..dimensions.height as usize {
        let row_start = row * row_bytes;
        let row_data = &raster[row_start..row_start + row_bytes];

        for x in 0..dimensions.width as usize {
            let byte = row_data[x / 8];
            let bit = (byte >> (7 - (x % 8))) & 1;
            pixels.push(bit);
        }
    }

    parser.set_position(next_position);

    Ok(NetpbmImage::Pbm {
        width: dimensions.width,
        height: dimensions.height,
        data: pixels,
    })
}

/// Encodes an image view as ASCII PBM P1.
///
/// `PixelFormat::Gray8` values are thresholded: values below 128 become black,
/// and values 128 or above become white.
pub fn encode_ascii<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Gray8)?;

    write_bitmap_header(writer, "P1", image)?;

    let row_len = image.width as usize;

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;

        for (index, sample) in image.data[start..end].iter().enumerate() {
            if index > 0 {
                write!(writer, " ")?;
            }
            write!(writer, "{}", gray8_to_pbm_sample(*sample))?;
        }

        writeln!(writer)?;
    }

    Ok(())
}

/// Encodes a native PBM image as ASCII PBM P1.
pub fn encode_ascii_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, data) = validate_native_pbm_image(image)?;

    write_bitmap_header_dimensions(writer, "P1", width, height)?;

    for row in 0..height as usize {
        let start = row * width as usize;
        let end = start + width as usize;

        for (index, sample) in data[start..end].iter().enumerate() {
            if index > 0 {
                write!(writer, " ")?;
            }
            write!(writer, "{sample}")?;
        }

        writeln!(writer)?;
    }

    Ok(())
}

/// Encodes an image view as binary PBM P4.
///
/// `PixelFormat::Gray8` values are thresholded: values below 128 become black,
/// and values 128 or above become white.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let image = validate_image_view(image, PixelFormat::Gray8)?;

    write_bitmap_header(writer, "P4", image)?;

    let row_len = image.width as usize;
    let row_bytes = pbm_row_bytes(Dimensions {
        width: image.width,
        height: image.height,
    })?;

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        let row_data = &image.data[start..end];

        for byte_index in 0..row_bytes {
            let mut packed = 0;

            for bit_index in 0..8 {
                let x = byte_index * 8 + bit_index;
                if x >= row_len {
                    break;
                }

                let bit = gray8_to_pbm_sample(row_data[x]) as u8;
                packed |= bit << (7 - bit_index);
            }

            writer.write_all(&[packed])?;
        }
    }

    Ok(())
}

/// Encodes a native PBM image as binary PBM P4.
pub fn encode_native<W: Write>(writer: &mut W, image: &NetpbmImage) -> Result<()> {
    let (width, height, data) = validate_native_pbm_image(image)?;

    write_bitmap_header_dimensions(writer, "P4", width, height)?;

    let row_len = width as usize;
    let row_bytes = pbm_row_bytes(Dimensions { width, height })?;

    for row in 0..height as usize {
        let start = row * row_len;
        let row_data = &data[start..start + row_len];

        for byte_index in 0..row_bytes {
            let mut packed = 0;

            for bit_index in 0..8 {
                let x = byte_index * 8 + bit_index;
                if x >= row_len {
                    break;
                }

                packed |= row_data[x] << (7 - bit_index);
            }

            writer.write_all(&[packed])?;
        }
    }

    Ok(())
}

/// Encodes image views as a binary PBM P4 multi-image stream.
pub fn encode_all<W: Write>(writer: &mut W, images: &[ImageView<'_>]) -> Result<()> {
    let images = images
        .iter()
        .map(|image| gray_image_to_pbm_native(*image))
        .collect::<Result<Vec<_>>>()?;

    encode_all_native(writer, &images)
}

/// Encodes native PBM images as a binary PBM P4 multi-image stream.
pub fn encode_all_native<W: Write>(writer: &mut W, images: &[NetpbmImage]) -> Result<()> {
    for image in images {
        encode_native(writer, image)?;
    }

    Ok(())
}

fn pbm_row_bytes(dimensions: Dimensions) -> Result<usize> {
    (dimensions.width as usize)
        .checked_add(7)
        .map(|width| width / 8)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width: dimensions.width,
            height: dimensions.height,
            bytes_per_pixel: PixelFormat::Gray8.bytes_per_pixel(),
        })
}

fn pbm_raster_slice<'a>(
    data: &'a [u8],
    parser: &HeaderParser<'_>,
    dimensions: Dimensions,
    row_bytes: usize,
) -> Result<&'a [u8]> {
    let raster_start = parser.position();
    let raster_len = row_bytes.checked_mul(dimensions.height as usize).ok_or(
        ImageError::ImageDimensionsTooLarge {
            width: dimensions.width,
            height: dimensions.height,
            bytes_per_pixel: PixelFormat::Gray8.bytes_per_pixel(),
        },
    )?;
    let raster_end =
        raster_start
            .checked_add(raster_len)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width: dimensions.width,
                height: dimensions.height,
                bytes_per_pixel: PixelFormat::Gray8.bytes_per_pixel(),
            })?;

    if raster_end > data.len() {
        return Err(ImageError::InvalidBufferLength {
            expected: raster_len,
            actual: data.len().saturating_sub(raster_start),
        });
    }

    Ok(&data[raster_start..raster_end])
}

fn validate_pbm_sample(sample: u32) -> Result<u8> {
    match sample {
        0 => Ok(0),
        1 => Ok(1),
        _ => Err(ImageError::InvalidData {
            reason: "invalid PBM sample",
        }),
    }
}

fn gray8_to_pbm_sample(sample: u8) -> u32 {
    if sample < 128 { 1 } else { 0 }
}

fn validate_native_pbm_image(image: &NetpbmImage) -> Result<(u32, u32, &[u8])> {
    let NetpbmImage::Pbm {
        width,
        height,
        data,
    } = image
    else {
        return Err(ImageError::UnsupportedFormat);
    };

    if *width == 0 || *height == 0 {
        return Err(ImageError::InvalidData {
            reason: "width and height must be greater than zero",
        });
    }

    let expected = (*width as usize).checked_mul(*height as usize).ok_or(
        ImageError::ImageDimensionsTooLarge {
            width: *width,
            height: *height,
            bytes_per_pixel: PixelFormat::Gray8.bytes_per_pixel(),
        },
    )?;

    if data.len() != expected {
        return Err(ImageError::InvalidBufferLength {
            expected,
            actual: data.len(),
        });
    }

    if data.iter().any(|sample| *sample > 1) {
        return Err(ImageError::InvalidData {
            reason: "invalid PBM sample",
        });
    }

    Ok((*width, *height, data))
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn decode_ascii_reads_pbm_p1_as_gray8_image() {
        let input = b"P1\n3 2\n0 1 0\n1 0 1\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 3);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [255, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn decode_ascii_native_reads_pbm_p1_samples() {
        let input = b"P1\n3 1\n0 1 0\n";
        let image = decode_ascii_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pbm {
                width: 3,
                height: 1,
                data: vec![0, 1, 0]
            }
        );
    }

    #[test]
    fn decode_ascii_accepts_comments_and_multiple_whitespace() {
        let input = b"P1\r\n# comment\r\n2 1\r\n0\t1\r\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [255, 0]);
    }

    #[test]
    fn decode_ascii_rejects_non_pbm_p1_magic() {
        let input = b"P4\n1 1\n\0";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_ascii_rejects_invalid_bitmap_sample() {
        let input = b"P1\n1 1\n2";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "invalid PBM sample"
            }
        );
    }

    #[test]
    fn decode_ascii_rejects_short_samples() {
        let input = b"P1\n2 1\n0";
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
        let input = b"P1\n1 1\n0 1";
        let error = decode_ascii(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "too many samples"
            }
        );
    }

    #[test]
    fn decode_reads_pbm_p4_as_gray8_image() {
        let input = b"P4\n3 2\n\x40\xa0";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 3);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [255, 0, 255, 0, 255, 0]);
    }

    #[test]
    fn decode_native_reads_pbm_p4_bits() {
        let input = b"P4\n3 2\n\x40\xa0";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pbm {
                width: 3,
                height: 2,
                data: vec![0, 1, 0, 1, 0, 1]
            }
        );
    }

    #[test]
    fn decode_all_native_returns_empty_vec_for_empty_input() {
        let images = decode_all_native(&mut Cursor::new([])).unwrap();

        assert!(images.is_empty());
    }

    #[test]
    fn decode_all_native_reads_single_pbm_p4_image() {
        let input = b"P4\n3 1\n\x40";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [NetpbmImage::Pbm {
                width: 3,
                height: 1,
                data: vec![0, 1, 0]
            }]
        );
    }

    #[test]
    fn decode_all_native_reads_concatenated_pbm_p4_images() {
        let input = b"P4\n3 1\n\x40P4\n2 1\n\xc0";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pbm {
                    width: 3,
                    height: 1,
                    data: vec![0, 1, 0]
                },
                NetpbmImage::Pbm {
                    width: 2,
                    height: 1,
                    data: vec![1, 1]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_accepts_whitespace_and_comments_between_images() {
        let input = b"P4\n1 1\n\0\n# next image\nP4\n1 1\n\x80";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![0]
                },
                NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![1]
                }
            ]
        );
    }

    #[test]
    fn decode_all_reads_normalized_pbm_p4_images() {
        let input = b"P4\n1 1\n\0P4\n1 1\n\x80";
        let images = decode_all(&mut Cursor::new(input)).unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].pixel_format, PixelFormat::Gray8);
        assert_eq!(images[0].data, [255]);
        assert_eq!(images[1].pixel_format, PixelFormat::Gray8);
        assert_eq!(images[1].data, [0]);
    }

    #[test]
    fn decode_all_native_rejects_mixed_magic() {
        let input = b"P4\n1 1\n\0P5\n1 1\n255\n\0";
        let error = decode_all_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_rejects_broken_second_image() {
        let input = b"P4\n1 1\n\0P4\n9 1\n\x80";
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
    fn decode_accepts_crlf_raster_separator() {
        let input = b"P4\r\n1 1\r\n\x80";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.data, [0]);
    }

    #[test]
    fn decode_rejects_non_pbm_p4_magic() {
        let input = b"P1\n1 1\n0";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_short_bitmap_data() {
        let input = b"P4\n9 1\n\x80";
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
    fn encode_ascii_writes_pbm_p1_from_gray8_image() {
        let data = [255, 0, 128, 127];
        let image = ImageView::new(4, 1, PixelFormat::Gray8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P1\n4 1\n0 1 0 1\n");
    }

    #[test]
    fn encode_ascii_native_writes_pbm_p1_samples() {
        let image = NetpbmImage::Pbm {
            width: 4,
            height: 1,
            data: vec![0, 1, 0, 1],
        };
        let mut output = Vec::new();

        encode_ascii_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P1\n4 1\n0 1 0 1\n");
    }

    #[test]
    fn encode_ascii_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 99, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image).unwrap();

        assert_eq!(output, b"P1\n1 2\n0\n1\n");
    }

    #[test]
    fn encode_writes_pbm_p4_from_gray8_image() {
        let data = [255, 0, 255, 0, 255, 0];
        let image = ImageView::new(3, 2, PixelFormat::Gray8, 3, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P4\n3 2\n\x40\xa0");
    }

    #[test]
    fn encode_native_writes_pbm_p4_bits() {
        let image = NetpbmImage::Pbm {
            width: 3,
            height: 2,
            data: vec![0, 1, 0, 1, 0, 1],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, b"P4\n3 2\n\x40\xa0");
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
    fn encode_all_writes_pbm_p4_multi_image_stream() {
        let first_data = [255, 0, 255];
        let second_data = [0, 0];
        let images = [
            ImageView::new(3, 1, PixelFormat::Gray8, 3, &first_data).unwrap(),
            ImageView::new(2, 1, PixelFormat::Gray8, 2, &second_data).unwrap(),
        ];
        let expected = [
            NetpbmImage::Pbm {
                width: 3,
                height: 1,
                data: vec![0, 1, 0],
            },
            NetpbmImage::Pbm {
                width: 2,
                height: 1,
                data: vec![1, 1],
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
    fn encode_all_rejects_invalid_sample_without_writing() {
        let data = [128];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let mut output = Vec::new();
        let error = encode_all(&mut output, &[image]).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "PBM conversion requires Gray8 samples to be 0 or 255"
            }
        );
        assert!(output.is_empty());
    }

    #[test]
    fn encode_all_native_writes_pbm_p4_multi_image_stream() {
        let images = [
            NetpbmImage::Pbm {
                width: 3,
                height: 1,
                data: vec![0, 1, 0],
            },
            NetpbmImage::Pbm {
                width: 2,
                height: 1,
                data: vec![1, 1],
            },
        ];
        let mut output = Vec::new();

        encode_all_native(&mut output, &images).unwrap();

        assert_eq!(decode_all_native(&mut Cursor::new(output)).unwrap(), images);
    }

    #[test]
    fn encode_all_native_rejects_mixed_format() {
        let images = [
            NetpbmImage::Pbm {
                width: 1,
                height: 1,
                data: vec![0],
            },
            NetpbmImage::Pgm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![0],
            },
        ];
        let error = encode_all_native(&mut Vec::new(), &images).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn encode_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 99, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, b"P4\n1 2\n\0\x80");
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
    fn integration_decode_encode_ascii_roundtrip_preserves_thresholded_pixels() {
        let input = b"P1\n4 1\n0 1 0 1\n";
        let image = decode_ascii(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode_ascii(&mut output, image.as_view()).unwrap();
        let decoded = decode_ascii(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }

    #[test]
    fn integration_decode_encode_roundtrip_preserves_thresholded_pixels() {
        let input = b"P4\n3 2\n\x40\xa0";
        let image = decode(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image.as_view()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }
}
