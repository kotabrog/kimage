use std::io::{Read, Write};

use crate::codecs::netpbm::{
    Dimensions, HeaderParser, read_bitmap_header, reject_trailing_tokens, validate_image_view,
    write_bitmap_header,
};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const ASCII_MAGIC: &[u8] = b"P1";
const BINARY_MAGIC: &[u8] = b"P4";
const WHITE: u8 = 255;
const BLACK: u8 = 0;

/// Decodes an ASCII PBM P1 image.
///
/// PBM samples are expanded to `PixelFormat::Gray8`, where `0` is black and
/// `255` is white.
pub fn decode_ascii<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let dimensions = read_bitmap_header(&mut parser, ASCII_MAGIC)?;

    let expected = dimensions.width as usize * dimensions.height as usize;
    let mut pixels = Vec::with_capacity(expected);

    for _ in 0..expected {
        let sample = parser.next_u32("bitmap sample")?;
        pixels.push(pbm_sample_to_gray8(sample)?);
    }

    reject_trailing_tokens(&mut parser)?;

    Image::new(
        dimensions.width,
        dimensions.height,
        PixelFormat::Gray8,
        pixels,
    )
}

/// Decodes a binary PBM P4 image.
///
/// PBM bits are expanded to `PixelFormat::Gray8`, where `0` is black and `255`
/// is white.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut parser = HeaderParser::new(&data);
    let dimensions = read_bitmap_header(&mut parser, BINARY_MAGIC)?;
    parser.consume_raster_separator()?;

    let row_bytes = pbm_row_bytes(dimensions)?;
    let raster = pbm_raster_slice(&data, &parser, dimensions, row_bytes)?;
    let mut pixels = Vec::with_capacity(dimensions.width as usize * dimensions.height as usize);

    for row in 0..dimensions.height as usize {
        let row_start = row * row_bytes;
        let row_data = &raster[row_start..row_start + row_bytes];

        for x in 0..dimensions.width as usize {
            let byte = row_data[x / 8];
            let bit = (byte >> (7 - (x % 8))) & 1;
            pixels.push(pbm_bit_to_gray8(bit));
        }
    }

    Image::new(
        dimensions.width,
        dimensions.height,
        PixelFormat::Gray8,
        pixels,
    )
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

fn pbm_sample_to_gray8(sample: u32) -> Result<u8> {
    match sample {
        0 => Ok(WHITE),
        1 => Ok(BLACK),
        _ => Err(ImageError::InvalidData {
            reason: "invalid PBM sample",
        }),
    }
}

fn pbm_bit_to_gray8(bit: u8) -> u8 {
    if bit == 0 { WHITE } else { BLACK }
}

fn gray8_to_pbm_sample(sample: u8) -> u32 {
    if sample < 128 { 1 } else { 0 }
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
