use std::io::{Cursor, Read, Write};

use crate::io::{read_u16_le, read_u32_le, write_u16_le, write_u32_le};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const FILE_HEADER_SIZE: u32 = 14;
const INFO_HEADER_SIZE: u32 = 40;
const PIXEL_OFFSET: u32 = FILE_HEADER_SIZE + INFO_HEADER_SIZE;
const PLANES: u16 = 1;
const BITS_PER_PIXEL_RGB24: u16 = 24;
const COMPRESSION_BI_RGB: u32 = 0;

/// Output options used when encoding a generic image view as BMP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmpEncodeOptions {
    pub pixel_encoding: BmpPixelEncoding,
    pub orientation: BmpOrientation,
    pub x_pixels_per_meter: i32,
    pub y_pixels_per_meter: i32,
}

impl BmpEncodeOptions {
    pub const fn new() -> Self {
        Self {
            pixel_encoding: BmpPixelEncoding::Rgb24,
            orientation: BmpOrientation::BottomUp,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
        }
    }

    pub const fn with_orientation(mut self, orientation: BmpOrientation) -> Self {
        self.orientation = orientation;
        self
    }

    pub const fn with_resolution(
        mut self,
        x_pixels_per_meter: i32,
        y_pixels_per_meter: i32,
    ) -> Self {
        self.x_pixels_per_meter = x_pixels_per_meter;
        self.y_pixels_per_meter = y_pixels_per_meter;
        self
    }
}

impl Default for BmpEncodeOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixel array representation used when encoding BMP.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmpPixelEncoding {
    Rgb24,
}

/// BMP row order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BmpOrientation {
    BottomUp,
    TopDown,
}

/// Native BMP image representation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmpImage {
    pub file_header: BmpFileHeader,
    pub dib_header: BmpDibHeader,
    pub color_masks: Vec<u32>,
    pub color_table: Vec<BmpColorTableEntry>,
    pub pixel_array: Vec<u8>,
}

/// BMP file header fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmpFileHeader {
    pub file_size: u32,
    pub reserved1: u16,
    pub reserved2: u16,
    pub pixel_offset: u32,
}

/// Supported BMP DIB headers.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmpDibHeader {
    BitmapInfoHeader(BmpInfoHeader),
}

/// BITMAPINFOHEADER fields.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BmpInfoHeader {
    pub width: i32,
    pub height: i32,
    pub planes: u16,
    pub bits_per_pixel: u16,
    pub compression: u32,
    pub image_size: u32,
    pub x_pixels_per_meter: i32,
    pub y_pixels_per_meter: i32,
    pub colors_used: u32,
    pub important_colors: u32,
}

/// BMP color table entry.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BmpColorTableEntry {
    pub blue: u8,
    pub green: u8,
    pub red: u8,
    pub reserved: u8,
}

/// Decodes an uncompressed 24-bit BMP image.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    decode_native(reader)?.to_image()
}

/// Decodes a BMP image while preserving native BMP fields.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<BmpImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut cursor = Cursor::new(data.as_slice());
    let mut signature = [0; 2];
    cursor.read_exact(&mut signature)?;

    if signature != *b"BM" {
        return Err(ImageError::UnsupportedFormat);
    }

    let file_header = BmpFileHeader {
        file_size: read_u32_le(&mut cursor)?,
        reserved1: read_u16_le(&mut cursor)?,
        reserved2: read_u16_le(&mut cursor)?,
        pixel_offset: read_u32_le(&mut cursor)?,
    };

    let dib_header_size = read_u32_le(&mut cursor)?;
    if dib_header_size != INFO_HEADER_SIZE {
        return Err(ImageError::UnsupportedFormat);
    }

    let info_header = BmpInfoHeader {
        width: read_i32_le(&mut cursor)?,
        height: read_i32_le(&mut cursor)?,
        planes: read_u16_le(&mut cursor)?,
        bits_per_pixel: read_u16_le(&mut cursor)?,
        compression: read_u32_le(&mut cursor)?,
        image_size: read_u32_le(&mut cursor)?,
        x_pixels_per_meter: read_i32_le(&mut cursor)?,
        y_pixels_per_meter: read_i32_le(&mut cursor)?,
        colors_used: read_u32_le(&mut cursor)?,
        important_colors: read_u32_le(&mut cursor)?,
    };

    validate_supported_info_header(&info_header)?;

    let width = info_header.width as u32;
    let height = info_header.height.unsigned_abs();
    let pixel_offset = validate_pixel_offset(file_header.pixel_offset, data.len())?;
    let pixel_data_len = bmp_pixel_array_len(width, height)?;
    let pixel_end =
        pixel_offset
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

    Ok(BmpImage {
        file_header,
        dib_header: BmpDibHeader::BitmapInfoHeader(info_header),
        color_masks: Vec::new(),
        color_table: Vec::new(),
        pixel_array: data[pixel_offset..pixel_end].to_vec(),
    })
}

