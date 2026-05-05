use std::io::{Read, Write};

use crate::codecs::netpbm::{normalize_sample_to_u8, normalize_sample_to_u16};
use crate::{Image, ImageError, ImageView, PixelFormat, Result};

const MAGIC: &[u8] = b"P7";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PamImage {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub maxval: u16,
    pub tuple_type: Option<PamTupleType>,
    pub data: Vec<u8>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PamTupleType {
    BlackAndWhite,
    Grayscale,
    Rgb,
    BlackAndWhiteAlpha,
    GrayscaleAlpha,
    RgbAlpha,
    Other(String),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PamEncodeTupleType {
    BlackAndWhite,
    Grayscale,
    Rgb,
    BlackAndWhiteAlpha,
    GrayscaleAlpha,
    RgbAlpha,
}

impl PamImage {
    pub fn to_image(&self) -> Result<Image> {
        Image::try_from(self.clone())
    }
}

impl TryFrom<PamImage> for Image {
    type Error = ImageError;

    fn try_from(image: PamImage) -> Result<Self> {
        pam_image_to_image(image)
    }
}

pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_native(reader)?)
}

pub fn decode_all<R: Read>(reader: &mut R) -> Result<Vec<Image>> {
    decode_all_native(reader)?
        .into_iter()
        .map(Image::try_from)
        .collect()
}

pub fn decode_native<R: Read>(reader: &mut R) -> Result<PamImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut position = 0;
    let image = decode_one_native(&data, &mut position)?;
    if has_more_data(&data, position) {
        return Err(ImageError::InvalidData {
            reason: "too many images",
        });
    }

    Ok(image)
}

pub fn decode_all_native<R: Read>(reader: &mut R) -> Result<Vec<PamImage>> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;

    let mut position = 0;
    let mut images = Vec::new();

    while has_more_data(&data, position) {
        images.push(decode_one_native(&data, &mut position)?);
    }

    Ok(images)
}

pub fn encode<W: Write>(
    writer: &mut W,
    image: ImageView<'_>,
    tuple_type: PamEncodeTupleType,
) -> Result<()> {
    let image = image_view_to_pam_native(image, tuple_type)?;
    encode_native(writer, &image)
}

pub fn encode_all<W: Write>(
    writer: &mut W,
    images: &[ImageView<'_>],
    tuple_type: PamEncodeTupleType,
) -> Result<()> {
    let images = images
        .iter()
        .map(|image| image_view_to_pam_native(*image, tuple_type))
        .collect::<Result<Vec<_>>>()?;

    encode_all_native(writer, &images)
}

pub fn encode_native<W: Write>(writer: &mut W, image: &PamImage) -> Result<()> {
    validate_pam_image(image)?;

    writeln!(writer, "P7")?;
    writeln!(writer, "WIDTH {}", image.width)?;
    writeln!(writer, "HEIGHT {}", image.height)?;
    writeln!(writer, "DEPTH {}", image.depth)?;
    writeln!(writer, "MAXVAL {}", image.maxval)?;
    if let Some(tuple_type) = &image.tuple_type {
        writeln!(writer, "TUPLTYPE {}", tuple_type.as_header_value())?;
    }
    writeln!(writer, "ENDHDR")?;

    if image.maxval < 256 {
        writer.write_all(&image.data)?;
    } else {
        for sample in image.data.chunks_exact(2) {
            writer.write_all(&u16::from_le_bytes([sample[0], sample[1]]).to_be_bytes())?;
        }
    }

    Ok(())
}

pub fn encode_all_native<W: Write>(writer: &mut W, images: &[PamImage]) -> Result<()> {
    for image in images {
        validate_pam_image(image)?;
    }

    for image in images {
        encode_native(writer, image)?;
    }

    Ok(())
}

fn decode_one_native(data: &[u8], position: &mut usize) -> Result<PamImage> {
    let header = read_header(data, position)?;
    let raster_len = pam_raster_len(header.width, header.height, header.depth, header.maxval)?;
    let raster_start = *position;
    let raster_end =
        raster_start
            .checked_add(raster_len)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width: header.width,
                height: header.height,
                bytes_per_pixel: bytes_per_tuple(header.depth, header.maxval)?,
            })?;

    if raster_end > data.len() {
        return Err(ImageError::InvalidBufferLength {
            expected: raster_len,
            actual: data.len().saturating_sub(raster_start),
        });
    }

    let raster = &data[raster_start..raster_end];
    let data = if header.maxval < 256 {
        validate_8_bit_samples(raster, header.maxval)?;
        raster.to_vec()
    } else {
        be_samples_to_le_bytes(raster, header.maxval)?
    };
    *position = raster_end;

    let image = PamImage {
        width: header.width,
        height: header.height,
        depth: header.depth,
        maxval: header.maxval,
        tuple_type: header.tuple_type,
        data,
    };
    validate_tuple_type_shape(&image)?;

    Ok(image)
}

