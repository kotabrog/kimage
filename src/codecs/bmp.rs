use std::collections::HashMap;
use std::io::{Cursor, Read, Write};

use crate::io::{read_u16_le, read_u32_le, write_u16_le, write_u32_le};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const FILE_HEADER_SIZE: u32 = 14;
const INFO_HEADER_SIZE: u32 = 40;
const PIXEL_OFFSET: u32 = FILE_HEADER_SIZE + INFO_HEADER_SIZE;
const PLANES: u16 = 1;
const BITS_PER_PIXEL_INDEXED1: u16 = 1;
const BITS_PER_PIXEL_INDEXED4: u16 = 4;
const BITS_PER_PIXEL_INDEXED8: u16 = 8;
const BITS_PER_PIXEL_RGB24: u16 = 24;
const COMPRESSION_BI_RGB: u32 = 0;
const COLOR_TABLE_ENTRY_SIZE: u32 = 4;

/// Output options used when encoding a generic image view as BMP.
#[derive(Clone, Debug, Eq, PartialEq)]
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

    pub fn with_pixel_encoding(mut self, pixel_encoding: BmpPixelEncoding) -> Self {
        self.pixel_encoding = pixel_encoding;
        self
    }
}

impl Default for BmpEncodeOptions {
    fn default() -> Self {
        Self::new()
    }
}

/// Pixel array representation used when encoding BMP.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BmpPixelEncoding {
    Rgb24,
    Indexed8 {
        color_table: Vec<BmpColorTableEntry>,
    },
    AutoIndexed8OrRgb24,
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

/// Decodes an uncompressed BMP image.
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

    validate_supported_native_info_header(&info_header)?;

    let width = info_header.width as u32;
    let height = info_header.height.unsigned_abs();
    let color_table_entry_count = color_table_entry_count(&info_header)?;
    let min_pixel_offset = expected_min_pixel_offset_for_color_table(color_table_entry_count)?;
    let pixel_offset =
        validate_pixel_offset(file_header.pixel_offset, data.len(), min_pixel_offset)?;
    let color_table = read_color_table(&data, color_table_entry_count)?;
    let pixel_data_len = bmp_pixel_array_len(width, height, info_header.bits_per_pixel)?;
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
        color_table,
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
    for entry in &image.color_table {
        writer.write_all(&[entry.blue, entry.green, entry.red, entry.reserved])?;
    }
    writer.write_all(&image.pixel_array)?;

    Ok(())
}

impl BmpImage {
    /// Converts this native BMP image into the generic normalized image buffer.
    pub fn to_image(&self) -> Result<Image> {
        let BmpDibHeader::BitmapInfoHeader(info_header) = &self.dib_header;
        validate_supported_generic_info_header(info_header)?;
        validate_bmp_image_pixels(self)?;

        match info_header.bits_per_pixel {
            BITS_PER_PIXEL_RGB24 => self.to_rgb24_image(info_header),
            BITS_PER_PIXEL_INDEXED1 | BITS_PER_PIXEL_INDEXED4 | BITS_PER_PIXEL_INDEXED8 => {
                self.to_indexed_image(info_header)
            }
            _ => Err(ImageError::UnsupportedFormat),
        }
    }

    fn to_rgb24_image(&self, info_header: &BmpInfoHeader) -> Result<Image> {
        let width = info_header.width as u32;
        let height = info_header.height.unsigned_abs();
        let row_size = bmp_row_size(width, info_header.bits_per_pixel)?;
        let row_len = rgb24_row_len(width)?;
        let mut pixels = rgb8_output_buffer(width, height)?;

        for output_row in 0..height as usize {
            let row_start = bmp_row_start(info_header.height, row_size, output_row)?;
            let row = &self.pixel_array[row_start..row_start + row_len];

            for pixel in row.chunks_exact(3) {
                pixels.extend_from_slice(&[pixel[2], pixel[1], pixel[0]]);
            }
        }

        Image::new(width, height, PixelFormat::Rgb8, pixels)
    }