/// Encodes an image view as a BMP image using the selected options.
pub fn encode<W: Write>(
    writer: &mut W,
    image: ImageView<'_>,
    options: BmpEncodeOptions,
) -> Result<()> {
    let image = image_view_to_bmp_native(image, options)?;
    encode_native(writer, &image)
}

/// Encodes a native BMP image.
pub fn encode_native<W: Write>(writer: &mut W, image: &BmpImage) -> Result<()> {
    validate_bmp_image_pixels(image)?;

    let BmpDibHeader::BitmapInfoHeader(info_header) = &image.dib_header;

    writer.write_all(b"BM")?;
    write_u32_le(writer, image.file_header.file_size)?;
    write_u16_le(writer, image.file_header.reserved1)?;
    write_u16_le(writer, image.file_header.reserved2)?;
    write_u32_le(writer, image.file_header.pixel_offset)?;

    write_u32_le(writer, INFO_HEADER_SIZE)?;
    write_i32_le(writer, info_header.width)?;
    write_i32_le(writer, info_header.height)?;
    write_u16_le(writer, info_header.planes)?;
    write_u16_le(writer, info_header.bits_per_pixel)?;
    write_u32_le(writer, info_header.compression)?;
    write_u32_le(writer, info_header.image_size)?;
    write_i32_le(writer, info_header.x_pixels_per_meter)?;
    write_i32_le(writer, info_header.y_pixels_per_meter)?;
    write_u32_le(writer, info_header.colors_used)?;
    write_u32_le(writer, info_header.important_colors)?;
    writer.write_all(&image.pixel_array)?;

    Ok(())
}

impl BmpImage {
    /// Converts this native BMP image into the generic normalized image buffer.
    pub fn to_image(&self) -> Result<Image> {
        validate_bmp_image_pixels(self)?;

        let BmpDibHeader::BitmapInfoHeader(info_header) = &self.dib_header;
        let width = info_header.width as u32;
        let height = info_header.height.unsigned_abs();
        let row_size = bmp_row_size(width)?;
        let row_len = rgb24_row_len(width)?;
        let mut pixels = Vec::with_capacity(
            (width as usize)
                .checked_mul(height as usize)
                .and_then(|pixels| pixels.checked_mul(PixelFormat::Rgb8.bytes_per_pixel()))
                .ok_or(ImageError::ImageDimensionsTooLarge {
                    width,
                    height,
                    bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
                })?,
        );

        for output_row in 0..height as usize {
            let bmp_row = match bmp_orientation(info_header.height)? {
                BmpOrientation::BottomUp => height as usize - 1 - output_row,
                BmpOrientation::TopDown => output_row,
            };
            let row_start = bmp_row * row_size;
            let row = &self.pixel_array[row_start..row_start + row_len];

            for pixel in row.chunks_exact(3) {
                pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
            }
        }

        Image::new(width, height, PixelFormat::Rgb8, pixels)
    }

    /// Validates whether the native fields describe a consistent BMP file layout.
    pub fn validate_file_layout(&self) -> Result<()> {
        validate_bmp_image_pixels(self)?;

        let BmpDibHeader::BitmapInfoHeader(info_header) = &self.dib_header;
        let width = info_header.width as u32;
        let height = info_header.height.unsigned_abs();
        let image_size = bmp_pixel_array_len(width, height)?;
        let image_size_u32 =
            u32::try_from(image_size).map_err(|_| ImageError::ImageDimensionsTooLarge {
                width,
                height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;
        let expected_file_size = self
            .file_header
            .pixel_offset
            .checked_add(image_size_u32)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width,
                height,
                bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
            })?;

        let expected_pixel_offset = expected_min_pixel_offset(self)?;
        if self.file_header.pixel_offset != expected_pixel_offset {
            return Err(ImageError::InvalidHeader {
                reason: "invalid pixel data offset",
            });
        }

        if self.file_header.file_size != expected_file_size {
            return Err(ImageError::InvalidHeader {
                reason: "invalid BMP file size",
            });
        }

        if info_header.image_size != 0 && info_header.image_size != image_size_u32 {
            return Err(ImageError::InvalidHeader {
                reason: "invalid BMP image size",
            });
        }

        Ok(())
    }
}

