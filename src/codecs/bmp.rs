use std::io::{Cursor, Read, Write};

use crate::io::{read_u16_le, read_u32_le, write_u16_le, write_u32_le};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const FILE_HEADER_SIZE: u32 = 14;
const INFO_HEADER_SIZE: u32 = 40;
const PIXEL_OFFSET: u32 = FILE_HEADER_SIZE + INFO_HEADER_SIZE;
const PLANES: u16 = 1;
const BITS_PER_PIXEL: u16 = 24;
const COMPRESSION_BI_RGB: u32 = 0;

/// Decodes an uncompressed 24-bit BMP image.
///
/// This initial implementation supports only bottom-up BMP files with a
/// BITMAPINFOHEADER DIB header.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut cursor = Cursor::new(data.as_slice());
    let mut signature = [0; 2];
    cursor.read_exact(&mut signature)?;

    if signature != *b"BM" {
        return Err(ImageError::UnsupportedFormat);
    }

    let file_size = read_u32_le(&mut cursor)?;
    let _reserved1 = read_u16_le(&mut cursor)?;
    let _reserved2 = read_u16_le(&mut cursor)?;
    let pixel_offset = read_u32_le(&mut cursor)?;

    let dib_header_size = read_u32_le(&mut cursor)?;
    if dib_header_size != INFO_HEADER_SIZE {
        return Err(ImageError::InvalidHeader {
            reason: "only BITMAPINFOHEADER is supported",
        });
    }

    let width = read_i32_le(&mut cursor)?;
    let height = read_i32_le(&mut cursor)?;
    let planes = read_u16_le(&mut cursor)?;
    let bits_per_pixel = read_u16_le(&mut cursor)?;
    let compression = read_u32_le(&mut cursor)?;
    let _image_size = read_u32_le(&mut cursor)?;
    let _x_pixels_per_meter = read_i32_le(&mut cursor)?;
    let _y_pixels_per_meter = read_i32_le(&mut cursor)?;
    let _colors_used = read_u32_le(&mut cursor)?;
    let _important_colors = read_u32_le(&mut cursor)?;

    if file_size as usize > data.len() {
        return Err(ImageError::InvalidHeader {
            reason: "file size is larger than input",
        });
    }

    if pixel_offset < PIXEL_OFFSET || pixel_offset as usize > data.len() {
        return Err(ImageError::InvalidHeader {
            reason: "invalid pixel data offset",
        });
    }

    if width <= 0 || height == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "width and height must be non-zero, and width must be positive",
        });
    }

    if height < 0 {
        return Err(ImageError::InvalidHeader {
            reason: "top-down BMP is not supported",
        });
    }

    if planes != PLANES {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP planes value",
        });
    }

    if bits_per_pixel != BITS_PER_PIXEL {
        return Err(ImageError::InvalidHeader {
            reason: "only 24-bit BMP is supported",
        });
    }

    if compression != COMPRESSION_BI_RGB {
        return Err(ImageError::InvalidHeader {
            reason: "only uncompressed BMP is supported",
        });
    }

    let width = width as u32;
    let height = height as u32;
    let row_size = bmp_row_size(width)?;
    let pixel_data_len =
        row_size
            .checked_mul(height as usize)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width,
                height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;
    let pixel_start = pixel_offset as usize;
    let pixel_end =
        pixel_start
            .checked_add(pixel_data_len)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width,
                height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;

    if pixel_end > data.len() {
        return Err(ImageError::InvalidBufferLength {
            expected: pixel_end,
            actual: data.len(),
        });
    }

    let mut pixels =
        Vec::with_capacity(width as usize * height as usize * PixelFormat::Rgb8.bytes_per_pixel());
    let row_len = width as usize * PixelFormat::Rgb8.bytes_per_pixel();

    for output_row in 0..height as usize {
        let bmp_row = height as usize - 1 - output_row;
        let row_start = pixel_start + bmp_row * row_size;
        let row = &data[row_start..row_start + row_len];

        for pixel in row.chunks_exact(3) {
            pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
        }
    }

    Image::new(width, height, PixelFormat::Rgb8, pixels)
}