#[derive(Default)]
struct PamHeader {
    width: Option<u32>,
    height: Option<u32>,
    depth: Option<u32>,
    maxval: Option<u16>,
    tuple_type_parts: Vec<String>,
}

struct CompletePamHeader {
    width: u32,
    height: u32,
    depth: u32,
    maxval: u16,
    tuple_type: Option<PamTupleType>,
}

fn read_header(data: &[u8], position: &mut usize) -> Result<CompletePamHeader> {
    let magic = read_line(data, position)?;
    if trim_line_end(magic) != MAGIC {
        return Err(ImageError::UnsupportedFormat);
    }

    let mut header = PamHeader::default();
    let mut saw_endhdr = false;

    while *position < data.len() {
        let line = trim_line_end(read_line(data, position)?);
        let trimmed = trim_ascii_whitespace(line);

        if trimmed.is_empty() || trimmed[0] == b'#' {
            continue;
        }

        if trimmed == b"ENDHDR" {
            saw_endhdr = true;
            break;
        }

        parse_header_line(trimmed, &mut header)?;
    }

    if !saw_endhdr {
        return Err(ImageError::InvalidHeader {
            reason: "missing ENDHDR",
        });
    }

    let width = required_header_u32(header.width, "missing WIDTH")?;
    let height = required_header_u32(header.height, "missing HEIGHT")?;
    let depth = required_header_u32(header.depth, "missing DEPTH")?;
    let maxval = required_header_u16(header.maxval, "missing MAXVAL")?;

    if width == 0 || height == 0 || depth == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "width, height, and depth must be greater than zero",
        });
    }

    let tuple_type = if header.tuple_type_parts.is_empty() {
        None
    } else {
        Some(PamTupleType::from_header_value(
            &header.tuple_type_parts.join(" "),
        ))
    };

    Ok(CompletePamHeader {
        width,
        height,
        depth,
        maxval,
        tuple_type,
    })
}

fn parse_header_line(line: &[u8], header: &mut PamHeader) -> Result<()> {
    let (key, rest) = split_first_token(line);

    match key {
        b"WIDTH" => {
            reject_duplicate(header.width.is_some())?;
            header.width = Some(parse_u32_token(rest, "width")?);
        }
        b"HEIGHT" => {
            reject_duplicate(header.height.is_some())?;
            header.height = Some(parse_u32_token(rest, "height")?);
        }
        b"DEPTH" => {
            reject_duplicate(header.depth.is_some())?;
            header.depth = Some(parse_u32_token(rest, "depth")?);
        }
        b"MAXVAL" => {
            reject_duplicate(header.maxval.is_some())?;
            let maxval = parse_u32_token(rest, "max value")?;
            if !(1..=u32::from(u16::MAX)).contains(&maxval) {
                return Err(ImageError::InvalidHeader {
                    reason: "max value must be between 1 and 65535",
                });
            }
            header.maxval = Some(maxval as u16);
        }
        b"TUPLTYPE" => {
            let value = trim_ascii_whitespace(rest);
            if value.is_empty() {
                return Err(ImageError::InvalidHeader {
                    reason: "missing TUPLTYPE value",
                });
            }
            let value = std::str::from_utf8(value).map_err(|_| ImageError::InvalidHeader {
                reason: "header contains non-UTF-8 tuple type",
            })?;
            header.tuple_type_parts.push(value.to_string());
        }
        _ => {
            return Err(ImageError::InvalidHeader {
                reason: "unknown PAM header field",
            });
        }
    }

    Ok(())
}

fn pam_image_to_image(image: PamImage) -> Result<Image> {
    validate_pam_image(&image)?;

    match image.tuple_type {
        Some(PamTupleType::BlackAndWhite) => black_and_white_to_image(image),
        Some(PamTupleType::BlackAndWhiteAlpha) => black_and_white_alpha_to_image(image),
        Some(PamTupleType::Grayscale) => grayscale_to_image(image),
        Some(PamTupleType::GrayscaleAlpha) => grayscale_alpha_to_image(image),
        Some(PamTupleType::Rgb) => rgb_to_image(image),
        Some(PamTupleType::RgbAlpha) => rgba_to_image(image),
        Some(PamTupleType::Other(_)) | None => Err(ImageError::UnsupportedFormat),
    }
}