/// Converts an image view into a native BMP image.
pub fn image_view_to_bmp_native(
    image: ImageView<'_>,
    options: BmpEncodeOptions,
) -> Result<BmpImage> {
    match options.pixel_encoding {
        BmpPixelEncoding::Rgb24 => image_view_to_rgb24_bmp_native(image, options),
    }
}

fn image_view_to_rgb24_bmp_native(
    image: ImageView<'_>,
    options: BmpEncodeOptions,
) -> Result<BmpImage> {
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
    let width_i32 =
        i32::try_from(image.width).map_err(|_| ImageError::ImageDimensionsTooLarge {
            width: image.width,
            height: image.height,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;
    let height_i32 =
        i32::try_from(image.height).map_err(|_| ImageError::ImageDimensionsTooLarge {
            width: image.width,
            height: image.height,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;

    let row_size = bmp_row_size(image.width)?;
    let image_size = bmp_pixel_array_len(image.width, image.height)?;
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
    let row_len = rgb24_row_len(image.width)?;
    let padding_len = row_size - row_len;
    let padding = [0; 3];
    let mut pixel_array = Vec::with_capacity(image_size);

    match options.orientation {
        BmpOrientation::BottomUp => {
            for output_row in (0..image.height as usize).rev() {
                write_rgb24_bmp_row(&mut pixel_array, image, output_row, row_len)?;
                pixel_array.extend_from_slice(&padding[..padding_len]);
            }
        }
        BmpOrientation::TopDown => {
            for output_row in 0..image.height as usize {
                write_rgb24_bmp_row(&mut pixel_array, image, output_row, row_len)?;
                pixel_array.extend_from_slice(&padding[..padding_len]);
            }
        }
    }

    Ok(BmpImage {
        file_header: BmpFileHeader {
            file_size,
            reserved1: 0,
            reserved2: 0,
            pixel_offset: PIXEL_OFFSET,
        },
        dib_header: BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
            width: width_i32,
            height: match options.orientation {
                BmpOrientation::BottomUp => height_i32,
                BmpOrientation::TopDown => -height_i32,
            },
            planes: PLANES,
            bits_per_pixel: BITS_PER_PIXEL_RGB24,
            compression: COMPRESSION_BI_RGB,
            image_size: image_size_u32,
            x_pixels_per_meter: options.x_pixels_per_meter,
            y_pixels_per_meter: options.y_pixels_per_meter,
            colors_used: 0,
            important_colors: 0,
        }),
        color_masks: Vec::new(),
        color_table: Vec::new(),
        pixel_array,
    })
}

fn write_rgb24_bmp_row<W: Write>(
    writer: &mut W,
    image: ImageView<'_>,
    row_index: usize,
    row_len: usize,
) -> Result<()> {
    let row_start = row_index * image.stride;
    let row = &image.data[row_start..row_start + row_len];

    for pixel in row.chunks_exact(3) {
        writer.write_all(&[pixel[2], pixel[1], pixel[0]])?;
    }

    Ok(())
}

fn validate_bmp_image_pixels(image: &BmpImage) -> Result<()> {
    if !image.color_masks.is_empty() || !image.color_table.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    let BmpDibHeader::BitmapInfoHeader(info_header) = &image.dib_header;
    validate_supported_info_header(info_header)?;

    let width = info_header.width as u32;
    let height = info_header.height.unsigned_abs();
    let image_size = bmp_pixel_array_len(width, height)?;

    if image.pixel_array.len() != image_size {
        return Err(ImageError::InvalidBufferLength {
            expected: image_size,
            actual: image.pixel_array.len(),
        });
    }

    Ok(())
}

fn expected_min_pixel_offset(image: &BmpImage) -> Result<u32> {
    let BmpDibHeader::BitmapInfoHeader(_) = &image.dib_header;

    if !image.color_masks.is_empty() || !image.color_table.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    Ok(PIXEL_OFFSET)
}

fn validate_supported_info_header(info_header: &BmpInfoHeader) -> Result<()> {
    if info_header.width <= 0 || info_header.height == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "width and height must be non-zero, and width must be positive",
        });
    }

    if info_header.planes != PLANES {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP planes value",
        });
    }

    if info_header.bits_per_pixel != BITS_PER_PIXEL_RGB24 {
        return Err(ImageError::UnsupportedFormat);
    }

    if info_header.compression != COMPRESSION_BI_RGB {
        return Err(ImageError::UnsupportedFormat);
    }

    Ok(())
}