/// Encodes an image view as an uncompressed 24-bit bottom-up BMP image.
pub fn encode<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    if image.pixel_format != PixelFormat::Rgb8 {
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

    let row_size = bmp_row_size(image.width)?;
    let image_size =
        row_size
            .checked_mul(image.height as usize)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width: image.width,
                height: image.height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;
    let image_size_u32 =
        u32::try_from(image_size).map_err(|_| ImageError::ImageDimensionsTooLarge {
            width: image.width,
            height: image.height,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;
    let file_size =
        PIXEL_OFFSET
            .checked_add(image_size_u32)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width: image.width,
                height: image.height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;

    writer.write_all(b"BM")?;
    write_u32_le(writer, file_size)?;
    write_u16_le(writer, 0)?;
    write_u16_le(writer, 0)?;
    write_u32_le(writer, PIXEL_OFFSET)?;

    write_u32_le(writer, INFO_HEADER_SIZE)?;
    write_u32_le(writer, image.width)?;
    write_u32_le(writer, image.height)?;
    write_u16_le(writer, PLANES)?;
    write_u16_le(writer, BITS_PER_PIXEL)?;
    write_u32_le(writer, COMPRESSION_BI_RGB)?;
    write_u32_le(writer, image_size_u32)?;
    write_u32_le(writer, 0)?;
    write_u32_le(writer, 0)?;
    write_u32_le(writer, 0)?;
    write_u32_le(writer, 0)?;

    let row_len = image.width as usize * PixelFormat::Rgb8.bytes_per_pixel();
    let padding_len = row_size - row_len;
    let padding = [0; 3];

    for output_row in (0..image.height as usize).rev() {
        let row_start = output_row * image.stride;
        let row = &image.data[row_start..row_start + row_len];

        for pixel in row.chunks_exact(3) {
            writer.write_all(&[pixel[2], pixel[1], pixel[0]])?;
        }

        writer.write_all(&padding[..padding_len])?;
    }

    Ok(())
}

fn read_i32_le<R: Read>(reader: &mut R) -> std::io::Result<i32> {
    Ok(read_u32_le(reader)? as i32)
}

fn bmp_row_size(width: u32) -> Result<usize> {
    let row_len = (width as usize)
        .checked_mul(PixelFormat::Rgb8.bytes_per_pixel())
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;

    row_len
        .checked_add(3)
        .map(|len| len / 4 * 4)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn decode_reads_24_bit_bottom_up_bmp_image() {
        let input = two_by_two_bmp();
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(
            image.data,
            [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255,]
        );
    }

    #[test]
    fn decode_rejects_non_bmp_magic() {
        let mut input = two_by_two_bmp();
        input[0] = b'Z';
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_unsupported_dib_header_size() {
        let mut input = two_by_two_bmp();
        input[14] = 12;
        input[15] = 0;
        input[16] = 0;
        input[17] = 0;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "only BITMAPINFOHEADER is supported"
            }
        );
    }

    #[test]
    fn decode_rejects_unsupported_bits_per_pixel() {
        let mut input = two_by_two_bmp();
        input[28] = 32;
        input[29] = 0;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "only 24-bit BMP is supported"
            }
        );
    }

    #[test]
    fn decode_rejects_unsupported_compression() {
        let mut input = two_by_two_bmp();
        input[30] = 1;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "only uncompressed BMP is supported"
            }
        );
    }

    #[test]
    fn decode_rejects_top_down_bmp() {
        let mut input = two_by_two_bmp();
        input[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "top-down BMP is not supported"
            }
        );
    }

    #[test]
    fn decode_rejects_short_pixel_data() {
        let mut input = two_by_two_bmp();
        input.truncate(input.len() - 1);
        let file_size = input.len() as u32;
        input[2..6].copy_from_slice(&file_size.to_le_bytes());
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 70,
                actual: 69
            }
        );
    }

    #[test]
    fn encode_writes_24_bit_bottom_up_bmp_image() {
        let data = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let image = ImageView::new(2, 2, PixelFormat::Rgb8, 6, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();

        assert_eq!(output, two_by_two_bmp());
    }

    #[test]
    fn encode_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 0, 0, 99, 0, 255, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Rgb8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.data, [255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn encode_rejects_non_rgb8_pixel_format() {
        let data = [0];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let error = encode(&mut Vec::new(), image).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Gray8
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
    fn integration_decode_encode_roundtrip_preserves_pixels() {
        let input = two_by_two_bmp();
        let image = decode(&mut Cursor::new(input)).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image.as_view()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }

    fn two_by_two_bmp() -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(b"BM");
        data.extend_from_slice(&70_u32.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&54_u32.to_le_bytes());

        data.extend_from_slice(&40_u32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&24_u16.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&16_u32.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());

        data.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);
        data.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);

        data
    }
}