fn black_and_white_to_image(image: PamImage) -> Result<Image> {
    let data = image
        .data
        .into_iter()
        .map(|sample| if sample == 0 { 0 } else { 255 })
        .collect();
    Image::new(image.width, image.height, PixelFormat::Gray8, data)
}

fn black_and_white_alpha_to_image(image: PamImage) -> Result<Image> {
    let data = image
        .data
        .into_iter()
        .map(|sample| if sample == 0 { 0 } else { 255 })
        .collect();
    Image::new(image.width, image.height, PixelFormat::GrayAlpha8, data)
}

fn grayscale_to_image(image: PamImage) -> Result<Image> {
    if image.maxval == u16::from(u8::MAX) {
        return Image::new(image.width, image.height, PixelFormat::Gray8, image.data);
    }

    if image.maxval < 256 {
        let data = image
            .data
            .into_iter()
            .map(|sample| normalize_sample_to_u8(sample, image.maxval))
            .collect();
        return Image::new(image.width, image.height, PixelFormat::Gray8, data);
    }

    let mut data = Vec::with_capacity(image.data.len());
    for sample in image.data.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        data.extend_from_slice(&normalize_sample_to_u16(sample, image.maxval).to_le_bytes());
    }
    Image::new(image.width, image.height, PixelFormat::Gray16, data)
}

fn grayscale_alpha_to_image(image: PamImage) -> Result<Image> {
    if image.maxval == u16::from(u8::MAX) {
        return Image::new(
            image.width,
            image.height,
            PixelFormat::GrayAlpha8,
            image.data,
        );
    }

    if image.maxval < 256 {
        let data = image
            .data
            .into_iter()
            .map(|sample| normalize_sample_to_u8(sample, image.maxval))
            .collect();
        return Image::new(image.width, image.height, PixelFormat::GrayAlpha8, data);
    }

    let mut data = Vec::with_capacity(image.data.len());
    for sample in image.data.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        data.extend_from_slice(&normalize_sample_to_u16(sample, image.maxval).to_le_bytes());
    }
    Image::new(image.width, image.height, PixelFormat::GrayAlpha16, data)
}

fn rgb_to_image(image: PamImage) -> Result<Image> {
    if image.maxval == u16::from(u8::MAX) {
        return Image::new(image.width, image.height, PixelFormat::Rgb8, image.data);
    }

    if image.maxval < 256 {
        let data = image
            .data
            .into_iter()
            .map(|sample| normalize_sample_to_u8(sample, image.maxval))
            .collect();
        return Image::new(image.width, image.height, PixelFormat::Rgb8, data);
    }

    let mut data = Vec::with_capacity(image.data.len());
    for sample in image.data.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        data.extend_from_slice(&normalize_sample_to_u16(sample, image.maxval).to_le_bytes());
    }
    Image::new(image.width, image.height, PixelFormat::Rgb16, data)
}

fn rgba_to_image(image: PamImage) -> Result<Image> {
    if image.maxval == u16::from(u8::MAX) {
        return Image::new(image.width, image.height, PixelFormat::Rgba8, image.data);
    }

    if image.maxval < 256 {
        let data = image
            .data
            .into_iter()
            .map(|sample| normalize_sample_to_u8(sample, image.maxval))
            .collect();
        return Image::new(image.width, image.height, PixelFormat::Rgba8, data);
    }

    let mut data = Vec::with_capacity(image.data.len());
    for sample in image.data.chunks_exact(2) {
        let sample = u16::from_le_bytes([sample[0], sample[1]]);
        data.extend_from_slice(&normalize_sample_to_u16(sample, image.maxval).to_le_bytes());
    }
    Image::new(image.width, image.height, PixelFormat::Rgba16, data)
}