fn validate_pixel_offset(pixel_offset: u32, input_len: usize) -> Result<usize> {
    if pixel_offset < PIXEL_OFFSET || pixel_offset as usize > input_len {
        return Err(ImageError::InvalidHeader {
            reason: "invalid pixel data offset",
        });
    }

    Ok(pixel_offset as usize)
}

fn bmp_orientation(height: i32) -> Result<BmpOrientation> {
    if height > 0 {
        Ok(BmpOrientation::BottomUp)
    } else if height < 0 {
        Ok(BmpOrientation::TopDown)
    } else {
        Err(ImageError::InvalidHeader {
            reason: "width and height must be non-zero, and width must be positive",
        })
    }
}

fn read_i32_le<R: Read>(reader: &mut R) -> std::io::Result<i32> {
    Ok(read_u32_le(reader)? as i32)
}

fn write_i32_le<W: Write>(writer: &mut W, value: i32) -> std::io::Result<()> {
    write_u32_le(writer, value as u32)
}

fn rgb24_row_len(width: u32) -> Result<usize> {
    (width as usize)
        .checked_mul(PixelFormat::Rgb8.bytes_per_pixel())
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })
}

fn bmp_row_size(width: u32) -> Result<usize> {
    let row_len = rgb24_row_len(width)?;

    row_len
        .checked_add(3)
        .map(|len| len / 4 * 4)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })
}