    fn to_indexed_image(&self, info_header: &BmpInfoHeader) -> Result<Image> {
        let width = info_header.width as u32;
        let height = info_header.height.unsigned_abs();
        let row_size = bmp_row_size(width, info_header.bits_per_pixel)?;
        let mut pixels = rgb8_output_buffer(width, height)?;

        for output_row in 0..height as usize {
            let row_start = bmp_row_start(info_header.height, row_size, output_row)?;
            let row = &self.pixel_array[row_start..row_start + row_size];

            for x in 0..width as usize {
                let index = indexed_pixel(row, x, info_header.bits_per_pixel)?;
                let entry =
                    self.color_table
                        .get(index as usize)
                        .ok_or(ImageError::InvalidData {
                            reason: "BMP color index out of range",
                        })?;
                pixels.extend_from_slice(&[entry.red, entry.green, entry.blue]);
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
        let image_size = bmp_pixel_array_len(width, height, info_header.bits_per_pixel)?;
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

        validate_color_table_layout(&self.color_table)?;

        Ok(())
    }
}

/// Converts an image view into a native BMP image.
pub fn image_view_to_bmp_native(
    image: ImageView<'_>,
    options: BmpEncodeOptions,
) -> Result<BmpImage> {
    match options.pixel_encoding.clone() {
        BmpPixelEncoding::Rgb24 => image_view_to_rgb24_bmp_native(image, options),
        BmpPixelEncoding::Indexed8 { color_table } => {
            image_view_to_indexed8_bmp_native(image, &options, &color_table)
        }
        BmpPixelEncoding::AutoIndexed8OrRgb24 => image_view_to_auto_indexed8_or_rgb24_bmp_native(
            image,
            BmpEncodeOptions {
                pixel_encoding: BmpPixelEncoding::Rgb24,
                orientation: options.orientation,
                x_pixels_per_meter: options.x_pixels_per_meter,
                y_pixels_per_meter: options.y_pixels_per_meter,
            },
        ),
    }
}

fn image_view_to_rgb24_bmp_native(
    image: ImageView<'_>,
    options: BmpEncodeOptions,
) -> Result<BmpImage> {
    let image = validate_rgb8_encode_input(image)?;
    let width_i32 = bmp_i32_dimension(image.width, image.width, image.height)?;
    let height_i32 = bmp_i32_dimension(image.height, image.width, image.height)?;

    let row_size = bmp_row_size(image.width, BITS_PER_PIXEL_RGB24)?;
    let image_size = bmp_pixel_array_len(image.width, image.height, BITS_PER_PIXEL_RGB24)?;
    let image_size_u32 = bmp_u32_image_size(image.width, image.height, image_size)?;
    let file_size = bmp_file_size(PIXEL_OFFSET, image.width, image.height, image_size_u32)?;
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

fn image_view_to_indexed8_bmp_native(
    image: ImageView<'_>,
    options: &BmpEncodeOptions,
    color_table: &[BmpColorTableEntry],
) -> Result<BmpImage> {
    let image = validate_rgb8_encode_input(image)?;
    validate_encode_color_table(color_table)?;

    let width_i32 = bmp_i32_dimension(image.width, image.width, image.height)?;
    let height_i32 = bmp_i32_dimension(image.height, image.width, image.height)?;
    let row_size = bmp_row_size(image.width, BITS_PER_PIXEL_INDEXED8)?;
    let image_size = bmp_pixel_array_len(image.width, image.height, BITS_PER_PIXEL_INDEXED8)?;
    let image_size_u32 = bmp_u32_image_size(image.width, image.height, image_size)?;
    let pixel_offset = expected_min_pixel_offset_for_color_table(color_table.len())?;
    let file_size = bmp_file_size(pixel_offset, image.width, image.height, image_size_u32)?;
    let padding_len = row_size - image.width as usize;
    let padding = [0; 3];
    let palette_indexes = palette_indexes(color_table);
    let mut pixel_array = Vec::with_capacity(image_size);

    match options.orientation {
        BmpOrientation::BottomUp => {
            for output_row in (0..image.height as usize).rev() {
                write_indexed8_bmp_row(&mut pixel_array, image, output_row, &palette_indexes)?;
                pixel_array.extend_from_slice(&padding[..padding_len]);
            }
        }
        BmpOrientation::TopDown => {
            for output_row in 0..image.height as usize {
                write_indexed8_bmp_row(&mut pixel_array, image, output_row, &palette_indexes)?;
                pixel_array.extend_from_slice(&padding[..padding_len]);
            }
        }
    }

    Ok(BmpImage {
        file_header: BmpFileHeader {
            file_size,
            reserved1: 0,
            reserved2: 0,
            pixel_offset,
        },
        dib_header: BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
            width: width_i32,
            height: match options.orientation {
                BmpOrientation::BottomUp => height_i32,
                BmpOrientation::TopDown => -height_i32,
            },
            planes: PLANES,
            bits_per_pixel: BITS_PER_PIXEL_INDEXED8,
            compression: COMPRESSION_BI_RGB,
            image_size: image_size_u32,
            x_pixels_per_meter: options.x_pixels_per_meter,
            y_pixels_per_meter: options.y_pixels_per_meter,
            colors_used: color_table.len() as u32,
            important_colors: 0,
        }),
        color_masks: Vec::new(),
        color_table: color_table.to_vec(),
        pixel_array,
    })
}

fn image_view_to_auto_indexed8_or_rgb24_bmp_native(
    image: ImageView<'_>,
    rgb24_options: BmpEncodeOptions,
) -> Result<BmpImage> {
    let image = validate_rgb8_encode_input(image)?;
    let color_table = auto_color_table(image)?;
    if color_table.len() > 256 {
        return image_view_to_rgb24_bmp_native(image, rgb24_options);
    }

    image_view_to_indexed8_bmp_native(image, &rgb24_options, &color_table)
}

fn validate_rgb8_encode_input(image: ImageView<'_>) -> Result<ImageView<'_>> {
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

    ImageView::new(
        image.width,
        image.height,
        image.pixel_format,
        image.stride,
        image.data,
    )
}

fn bmp_i32_dimension(value: u32, width: u32, height: u32) -> Result<i32> {
    i32::try_from(value).map_err(|_| ImageError::ImageDimensionsTooLarge {
        width,
        height,
        bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
    })
}

fn bmp_u32_image_size(width: u32, height: u32, image_size: usize) -> Result<u32> {
    u32::try_from(image_size).map_err(|_| ImageError::ImageDimensionsTooLarge {
        width,
        height,
        bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
    })
}

fn bmp_file_size(pixel_offset: u32, width: u32, height: u32, image_size: u32) -> Result<u32> {
    pixel_offset
        .checked_add(image_size)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
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

fn write_indexed8_bmp_row<W: Write>(
    writer: &mut W,
    image: ImageView<'_>,
    row_index: usize,
    palette_indexes: &HashMap<[u8; 3], u8>,
) -> Result<()> {
    let row_len = rgb24_row_len(image.width)?;
    let row_start = row_index * image.stride;
    let row = &image.data[row_start..row_start + row_len];

    for pixel in row.chunks_exact(3) {
        let rgb = [pixel[0], pixel[1], pixel[2]];
        let index = palette_indexes.get(&rgb).ok_or(ImageError::InvalidData {
            reason: "BMP palette does not contain input color",
        })?;
        writer.write_all(&[*index])?;
    }

    Ok(())
}

fn indexed_pixel(row: &[u8], x: usize, bits_per_pixel: u16) -> Result<u8> {
    match bits_per_pixel {
        BITS_PER_PIXEL_INDEXED1 => {
            let byte = row[x / 8];
            Ok((byte >> (7 - x % 8)) & 0x01)
        }
        BITS_PER_PIXEL_INDEXED4 => {
            let byte = row[x / 2];
            if x & 1 == 0 {
                Ok(byte >> 4)
            } else {
                Ok(byte & 0x0f)
            }
        }
        BITS_PER_PIXEL_INDEXED8 => Ok(row[x]),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

fn validate_encode_color_table(color_table: &[BmpColorTableEntry]) -> Result<()> {
    if color_table.is_empty() || color_table.len() > 256 {
        return Err(ImageError::InvalidData {
            reason: "invalid BMP color table length",
        });
    }

    if color_table.iter().any(|entry| entry.reserved != 0) {
        return Err(ImageError::InvalidData {
            reason: "invalid BMP color table reserved value",
        });
    }

    Ok(())
}

fn palette_indexes(color_table: &[BmpColorTableEntry]) -> HashMap<[u8; 3], u8> {
    let mut indexes = HashMap::with_capacity(color_table.len());
    for (index, entry) in color_table.iter().enumerate() {
        indexes
            .entry([entry.red, entry.green, entry.blue])
            .or_insert(index as u8);
    }
    indexes
}

fn auto_color_table(image: ImageView<'_>) -> Result<Vec<BmpColorTableEntry>> {
    let mut indexes = HashMap::with_capacity(256);
    let mut color_table = Vec::new();
    let row_len = rgb24_row_len(image.width)?;

    for row_index in 0..image.height as usize {
        let row_start = row_index * image.stride;
        let row = &image.data[row_start..row_start + row_len];

        for pixel in row.chunks_exact(3) {
            let rgb = [pixel[0], pixel[1], pixel[2]];
            if indexes.contains_key(&rgb) {
                continue;
            }

            if color_table.len() == 256 {
                color_table.push(BmpColorTableEntry {
                    red: pixel[0],
                    green: pixel[1],
                    blue: pixel[2],
                    reserved: 0,
                });
                return Ok(color_table);
            }

            indexes.insert(rgb, color_table.len() as u8);
            color_table.push(BmpColorTableEntry {
                red: pixel[0],
                green: pixel[1],
                blue: pixel[2],
                reserved: 0,
            });
        }
    }

    Ok(color_table)
}

fn validate_bmp_image_pixels(image: &BmpImage) -> Result<()> {
    if !image.color_masks.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    let BmpDibHeader::BitmapInfoHeader(info_header) = &image.dib_header;
    validate_supported_native_info_header(info_header)?;
    validate_color_table(info_header, &image.color_table)?;

    let width = info_header.width as u32;
    let height = info_header.height.unsigned_abs();
    let image_size = bmp_pixel_array_len(width, height, info_header.bits_per_pixel)?;

    if image.pixel_array.len() != image_size {
        return Err(ImageError::InvalidBufferLength {
            expected: image_size,
            actual: image.pixel_array.len(),
        });
    }

    Ok(())
}

fn expected_min_pixel_offset(image: &BmpImage) -> Result<u32> {
    let BmpDibHeader::BitmapInfoHeader(info_header) = &image.dib_header;

    if !image.color_masks.is_empty() {
        return Err(ImageError::UnsupportedFormat);
    }

    let expected_color_table_entry_count = color_table_entry_count(info_header)?;
    if image.color_table.len() != expected_color_table_entry_count {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        });
    }

    expected_min_pixel_offset_for_color_table(image.color_table.len())
}

fn validate_supported_generic_info_header(info_header: &BmpInfoHeader) -> Result<()> {
    validate_supported_info_header_shape(info_header)?;

    match (info_header.bits_per_pixel, info_header.compression) {
        (
            BITS_PER_PIXEL_INDEXED1
            | BITS_PER_PIXEL_INDEXED4
            | BITS_PER_PIXEL_INDEXED8
            | BITS_PER_PIXEL_RGB24,
            COMPRESSION_BI_RGB,
        ) => Ok(()),
        (_, COMPRESSION_BI_RGB) => Err(ImageError::UnsupportedFormat),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

fn validate_supported_native_info_header(info_header: &BmpInfoHeader) -> Result<()> {
    validate_supported_info_header_shape(info_header)?;

    match (info_header.bits_per_pixel, info_header.compression) {
        (
            BITS_PER_PIXEL_INDEXED1
            | BITS_PER_PIXEL_INDEXED4
            | BITS_PER_PIXEL_INDEXED8
            | BITS_PER_PIXEL_RGB24,
            COMPRESSION_BI_RGB,
        ) => Ok(()),
        (_, COMPRESSION_BI_RGB) => Err(ImageError::UnsupportedFormat),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

fn validate_supported_info_header_shape(info_header: &BmpInfoHeader) -> Result<()> {
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

    Ok(())
}

fn validate_pixel_offset(
    pixel_offset: u32,
    input_len: usize,
    min_pixel_offset: u32,
) -> Result<usize> {
    if pixel_offset < min_pixel_offset || pixel_offset as usize > input_len {
        return Err(ImageError::InvalidHeader {
            reason: "invalid pixel data offset",
        });
    }

    Ok(pixel_offset as usize)
}

fn color_table_entry_count(info_header: &BmpInfoHeader) -> Result<usize> {
    let max_entries = match max_color_table_entries(info_header.bits_per_pixel) {
        Some(max_entries) => max_entries,
        None => return Ok(0),
    };
    if info_header.colors_used == 0 {
        return Ok(max_entries);
    }

    let colors_used =
        usize::try_from(info_header.colors_used).map_err(|_| ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        })?;
    if colors_used > max_entries {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        });
    }

    Ok(colors_used)
}

fn max_color_table_entries(bits_per_pixel: u16) -> Option<usize> {
    match bits_per_pixel {
        BITS_PER_PIXEL_INDEXED1 | BITS_PER_PIXEL_INDEXED4 | BITS_PER_PIXEL_INDEXED8 => {
            Some(1_usize << bits_per_pixel)
        }
        _ => None,
    }
}

fn read_color_table(data: &[u8], entry_count: usize) -> Result<Vec<BmpColorTableEntry>> {
    let table_bytes = entry_count
        .checked_mul(COLOR_TABLE_ENTRY_SIZE as usize)
        .ok_or(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        })?;
    let table_start = PIXEL_OFFSET as usize;
    let table_end = table_start
        .checked_add(table_bytes)
        .ok_or(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        })?;

    if table_end > data.len() {
        return Err(ImageError::InvalidBufferLength {
            expected: table_end,
            actual: data.len(),
        });
    }

    Ok(data[table_start..table_end]
        .chunks_exact(COLOR_TABLE_ENTRY_SIZE as usize)
        .map(|entry| BmpColorTableEntry {
            blue: entry[0],
            green: entry[1],
            red: entry[2],
            reserved: entry[3],
        })
        .collect())
}

fn validate_color_table(
    info_header: &BmpInfoHeader,
    color_table: &[BmpColorTableEntry],
) -> Result<()> {
    let expected_entry_count = color_table_entry_count(info_header)?;
    if color_table.len() != expected_entry_count {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        });
    }