fn image_view_to_pam_native(
    image: ImageView<'_>,
    tuple_type: PamEncodeTupleType,
) -> Result<PamImage> {
    let image = validate_image_view(image)?;
    let (depth, maxval, tuple_type, data) = match tuple_type {
        PamEncodeTupleType::BlackAndWhite => {
            let image = require_pixel_format(image, PixelFormat::Gray8)?;
            (
                1,
                1,
                PamTupleType::BlackAndWhite,
                black_and_white_data(image)?,
            )
        }
        PamEncodeTupleType::Grayscale => match image.pixel_format {
            PixelFormat::Gray8 => (
                1,
                u16::from(u8::MAX),
                PamTupleType::Grayscale,
                packed_image_data(image),
            ),
            PixelFormat::Gray16 => (
                1,
                u16::MAX,
                PamTupleType::Grayscale,
                packed_image_data(image),
            ),
            pixel_format => return Err(ImageError::UnsupportedPixelFormat { pixel_format }),
        },
        PamEncodeTupleType::Rgb => match image.pixel_format {
            PixelFormat::Rgb8 => (
                3,
                u16::from(u8::MAX),
                PamTupleType::Rgb,
                packed_image_data(image),
            ),
            PixelFormat::Rgb16 => (3, u16::MAX, PamTupleType::Rgb, packed_image_data(image)),
            pixel_format => return Err(ImageError::UnsupportedPixelFormat { pixel_format }),
        },
        PamEncodeTupleType::BlackAndWhiteAlpha => {
            let image = require_pixel_format(image, PixelFormat::GrayAlpha8)?;
            (
                2,
                1,
                PamTupleType::BlackAndWhiteAlpha,
                black_and_white_data(image)?,
            )
        }
        PamEncodeTupleType::GrayscaleAlpha => match image.pixel_format {
            PixelFormat::GrayAlpha8 => (
                2,
                u16::from(u8::MAX),
                PamTupleType::GrayscaleAlpha,
                packed_image_data(image),
            ),
            PixelFormat::GrayAlpha16 => (
                2,
                u16::MAX,
                PamTupleType::GrayscaleAlpha,
                packed_image_data(image),
            ),
            pixel_format => return Err(ImageError::UnsupportedPixelFormat { pixel_format }),
        },
        PamEncodeTupleType::RgbAlpha => match image.pixel_format {
            PixelFormat::Rgba8 => (
                4,
                u16::from(u8::MAX),
                PamTupleType::RgbAlpha,
                packed_image_data(image),
            ),
            PixelFormat::Rgba16 => (
                4,
                u16::MAX,
                PamTupleType::RgbAlpha,
                packed_image_data(image),
            ),
            pixel_format => return Err(ImageError::UnsupportedPixelFormat { pixel_format }),
        },
    };

    Ok(PamImage {
        width: image.width,
        height: image.height,
        depth,
        maxval,
        tuple_type: Some(tuple_type),
        data,
    })
}

fn require_pixel_format(image: ImageView<'_>, pixel_format: PixelFormat) -> Result<ImageView<'_>> {
    if image.pixel_format != pixel_format {
        return Err(ImageError::UnsupportedPixelFormat {
            pixel_format: image.pixel_format,
        });
    }
    Ok(image)
}

fn black_and_white_data(image: ImageView<'_>) -> Result<Vec<u8>> {
    let mut data = Vec::with_capacity(image.width as usize * image.height as usize);

    for row in image_rows(image) {
        for sample in row {
            match *sample {
                0 => data.push(0),
                255 => data.push(1),
                _ => {
                    return Err(ImageError::InvalidData {
                        reason: "black-and-white PAM conversion requires samples to be 0 or 255",
                    });
                }
            }
        }
    }

    Ok(data)
}

fn validate_pam_image(image: &PamImage) -> Result<()> {
    if image.width == 0 || image.height == 0 || image.depth == 0 {
        return Err(ImageError::InvalidData {
            reason: "width, height, and depth must be greater than zero",
        });
    }
    if image.maxval == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "max value must be between 1 and 65535",
        });
    }

    validate_tuple_type_shape(image)?;

    let expected = pam_raster_len(image.width, image.height, image.depth, image.maxval)?;
    if image.data.len() != expected {
        return Err(ImageError::InvalidBufferLength {
            expected,
            actual: image.data.len(),
        });
    }

    if image.maxval < 256 {
        validate_8_bit_samples(&image.data, image.maxval)?;
    } else {
        validate_16_bit_samples(&image.data, image.maxval)?;
    }

    Ok(())
}

fn validate_tuple_type_shape(image: &PamImage) -> Result<()> {
    let Some(tuple_type) = &image.tuple_type else {
        return Ok(());
    };

    let expected = match tuple_type {
        PamTupleType::BlackAndWhite => {
            if image.maxval != 1 {
                return Err(ImageError::InvalidHeader {
                    reason: "BLACKANDWHITE PAM requires maxval 1",
                });
            }
            1
        }
        PamTupleType::BlackAndWhiteAlpha => {
            if image.maxval != 1 {
                return Err(ImageError::InvalidHeader {
                    reason: "BLACKANDWHITE_ALPHA PAM requires maxval 1",
                });
            }
            2
        }
        PamTupleType::Grayscale => 1,
        PamTupleType::GrayscaleAlpha => 2,
        PamTupleType::Rgb => 3,
        PamTupleType::RgbAlpha => 4,
        PamTupleType::Other(_) => return Ok(()),
    };

    if image.depth != expected {
        return Err(ImageError::InvalidHeader {
            reason: "PAM tuple type does not match depth",
        });
    }

    Ok(())
}