fn bmp_pixel_array_len(width: u32, height: u32) -> Result<usize> {
    bmp_row_size(width)?
        .checked_mul(height as usize)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
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
    fn decode_reads_24_bit_top_down_bmp_image() {
        let input = two_by_two_top_down_bmp();
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image.data,
            [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255,]
        );
    }

    #[test]
    fn decode_native_preserves_bmp_fields() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();

        assert_eq!(
            image.file_header,
            BmpFileHeader {
                file_size: 70,
                reserved1: 0,
                reserved2: 0,
                pixel_offset: 54,
            }
        );
        assert_eq!(
            image.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 2,
                height: 2,
                planes: 1,
                bits_per_pixel: 24,
                compression: 0,
                image_size: 16,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 0,
                important_colors: 0,
            })
        );
        assert_eq!(image.pixel_array.len(), 16);
    }

    #[test]
    fn validate_file_layout_accepts_consistent_bmp_image() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();

        image.validate_file_layout().unwrap();
    }

    #[test]
    fn validate_file_layout_rejects_pixel_offset_after_unknown_gap() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp_with_pixel_gap())).unwrap();
        let error = image.validate_file_layout().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid pixel data offset"
            }
        );
    }

    #[test]
    fn validate_file_layout_rejects_inconsistent_file_size() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        image.file_header.file_size += 1;
        let error = image.validate_file_layout().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP file size"
            }
        );
    }

    #[test]
    fn validate_file_layout_rejects_inconsistent_nonzero_image_size() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let BmpDibHeader::BitmapInfoHeader(info_header) = &mut image.dib_header;
        info_header.image_size += 1;
        let error = image.validate_file_layout().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP image size"
            }
        );
    }

    #[test]
    fn validate_file_layout_allows_zero_image_size_for_bi_rgb() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let BmpDibHeader::BitmapInfoHeader(info_header) = &mut image.dib_header;
        info_header.image_size = 0;

        image.validate_file_layout().unwrap();
    }

    #[test]
    fn decode_allows_file_size_larger_than_input_when_pixels_are_present() {
        let mut input = two_by_two_bmp();
        input[2..6].copy_from_slice(&100_u32.to_le_bytes());

        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
    }

    #[test]
    fn decode_uses_pixel_offset_after_unknown_gap() {
        let input = two_by_two_bmp_with_pixel_gap();
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
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

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_unsupported_bits_per_pixel() {
        let mut input = two_by_two_bmp();
        input[28] = 32;
        input[29] = 0;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_unsupported_compression() {
        let mut input = two_by_two_bmp();
        input[30] = 1;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
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

        encode(&mut output, image, BmpEncodeOptions::default()).unwrap();

        assert_eq!(output, two_by_two_bmp());
    }

    #[test]
    fn encode_writes_24_bit_top_down_bmp_image() {
        let data = [255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 255];
        let image = ImageView::new(2, 2, PixelFormat::Rgb8, 6, &data).unwrap();
        let mut output = Vec::new();

        encode(
            &mut output,
            image,
            BmpEncodeOptions::new().with_orientation(BmpOrientation::TopDown),
        )
        .unwrap();

        assert_eq!(output, two_by_two_top_down_bmp());
    }

    #[test]
    fn encode_writes_resolution_metadata() {
        let data = [255, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let mut output = Vec::new();

        encode(
            &mut output,
            image,
            BmpEncodeOptions::new().with_resolution(100, 200),
        )
        .unwrap();
        let native = decode_native(&mut Cursor::new(output)).unwrap();

        assert_eq!(
            native.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 1,
                height: 1,
                planes: 1,
                bits_per_pixel: 24,
                compression: 0,
                image_size: 4,
                x_pixels_per_meter: 100,
                y_pixels_per_meter: 200,
                colors_used: 0,
                important_colors: 0,
            })
        );
    }

    #[test]
    fn encode_native_writes_bmp_image() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, two_by_two_bmp());
    }

    #[test]
    fn encode_native_allows_zero_image_size_for_bi_rgb() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let BmpDibHeader::BitmapInfoHeader(info_header) = &mut image.dib_header;
        info_header.image_size = 0;
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(&output[34..38], &0_u32.to_le_bytes());
        assert_eq!(decode(&mut Cursor::new(output)).unwrap().width, 2);
    }

    #[test]
    fn encode_native_preserves_reserved_file_header_fields() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        image.file_header.reserved1 = 1;
        image.file_header.reserved2 = 2;
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(&output[6..8], &1_u16.to_le_bytes());
        assert_eq!(&output[8..10], &2_u16.to_le_bytes());
    }

    #[test]
    fn encode_native_preserves_inconsistent_file_header_fields() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        image.file_header.file_size += 1;
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(&output[2..6], &71_u32.to_le_bytes());
    }

    #[test]
    fn encode_native_preserves_inconsistent_nonzero_image_size() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let BmpDibHeader::BitmapInfoHeader(info_header) = &mut image.dib_header;
        info_header.image_size += 1;
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(&output[34..38], &17_u32.to_le_bytes());
    }

    #[test]
    fn image_view_to_bmp_native_converts_rgb8_image() {
        let data = [255, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();

        let native = image_view_to_bmp_native(image, BmpEncodeOptions::default()).unwrap();

        assert_eq!(native.file_header.file_size, 58);
        assert_eq!(native.file_header.pixel_offset, 54);
        assert_eq!(native.pixel_array, [0, 0, 255, 0]);
    }

    #[test]
    fn encode_writes_only_pixel_bytes_from_strided_rows() {
        let data = [255, 0, 0, 99, 0, 255, 0, 88];
        let image = ImageView::new(1, 2, PixelFormat::Rgb8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, BmpEncodeOptions::default()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded.width, 1);
        assert_eq!(decoded.height, 2);
        assert_eq!(decoded.data, [255, 0, 0, 0, 255, 0]);
    }

    #[test]
    fn encode_rejects_non_rgb8_pixel_format() {
        let data = [0];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let error = encode(&mut Vec::new(), image, BmpEncodeOptions::default()).unwrap_err();

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
        let error = encode(&mut Vec::new(), image, BmpEncodeOptions::default()).unwrap_err();

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

        encode(&mut output, image.as_view(), BmpEncodeOptions::default()).unwrap();
        let decoded = decode(&mut Cursor::new(output)).unwrap();

        assert_eq!(decoded, image);
    }

    fn two_by_two_bmp() -> Vec<u8> {
        two_by_two_bmp_with_height(2)
    }

    fn two_by_two_top_down_bmp() -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(b"BM");
        data.extend_from_slice(&70_u32.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&54_u32.to_le_bytes());

        data.extend_from_slice(&40_u32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&(-2_i32).to_le_bytes());
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&24_u16.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&16_u32.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());

        data.extend_from_slice(&[0, 0, 255, 0, 255, 0, 0, 0]);
        data.extend_from_slice(&[255, 0, 0, 255, 255, 255, 0, 0]);

        data
    }

    fn two_by_two_bmp_with_pixel_gap() -> Vec<u8> {
        let mut data = two_by_two_bmp();
        data[2..6].copy_from_slice(&72_u32.to_le_bytes());
        data[10..14].copy_from_slice(&56_u32.to_le_bytes());
        data.splice(54..54, [0xaa, 0xbb]);
        data
    }

    fn two_by_two_bmp_with_height(height: i32) -> Vec<u8> {
        let mut data = Vec::new();

        data.extend_from_slice(b"BM");
        data.extend_from_slice(&70_u32.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&54_u32.to_le_bytes());

        data.extend_from_slice(&40_u32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&height.to_le_bytes());
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