    Ok(())
}

fn validate_color_table_layout(color_table: &[BmpColorTableEntry]) -> Result<()> {
    if color_table.iter().any(|entry| entry.reserved != 0) {
        return Err(ImageError::InvalidHeader {
            reason: "invalid BMP color table reserved value",
        });
    }

    Ok(())
}

fn rgb8_output_buffer(width: u32, height: u32) -> Result<Vec<u8>> {
    let len = (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(PixelFormat::Rgb8.bytes_per_pixel()))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;

    Ok(Vec::with_capacity(len))
}

fn bmp_row_start(height: i32, row_size: usize, output_row: usize) -> Result<usize> {
    let height_abs = height.unsigned_abs() as usize;
    let bmp_row = match bmp_orientation(height)? {
        BmpOrientation::BottomUp => height_abs - 1 - output_row,
        BmpOrientation::TopDown => output_row,
    };

    bmp_row
        .checked_mul(row_size)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width: 1,
            height: height.unsigned_abs(),
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })
}

fn expected_min_pixel_offset_for_color_table(entry_count: usize) -> Result<u32> {
    let table_bytes = u32::try_from(entry_count)
        .ok()
        .and_then(|entry_count| entry_count.checked_mul(COLOR_TABLE_ENTRY_SIZE))
        .ok_or(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        })?;

    PIXEL_OFFSET
        .checked_add(table_bytes)
        .ok_or(ImageError::InvalidHeader {
            reason: "invalid BMP color table length",
        })
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