fn validate_image_view(image: ImageView<'_>) -> Result<ImageView<'_>> {
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

fn packed_image_data(image: ImageView<'_>) -> Vec<u8> {
    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();
    let mut data = Vec::with_capacity(row_len * image.height as usize);

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        data.extend_from_slice(&image.data[start..end]);
    }

    data
}

fn image_rows(image: ImageView<'_>) -> impl Iterator<Item = &'_ [u8]> {
    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();

    (0..image.height as usize).map(move |row| {
        let start = row * image.stride;
        let end = start + row_len;
        &image.data[start..end]
    })
}

fn validate_8_bit_samples(data: &[u8], maxval: u16) -> Result<()> {
    if data.iter().any(|sample| u16::from(*sample) > maxval) {
        return Err(ImageError::InvalidData {
            reason: "sample value exceeds max value",
        });
    }
    Ok(())
}

fn validate_16_bit_samples(data: &[u8], maxval: u16) -> Result<()> {
    if data
        .chunks_exact(2)
        .any(|sample| u16::from_le_bytes([sample[0], sample[1]]) > maxval)
    {
        return Err(ImageError::InvalidData {
            reason: "sample value exceeds max value",
        });
    }
    Ok(())
}

fn be_samples_to_le_bytes(data: &[u8], maxval: u16) -> Result<Vec<u8>> {
    let mut output = Vec::with_capacity(data.len());

    for sample in data.chunks_exact(2) {
        let sample = u16::from_be_bytes([sample[0], sample[1]]);
        if sample > maxval {
            return Err(ImageError::InvalidData {
                reason: "sample value exceeds max value",
            });
        }
        output.extend_from_slice(&sample.to_le_bytes());
    }

    Ok(output)
}

fn pam_raster_len(width: u32, height: u32, depth: u32, maxval: u16) -> Result<usize> {
    let bytes_per_tuple = bytes_per_tuple(depth, maxval)?;

    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(bytes_per_tuple))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: bytes_per_tuple,
        })
}

fn bytes_per_tuple(depth: u32, maxval: u16) -> Result<usize> {
    (depth as usize)
        .checked_mul(bytes_per_sample(maxval))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width: 0,
            height: 0,
            bytes_per_pixel: bytes_per_sample(maxval),
        })
}

fn bytes_per_sample(maxval: u16) -> usize {
    if maxval < 256 { 1 } else { 2 }
}

fn read_line<'a>(data: &'a [u8], position: &mut usize) -> Result<&'a [u8]> {
    if *position >= data.len() {
        return Err(ImageError::InvalidHeader {
            reason: "unexpected end of header",
        });
    }

    let start = *position;
    while *position < data.len() && data[*position] != b'\n' {
        *position += 1;
    }

    if *position >= data.len() {
        return Err(ImageError::InvalidHeader {
            reason: "PAM header line is not newline terminated",
        });
    }

    *position += 1;
    Ok(&data[start..*position])
}

fn has_more_data(data: &[u8], position: usize) -> bool {
    position < data.len()
}

fn trim_line_end(line: &[u8]) -> &[u8] {
    let line = line.strip_suffix(b"\n").unwrap_or(line);
    line.strip_suffix(b"\r").unwrap_or(line)
}

fn trim_ascii_whitespace(mut data: &[u8]) -> &[u8] {
    while let Some((first, rest)) = data.split_first() {
        if !first.is_ascii_whitespace() {
            break;
        }
        data = rest;
    }

    while let Some((last, rest)) = data.split_last() {
        if !last.is_ascii_whitespace() {
            break;
        }
        data = rest;
    }

    data
}

fn split_first_token(line: &[u8]) -> (&[u8], &[u8]) {
    let mut index = 0;
    while index < line.len() && !line[index].is_ascii_whitespace() {
        index += 1;
    }

    let rest = if index < line.len() {
        &line[index + 1..]
    } else {
        &[]
    };
    (&line[..index], rest)
}