fn bmp_row_size(width: u32, bits_per_pixel: u16) -> Result<usize> {
    let row_bits = (width as usize)
        .checked_mul(bits_per_pixel as usize)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })?;

    row_bits
        .checked_add(31)
        .map(|bits| bits / 32 * 4)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height: 1,
            bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
        })
}

fn bmp_pixel_array_len(width: u32, height: u32, bits_per_pixel: u16) -> Result<usize> {
    bmp_row_size(width, bits_per_pixel)?
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
    fn decode_reads_1_bit_indexed_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed1_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_reads_1_bit_indexed_top_down_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed1_top_down_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn decode_reads_4_bit_indexed_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed4_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_reads_4_bit_indexed_top_down_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed4_top_down_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0]);
    }

    #[test]
    fn decode_reads_8_bit_indexed_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed8_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_reads_8_bit_indexed_top_down_bmp_image() {
        let image = decode(&mut Cursor::new(two_by_two_indexed8_top_down_bmp())).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 0, 0, 0, 0, 0, 0, 0, 255, 0, 0]);
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
    fn decode_native_reads_1_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed1_bmp())).unwrap();

        assert_eq!(
            image.file_header,
            BmpFileHeader {
                file_size: 70,
                reserved1: 0,
                reserved2: 0,
                pixel_offset: 62,
            }
        );
        assert_eq!(
            image.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 2,
                height: 2,
                planes: 1,
                bits_per_pixel: 1,
                compression: 0,
                image_size: 8,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 0,
                important_colors: 0,
            })
        );
        assert_eq!(image.color_table, black_and_red_color_table());
        assert_eq!(image.pixel_array, [0x80, 0, 0, 0, 0x40, 0, 0, 0]);
    }

    #[test]
    fn decode_native_reads_4_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed4_bmp())).unwrap();

        assert_eq!(
            image.file_header,
            BmpFileHeader {
                file_size: 70,
                reserved1: 0,
                reserved2: 0,
                pixel_offset: 62,
            }
        );
        assert_eq!(
            image.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 2,
                height: 2,
                planes: 1,
                bits_per_pixel: 4,
                compression: 0,
                image_size: 8,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 2,
                important_colors: 0,
            })
        );
        assert_eq!(image.color_table, black_and_red_color_table());
        assert_eq!(image.pixel_array, [0x10, 0, 0, 0, 0x01, 0, 0, 0]);
    }

    #[test]
    fn decode_native_reads_8_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed8_bmp())).unwrap();

        assert_eq!(
            image.file_header,
            BmpFileHeader {
                file_size: 70,
                reserved1: 0,
                reserved2: 0,
                pixel_offset: 62,
            }
        );
        assert_eq!(
            image.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 2,
                height: 2,
                planes: 1,
                bits_per_pixel: 8,
                compression: 0,
                image_size: 8,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 2,
                important_colors: 0,
            })
        );
        assert_eq!(
            image.color_table,
            [
                BmpColorTableEntry {
                    blue: 0,
                    green: 0,
                    red: 0,
                    reserved: 0,
                },
                BmpColorTableEntry {
                    blue: 0,
                    green: 0,
                    red: 255,
                    reserved: 0,
                },
            ]
        );
        assert_eq!(image.pixel_array, [1, 0, 0, 0, 0, 1, 0, 0]);
    }

    #[test]
    fn decode_native_uses_default_color_table_len_for_8_bit_bmp() {
        let image =
            decode_native(&mut Cursor::new(two_by_two_indexed8_bmp_with_256_colors())).unwrap();

        assert_eq!(image.color_table.len(), 256);
        assert_eq!(image.file_header.pixel_offset, 1078);
    }

    #[test]
    fn decode_native_uses_default_color_table_len_for_1_bit_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed1_bmp_with_color_table(
            2,
        )))
        .unwrap();

        assert_eq!(image.color_table.len(), 2);
        assert_eq!(image.file_header.pixel_offset, 62);
    }

    #[test]
    fn decode_native_uses_default_color_table_len_for_4_bit_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed4_bmp_with_color_table(
            16,
        )))
        .unwrap();

        assert_eq!(image.color_table.len(), 16);
        assert_eq!(image.file_header.pixel_offset, 118);
    }

    #[test]
    fn validate_file_layout_accepts_consistent_bmp_image() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();

        image.validate_file_layout().unwrap();
    }

    #[test]
    fn validate_file_layout_accepts_consistent_8_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed8_bmp())).unwrap();

        image.validate_file_layout().unwrap();
    }

    #[test]
    fn validate_file_layout_accepts_consistent_1_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed1_bmp())).unwrap();

        image.validate_file_layout().unwrap();
    }

    #[test]
    fn validate_file_layout_accepts_consistent_4_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed4_bmp())).unwrap();

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
    fn validate_file_layout_rejects_nonzero_color_table_reserved_value() {
        let mut image = decode_native(&mut Cursor::new(two_by_two_indexed8_bmp())).unwrap();
        image.color_table[0].reserved = 1;
        let error = image.validate_file_layout().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP color table reserved value"
            }
        );
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
    fn decode_rejects_8_bit_color_index_out_of_range() {
        let mut input = two_by_two_indexed8_bmp();
        input[62] = 2;
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "BMP color index out of range"
            }
        );
    }

    #[test]
    fn decode_rejects_1_bit_color_index_out_of_range() {
        let input = two_by_two_indexed1_bmp_with_color_table(1);
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "BMP color index out of range"
            }
        );
    }

    #[test]
    fn decode_rejects_4_bit_color_index_out_of_range() {
        let input = two_by_two_indexed4_bmp_with_color_table(1);
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "BMP color index out of range"
            }
        );
    }

    #[test]
    fn decode_ignores_8_bit_color_table_reserved_byte() {
        let mut input = two_by_two_indexed8_bmp();
        input[57] = 1;
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.data, [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn decode_native_rejects_8_bit_color_table_larger_than_max() {
        let mut input = two_by_two_indexed8_bmp();
        input[46..50].copy_from_slice(&257_u32.to_le_bytes());
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP color table length"
            }
        );
    }

    #[test]
    fn decode_native_rejects_1_bit_color_table_larger_than_max() {
        let mut input = two_by_two_indexed1_bmp();
        input[46..50].copy_from_slice(&3_u32.to_le_bytes());
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP color table length"
            }
        );
    }

    #[test]
    fn decode_native_rejects_4_bit_color_table_larger_than_max() {
        let mut input = two_by_two_indexed4_bmp();
        input[46..50].copy_from_slice(&17_u32.to_le_bytes());
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid BMP color table length"
            }
        );
    }

    #[test]
    fn decode_native_rejects_pixel_offset_inside_color_table() {
        let mut input = two_by_two_indexed8_bmp();
        input[10..14].copy_from_slice(&58_u32.to_le_bytes());
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "invalid pixel data offset"
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
    fn encode_writes_8_bit_indexed_bmp_with_palette() {
        let data = [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0];
        let image = ImageView::new(2, 2, PixelFormat::Rgb8, 6, &data).unwrap();
        let options = BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::Indexed8 {
            color_table: black_and_red_color_table(),
        });
        let mut output = Vec::new();

        encode(&mut output, image, options).unwrap();

        assert_eq!(output, two_by_two_indexed8_bmp());
    }

    #[test]
    fn encode_native_writes_bmp_image() {
        let image = decode_native(&mut Cursor::new(two_by_two_bmp())).unwrap();
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, two_by_two_bmp());
    }

    #[test]
    fn encode_native_writes_8_bit_indexed_bmp() {
        let image = decode_native(&mut Cursor::new(two_by_two_indexed8_bmp())).unwrap();
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(output, two_by_two_indexed8_bmp());
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
    fn image_view_to_bmp_native_converts_rgb8_image_to_indexed8_with_palette() {
        let data = [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0];
        let image = ImageView::new(2, 2, PixelFormat::Rgb8, 6, &data).unwrap();
        let options = BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::Indexed8 {
            color_table: black_and_red_color_table(),
        });

        let native = image_view_to_bmp_native(image, options).unwrap();

        assert_eq!(native.file_header.file_size, 70);
        assert_eq!(native.file_header.pixel_offset, 62);
        assert_eq!(
            native.dib_header,
            BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
                width: 2,
                height: 2,
                planes: 1,
                bits_per_pixel: 8,
                compression: 0,
                image_size: 8,
                x_pixels_per_meter: 0,
                y_pixels_per_meter: 0,
                colors_used: 2,
                important_colors: 0,
            })
        );
        assert_eq!(native.color_table, black_and_red_color_table());
        assert_eq!(native.pixel_array, [1, 0, 0, 0, 0, 1, 0, 0]);
    }

    #[test]
    fn image_view_to_bmp_native_rejects_indexed8_palette_without_input_color() {
        let data = [0, 255, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let options = BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::Indexed8 {
            color_table: black_and_red_color_table(),
        });

        let error = image_view_to_bmp_native(image, options).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "BMP palette does not contain input color"
            }
        );
    }

    #[test]
    fn image_view_to_bmp_native_rejects_empty_indexed8_palette() {
        let data = [0, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let options = BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::Indexed8 {
            color_table: Vec::new(),
        });

        let error = image_view_to_bmp_native(image, options).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "invalid BMP color table length"
            }
        );
    }

    #[test]
    fn image_view_to_bmp_native_rejects_indexed8_palette_reserved_value() {
        let data = [0, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let options = BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::Indexed8 {
            color_table: vec![BmpColorTableEntry {
                blue: 0,
                green: 0,
                red: 0,
                reserved: 1,
            }],
        });

        let error = image_view_to_bmp_native(image, options).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "invalid BMP color table reserved value"
            }
        );
    }

    #[test]
    fn image_view_to_bmp_native_auto_uses_indexed8_for_256_or_fewer_colors() {
        let data = [0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 0, 0];
        let image = ImageView::new(2, 2, PixelFormat::Rgb8, 6, &data).unwrap();

        let native = image_view_to_bmp_native(
            image,
            BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::AutoIndexed8OrRgb24),
        )
        .unwrap();

        let BmpDibHeader::BitmapInfoHeader(info_header) = native.dib_header;
        assert_eq!(info_header.bits_per_pixel, 8);
        assert_eq!(info_header.colors_used, 2);
        assert_eq!(native.color_table, black_and_red_color_table());
        assert_eq!(native.pixel_array, [1, 0, 0, 0, 0, 1, 0, 0]);
    }

    #[test]
    fn image_view_to_bmp_native_auto_uses_rgb24_for_more_than_256_colors() {
        let mut data = Vec::new();
        for value in 0..257_u32 {
            data.extend_from_slice(&[(value & 0xff) as u8, (value >> 8) as u8, 0]);
        }
        let image = ImageView::new(257, 1, PixelFormat::Rgb8, 257 * 3, &data).unwrap();

        let native = image_view_to_bmp_native(
            image,
            BmpEncodeOptions::new().with_pixel_encoding(BmpPixelEncoding::AutoIndexed8OrRgb24),
        )
        .unwrap();

        let BmpDibHeader::BitmapInfoHeader(info_header) = native.dib_header;
        assert_eq!(info_header.bits_per_pixel, 24);
        assert_eq!(info_header.colors_used, 0);
        assert!(native.color_table.is_empty());
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

    fn two_by_two_indexed1_bmp() -> Vec<u8> {
        two_by_two_indexed_bmp_with_color_table(
            BITS_PER_PIXEL_INDEXED1,
            2,
            &[0x80, 0, 0, 0, 0x40, 0, 0, 0],
        )
    }

    fn two_by_two_indexed1_top_down_bmp() -> Vec<u8> {
        let mut data = two_by_two_indexed1_bmp();
        data[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
        data[62..70].copy_from_slice(&[0x80, 0, 0, 0, 0x40, 0, 0, 0]);
        data
    }

    fn two_by_two_indexed1_bmp_with_color_table(color_count: usize) -> Vec<u8> {
        two_by_two_indexed_bmp_with_color_table(
            BITS_PER_PIXEL_INDEXED1,
            color_count,
            &[0x80, 0, 0, 0, 0x40, 0, 0, 0],
        )
    }

    fn two_by_two_indexed4_bmp() -> Vec<u8> {
        two_by_two_indexed4_bmp_with_color_table(2)
    }

    fn two_by_two_indexed4_top_down_bmp() -> Vec<u8> {
        let mut data = two_by_two_indexed4_bmp();
        data[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
        data[62..70].copy_from_slice(&[0x10, 0, 0, 0, 0x01, 0, 0, 0]);
        data
    }

    fn two_by_two_indexed4_bmp_with_color_table(color_count: usize) -> Vec<u8> {
        two_by_two_indexed_bmp_with_color_table(
            BITS_PER_PIXEL_INDEXED4,
            color_count,
            &[0x10, 0, 0, 0, 0x01, 0, 0, 0],
        )
    }

    fn two_by_two_indexed8_bmp() -> Vec<u8> {
        two_by_two_indexed8_bmp_with_color_table(2)
    }

    fn two_by_two_indexed8_top_down_bmp() -> Vec<u8> {
        let mut data = two_by_two_indexed8_bmp();
        data[22..26].copy_from_slice(&(-2_i32).to_le_bytes());
        data[62..70].copy_from_slice(&[1, 0, 0, 0, 0, 1, 0, 0]);
        data
    }

    fn two_by_two_indexed8_bmp_with_256_colors() -> Vec<u8> {
        two_by_two_indexed8_bmp_with_color_table(256)
    }

    fn black_and_red_color_table() -> Vec<BmpColorTableEntry> {
        vec![
            BmpColorTableEntry {
                blue: 0,
                green: 0,
                red: 0,
                reserved: 0,
            },
            BmpColorTableEntry {
                blue: 0,
                green: 0,
                red: 255,
                reserved: 0,
            },
        ]
    }

    fn two_by_two_indexed8_bmp_with_color_table(color_count: usize) -> Vec<u8> {
        two_by_two_indexed_bmp_with_color_table(
            BITS_PER_PIXEL_INDEXED8,
            color_count,
            &[1, 0, 0, 0, 0, 1, 0, 0],
        )
    }

    fn two_by_two_indexed_bmp_with_color_table(
        bits_per_pixel: u16,
        color_count: usize,
        pixel_array: &[u8],
    ) -> Vec<u8> {
        let color_table_len = color_count * COLOR_TABLE_ENTRY_SIZE as usize;
        let pixel_offset = PIXEL_OFFSET as usize + color_table_len;
        let file_size = pixel_offset + pixel_array.len();
        let max_colors = 1_usize << bits_per_pixel;
        let mut data = Vec::new();

        data.extend_from_slice(b"BM");
        data.extend_from_slice(&(file_size as u32).to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&0_u16.to_le_bytes());
        data.extend_from_slice(&(pixel_offset as u32).to_le_bytes());

        data.extend_from_slice(&40_u32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&2_i32.to_le_bytes());
        data.extend_from_slice(&1_u16.to_le_bytes());
        data.extend_from_slice(&bits_per_pixel.to_le_bytes());
        data.extend_from_slice(&0_u32.to_le_bytes());
        data.extend_from_slice(&(pixel_array.len() as u32).to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(&0_i32.to_le_bytes());
        data.extend_from_slice(
            &(if color_count == max_colors {
                0
            } else {
                color_count as u32
            })
            .to_le_bytes(),
        );
        data.extend_from_slice(&0_u32.to_le_bytes());

        data.extend_from_slice(&[0, 0, 0, 0]);
        data.extend_from_slice(&[0, 0, 255, 0]);
        for _ in 2..color_count {
            data.extend_from_slice(&[0, 0, 0, 0]);
        }
        data.extend_from_slice(pixel_array);

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