fn parse_u32_token(data: &[u8], name: &'static str) -> Result<u32> {
    let data = trim_ascii_whitespace(data);
    if data.iter().any(|byte| byte.is_ascii_whitespace()) {
        return Err(ImageError::InvalidHeader { reason: name });
    }

    let text = std::str::from_utf8(data).map_err(|_| ImageError::InvalidHeader {
        reason: "header contains non-UTF-8 token",
    })?;
    text.parse::<u32>()
        .map_err(|_| ImageError::InvalidHeader { reason: name })
}

fn reject_duplicate(duplicate: bool) -> Result<()> {
    if duplicate {
        return Err(ImageError::InvalidHeader {
            reason: "duplicate PAM header field",
        });
    }
    Ok(())
}

fn required_header_u32(value: Option<u32>, reason: &'static str) -> Result<u32> {
    value.ok_or(ImageError::InvalidHeader { reason })
}

fn required_header_u16(value: Option<u16>, reason: &'static str) -> Result<u16> {
    value.ok_or(ImageError::InvalidHeader { reason })
}

impl PamTupleType {
    fn from_header_value(value: &str) -> Self {
        match value {
            "BLACKANDWHITE" => Self::BlackAndWhite,
            "GRAYSCALE" => Self::Grayscale,
            "RGB" => Self::Rgb,
            "BLACKANDWHITE_ALPHA" => Self::BlackAndWhiteAlpha,
            "GRAYSCALE_ALPHA" => Self::GrayscaleAlpha,
            "RGB_ALPHA" => Self::RgbAlpha,
            _ => Self::Other(value.to_string()),
        }
    }

    fn as_header_value(&self) -> &str {
        match self {
            Self::BlackAndWhite => "BLACKANDWHITE",
            Self::Grayscale => "GRAYSCALE",
            Self::Rgb => "RGB",
            Self::BlackAndWhiteAlpha => "BLACKANDWHITE_ALPHA",
            Self::GrayscaleAlpha => "GRAYSCALE_ALPHA",
            Self::RgbAlpha => "RGB_ALPHA",
            Self::Other(value) => value,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use super::*;

    #[test]
    fn decode_native_reads_grayscale_pam() {
        let input =
            b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 1\nMAXVAL 15\nTUPLTYPE GRAYSCALE\nENDHDR\n\0\x0f";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            PamImage {
                width: 2,
                height: 1,
                depth: 1,
                maxval: 15,
                tuple_type: Some(PamTupleType::Grayscale),
                data: vec![0, 15],
            }
        );
    }

    #[test]
    fn decode_native_reads_rgb_16_bit_samples_as_little_endian() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 65535\nTUPLTYPE RGB\nENDHDR\n\x12\x34\x56\x78\xff\xff";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            PamImage {
                width: 1,
                height: 1,
                depth: 3,
                maxval: 65535,
                tuple_type: Some(PamTupleType::Rgb),
                data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff],
            }
        );
    }

    #[test]
    fn decode_native_reads_header_without_tuple_type() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 255\nENDHDR\n\x01\x02";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            PamImage {
                width: 1,
                height: 1,
                depth: 2,
                maxval: 255,
                tuple_type: None,
                data: vec![1, 2],
            }
        );
    }

    #[test]
    fn decode_native_preserves_unknown_tuple_type() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 255\nTUPLTYPE FOO\nTUPLTYPE BAR\nENDHDR\n\x01\x02";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image.tuple_type,
            Some(PamTupleType::Other("FOO BAR".into()))
        );
        assert_eq!(image.data, [1, 2]);
    }

    #[test]
    fn decode_all_native_reads_multi_image_stream() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n\0P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n\xff";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                PamImage {
                    width: 1,
                    height: 1,
                    depth: 1,
                    maxval: 255,
                    tuple_type: Some(PamTupleType::Grayscale),
                    data: vec![0],
                },
                PamImage {
                    width: 1,
                    height: 1,
                    depth: 1,
                    maxval: 255,
                    tuple_type: Some(PamTupleType::Grayscale),
                    data: vec![255],
                },
            ]
        );
    }

    #[test]
    fn decode_all_reads_multi_image_stream_as_images() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\0\0\0P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\xff\xff\xff";
        let images = decode_all(&mut Cursor::new(input)).unwrap();

        assert_eq!(images.len(), 2);
        assert_eq!(images[0].pixel_format, PixelFormat::Rgb8);
        assert_eq!(images[0].data, [0, 0, 0]);
        assert_eq!(images[1].pixel_format, PixelFormat::Rgb8);
        assert_eq!(images[1].data, [255, 255, 255]);
    }

    #[test]
    fn decode_all_rejects_unsupported_image_conversion() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nTUPLTYPE HEIGHTMAP\nENDHDR\n\x80";
        let error = decode_all(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_converts_black_and_white_to_gray8() {
        let input =
            b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 1\nMAXVAL 1\nTUPLTYPE BLACKANDWHITE\nENDHDR\n\0\x01";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 255]);
    }

    #[test]
    fn decode_normalizes_grayscale_to_gray8() {
        let input =
            b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 1\nMAXVAL 15\nTUPLTYPE GRAYSCALE\nENDHDR\n\0\x0f";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0, 255]);
    }

    #[test]
    fn decode_normalizes_rgb_to_rgb16() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 1000\nTUPLTYPE RGB\nENDHDR\n\x03\xe8\x01\xf4\0\0";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgb16);
        assert_eq!(image.data, [0xff, 0xff, 0x00, 0x80, 0, 0]);
    }

    #[test]
    fn decode_converts_rgb_alpha_to_rgba8() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 15\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x0f\0\x08\x04";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgba8);
        assert_eq!(image.data, [255, 0, 136, 68]);
    }

    #[test]
    fn decode_converts_black_and_white_alpha_to_gray_alpha8() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 1\nTUPLTYPE BLACKANDWHITE_ALPHA\nENDHDR\n\x01\0";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::GrayAlpha8);
        assert_eq!(image.data, [255, 0]);
    }

    #[test]
    fn decode_converts_grayscale_alpha_to_gray_alpha16() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 1000\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n\x03\xe8\x01\xf4";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::GrayAlpha16);
        assert_eq!(image.data, [0xff, 0xff, 0x00, 0x80]);
    }

    #[test]
    fn decode_converts_rgb_alpha_to_rgba16() {
        let input =
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 1000\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x03\xe8\x01\xf4\0\0\x03\xe8";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgba16);
        assert_eq!(image.data, [0xff, 0xff, 0x00, 0x80, 0, 0, 0xff, 0xff]);
    }

    #[test]
    fn decode_rejects_unknown_tuple_type_for_image_conversion() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nTUPLTYPE HEIGHTMAP\nENDHDR\n\x80";
        let error = decode(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn encode_writes_rgb8_image_as_rgb_pam() {
        let data = [255, 0, 128];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, PamEncodeTupleType::Rgb).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\xff\0\x80"
        );
    }

    #[test]
    fn encode_writes_rgba8_image_as_rgb_alpha_pam() {
        let data = [255, 0, 128, 64];
        let image = ImageView::new(1, 1, PixelFormat::Rgba8, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, PamEncodeTupleType::RgbAlpha).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 255\nTUPLTYPE RGB_ALPHA\nENDHDR\n\xff\0\x80\x40"
        );
    }

    #[test]
    fn encode_writes_gray8_image_as_black_and_white_pam() {
        let data = [0, 255];
        let image = ImageView::new(2, 1, PixelFormat::Gray8, 2, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, PamEncodeTupleType::BlackAndWhite).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 1\nMAXVAL 1\nTUPLTYPE BLACKANDWHITE\nENDHDR\n\0\x01"
        );
    }

    #[test]
    fn encode_writes_gray_alpha16_image_as_grayscale_alpha_pam() {
        let data = [0x34, 0x12, 0xff, 0xff];
        let image = ImageView::new(1, 1, PixelFormat::GrayAlpha16, 4, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, PamEncodeTupleType::GrayscaleAlpha).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 65535\nTUPLTYPE GRAYSCALE_ALPHA\nENDHDR\n\x12\x34\xff\xff"
        );
    }

    #[test]
    fn encode_writes_rgba16_image_as_rgb_alpha_pam() {
        let data = [0x34, 0x12, 0x78, 0x56, 0xff, 0xff, 0, 0];
        let image = ImageView::new(1, 1, PixelFormat::Rgba16, 8, &data).unwrap();
        let mut output = Vec::new();

        encode(&mut output, image, PamEncodeTupleType::RgbAlpha).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 4\nMAXVAL 65535\nTUPLTYPE RGB_ALPHA\nENDHDR\n\x12\x34\x56\x78\xff\xff\0\0"
        );
    }

    #[test]
    fn encode_rejects_non_black_and_white_samples() {
        let data = [128];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let error = encode(&mut Vec::new(), image, PamEncodeTupleType::BlackAndWhite).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "black-and-white PAM conversion requires samples to be 0 or 255"
            }
        );
    }

    #[test]
    fn encode_rejects_pixel_format_not_matching_tuple_type() {
        let data = [255, 0, 128];
        let image = ImageView::new(1, 1, PixelFormat::Rgb8, 3, &data).unwrap();
        let error = encode(&mut Vec::new(), image, PamEncodeTupleType::Grayscale).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Rgb8
            }
        );
    }

    #[test]
    fn encode_all_writes_multi_image_stream() {
        let first_data = [0, 0, 0];
        let second_data = [255, 255, 255];
        let images = [
            ImageView::new(1, 1, PixelFormat::Rgb8, 3, &first_data).unwrap(),
            ImageView::new(1, 1, PixelFormat::Rgb8, 3, &second_data).unwrap(),
        ];
        let mut output = Vec::new();

        encode_all(&mut output, &images, PamEncodeTupleType::Rgb).unwrap();

        assert_eq!(
            decode_all(&mut Cursor::new(output)).unwrap(),
            [
                Image::new(1, 1, PixelFormat::Rgb8, vec![0, 0, 0]).unwrap(),
                Image::new(1, 1, PixelFormat::Rgb8, vec![255, 255, 255]).unwrap(),
            ]
        );
    }

    #[test]
    fn encode_all_rejects_invalid_image_without_writing() {
        let image = ImageView {
            width: 0,
            height: 1,
            pixel_format: PixelFormat::Rgb8,
            stride: 0,
            data: &[],
        };
        let mut output = Vec::new();
        let error = encode_all(&mut output, &[image], PamEncodeTupleType::Rgb).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "width and height must be greater than zero"
            }
        );
        assert!(output.is_empty());
    }

    #[test]
    fn encode_native_writes_16_bit_samples_as_big_endian() {
        let image = PamImage {
            width: 1,
            height: 1,
            depth: 3,
            maxval: 65535,
            tuple_type: Some(PamTupleType::Rgb),
            data: vec![0x34, 0x12, 0x78, 0x56, 0xff, 0xff],
        };
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(
            output,
            b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 65535\nTUPLTYPE RGB\nENDHDR\n\x12\x34\x56\x78\xff\xff"
        );
    }

    #[test]
    fn encode_all_native_writes_multi_image_stream() {
        let images = [
            PamImage {
                width: 1,
                height: 1,
                depth: 1,
                maxval: 255,
                tuple_type: Some(PamTupleType::Grayscale),
                data: vec![0],
            },
            PamImage {
                width: 1,
                height: 1,
                depth: 1,
                maxval: 255,
                tuple_type: Some(PamTupleType::Grayscale),
                data: vec![255],
            },
        ];
        let mut output = Vec::new();

        encode_all_native(&mut output, &images).unwrap();

        assert_eq!(decode_all_native(&mut Cursor::new(output)).unwrap(), images);
    }

    #[test]
    fn encode_all_native_rejects_invalid_image_without_writing() {
        let images = [PamImage {
            width: 1,
            height: 1,
            depth: 1,
            maxval: 255,
            tuple_type: Some(PamTupleType::Grayscale),
            data: vec![0, 1],
        }];
        let mut output = Vec::new();
        let error = encode_all_native(&mut output, &images).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 1,
                actual: 2
            }
        );
        assert!(output.is_empty());
    }

    #[test]
    fn decode_rejects_missing_required_header() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nMAXVAL 255\nENDHDR\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "missing DEPTH"
            }
        );
    }

    #[test]
    fn decode_rejects_duplicate_required_header() {
        let input = b"P7\nWIDTH 1\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nENDHDR\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "duplicate PAM header field"
            }
        );
    }

    #[test]
    fn decode_rejects_unknown_header_field() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nFOO 1\nENDHDR\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "unknown PAM header field"
            }
        );
    }

    #[test]
    fn decode_rejects_missing_endhdr() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "PAM header line is not newline terminated"
            }
        );
    }

    #[test]
    fn decode_rejects_tuple_type_depth_mismatch() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 2\nMAXVAL 255\nTUPLTYPE GRAYSCALE\nENDHDR\n\0\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "PAM tuple type does not match depth"
            }
        );
    }

    #[test]
    fn decode_rejects_zero_maxval() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 0\nENDHDR\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "max value must be between 1 and 65535"
            }
        );
    }

    #[test]
    fn decode_rejects_short_raster() {
        let input = b"P7\nWIDTH 2\nHEIGHT 1\nDEPTH 1\nMAXVAL 255\nENDHDR\n\0";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 2,
                actual: 1
            }
        );
    }

    #[test]
    fn decode_rejects_sample_above_maxval() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 1\nMAXVAL 15\nENDHDR\n\x10";
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "sample value exceeds max value"
            }
        );
    }
}
