use std::io::Write;

use crate::{ImageError, ImageView, PixelFormat, Result};

pub(crate) const MAX_SUPPORTED_VALUE: u32 = 65_535;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Dimensions {
    pub(crate) width: u32,
    pub(crate) height: u32,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NetpbmImage {
    Pbm {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    Pgm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
    Ppm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
}

pub(crate) struct HeaderParser<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> HeaderParser<'a> {
    pub(crate) fn new(data: &'a [u8]) -> Self {
        Self { data, position: 0 }
    }

    pub(crate) fn position(&self) -> usize {
        self.position
    }

    pub(crate) fn set_position(&mut self, position: usize) {
        self.position = position;
    }

    pub(crate) fn next_token(&mut self) -> Result<&'a [u8]> {
        self.skip_whitespace_and_comments();

        if self.position >= self.data.len() {
            return Err(ImageError::InvalidHeader {
                reason: "unexpected end of header",
            });
        }

        let start = self.position;

        while self.position < self.data.len() && !is_whitespace(self.data[self.position]) {
            self.position += 1;
        }

        Ok(&self.data[start..self.position])
    }

    pub(crate) fn next_u32(&mut self, name: &'static str) -> Result<u32> {
        let token = self.next_token()?;
        let text = std::str::from_utf8(token).map_err(|_| ImageError::InvalidHeader {
            reason: "header contains non-UTF-8 token",
        })?;

        text.parse::<u32>()
            .map_err(|_| ImageError::InvalidHeader { reason: name })
    }

    pub(crate) fn next_u8_sample(&mut self, max_value: u32) -> Result<u8> {
        let sample = self.next_u32("sample")?;

        if sample > max_value {
            return Err(ImageError::InvalidData {
                reason: "sample value exceeds max value",
            });
        }

        u8::try_from(sample).map_err(|_| ImageError::InvalidData {
            reason: "sample value is larger than u8",
        })
    }

    pub(crate) fn next_u16_sample(&mut self, max_value: u32) -> Result<u16> {
        let sample = self.next_u32("sample")?;

        if sample > max_value {
            return Err(ImageError::InvalidData {
                reason: "sample value exceeds max value",
            });
        }

        u16::try_from(sample).map_err(|_| ImageError::InvalidData {
            reason: "sample value is larger than u16",
        })
    }

    pub(crate) fn next_max_value(&mut self) -> Result<u32> {
        let max_value = self.next_u32("max value")?;

        if !(1..=MAX_SUPPORTED_VALUE).contains(&max_value) {
            return Err(ImageError::InvalidHeader {
                reason: "max value must be between 1 and 65535",
            });
        }

        Ok(max_value)
    }

    pub(crate) fn has_more_tokens(&mut self) -> bool {
        self.skip_whitespace_and_comments();
        self.position < self.data.len()
    }

    pub(crate) fn consume_raster_separator(&mut self) -> Result<()> {
        if self.position >= self.data.len() || !is_whitespace(self.data[self.position]) {
            return Err(ImageError::InvalidHeader {
                reason: "missing raster separator",
            });
        }

        if self.data[self.position] == b'\r'
            && self.position + 1 < self.data.len()
            && self.data[self.position + 1] == b'\n'
        {
            self.position += 2;
            return Ok(());
        }

        self.position += 1;
        Ok(())
    }

    fn skip_whitespace_and_comments(&mut self) {
        loop {
            while self.position < self.data.len() && is_whitespace(self.data[self.position]) {
                self.position += 1;
            }

            if self.position >= self.data.len() || self.data[self.position] != b'#' {
                break;
            }

            while self.position < self.data.len()
                && self.data[self.position] != b'\r'
                && self.data[self.position] != b'\n'
            {
                self.position += 1;
            }
        }
    }
}

pub(crate) fn read_bitmap_header(
    parser: &mut HeaderParser<'_>,
    expected_magic: &[u8],
) -> Result<Dimensions> {
    let magic = parser.next_token()?;

    if magic != expected_magic {
        return Err(ImageError::UnsupportedFormat);
    }

    let dimensions = Dimensions {
        width: parser.next_u32("width")?,
        height: parser.next_u32("height")?,
    };
    validate_dimensions(dimensions)?;

    Ok(dimensions)
}

pub(crate) fn read_any_sample_header(
    parser: &mut HeaderParser<'_>,
    expected_magic: &[u8],
) -> Result<(Dimensions, u16)> {
    let dimensions = read_bitmap_header(parser, expected_magic)?;
    let max_value = parser.next_max_value()?;

    Ok((dimensions, max_value as u16))
}

pub(crate) fn validate_image_view(
    image: ImageView<'_>,
    pixel_format: PixelFormat,
) -> Result<ImageView<'_>> {
    if image.pixel_format != pixel_format {
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

pub(crate) fn read_ascii_samples_with_max_value(
    parser: &mut HeaderParser<'_>,
    expected: usize,
    max_value: u32,
) -> Result<Vec<u8>> {
    let mut pixels = Vec::with_capacity(expected);

    for _ in 0..expected {
        pixels.push(parser.next_u8_sample(max_value)?);
    }

    reject_trailing_tokens(parser)?;

    Ok(pixels)
}

pub(crate) fn read_ascii_sample_bytes_with_max_value(
    parser: &mut HeaderParser<'_>,
    expected_samples: usize,
    max_value: u16,
) -> Result<Vec<u8>> {
    if max_value < 256 {
        return read_ascii_samples_with_max_value(parser, expected_samples, u32::from(max_value));
    }

    let mut pixels = Vec::with_capacity(expected_samples * 2);

    for _ in 0..expected_samples {
        pixels.extend_from_slice(&parser.next_u16_sample(u32::from(max_value))?.to_le_bytes());
    }

    reject_trailing_tokens(parser)?;

    Ok(pixels)
}

pub(crate) fn reject_trailing_tokens(parser: &mut HeaderParser<'_>) -> Result<()> {
    if parser.has_more_tokens() {
        return Err(ImageError::InvalidData {
            reason: "too many samples",
        });
    }

    Ok(())
}

pub(crate) fn packed_raster_len(
    dimensions: Dimensions,
    pixel_format: PixelFormat,
) -> Result<usize> {
    let row_len = dimensions
        .width
        .checked_mul(pixel_format.bytes_per_pixel() as u32)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width: dimensions.width,
            height: dimensions.height,
            bytes_per_pixel: pixel_format.bytes_per_pixel(),
        })? as usize;

    row_len
        .checked_mul(dimensions.height as usize)
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width: dimensions.width,
            height: dimensions.height,
            bytes_per_pixel: pixel_format.bytes_per_pixel(),
        })
}

pub(crate) fn raster_slice<'a>(
    data: &'a [u8],
    parser: &HeaderParser<'_>,
    dimensions: Dimensions,
    pixel_format: PixelFormat,
) -> Result<&'a [u8]> {
    let raster_start = parser.position();
    let raster_len = packed_raster_len(dimensions, pixel_format)?;
    let raster_end =
        raster_start
            .checked_add(raster_len)
            .ok_or(ImageError::ImageDimensionsTooLarge {
                width: dimensions.width,
                height: dimensions.height,
                bytes_per_pixel: pixel_format.bytes_per_pixel(),
            })?;

    if raster_end > data.len() {
        return Err(ImageError::InvalidBufferLength {
            expected: raster_len,
            actual: data.len().saturating_sub(raster_start),
        });
    }

    Ok(&data[raster_start..raster_end])
}

pub(crate) fn write_sample_header_with_max_value<W: Write>(
    writer: &mut W,
    magic: &str,
    width: u32,
    height: u32,
    maxval: u16,
) -> Result<()> {
    writeln!(writer, "{magic}")?;
    writeln!(writer, "{width} {height}")?;
    writeln!(writer, "{maxval}")?;

    Ok(())
}

pub(crate) fn normalize_sample_to_u8(sample: u8, maxval: u16) -> u8 {
    let sample = u32::from(sample);
    let maxval = u32::from(maxval);

    ((sample * u32::from(u8::MAX) + maxval / 2) / maxval) as u8
}

pub(crate) fn normalize_sample_to_u16(sample: u16, maxval: u16) -> u16 {
    let sample = u32::from(sample);
    let maxval = u32::from(maxval);

    ((sample * u32::from(u16::MAX) + maxval / 2) / maxval) as u16
}

pub(crate) fn write_bitmap_header<W: Write>(
    writer: &mut W,
    magic: &str,
    image: ImageView<'_>,
) -> Result<()> {
    write_bitmap_header_dimensions(writer, magic, image.width, image.height)
}

pub(crate) fn write_bitmap_header_dimensions<W: Write>(
    writer: &mut W,
    magic: &str,
    width: u32,
    height: u32,
) -> Result<()> {
    writeln!(writer, "{magic}")?;
    writeln!(writer, "{width} {height}")?;

    Ok(())
}

pub(crate) fn write_packed_rows<W: Write>(writer: &mut W, image: ImageView<'_>) -> Result<()> {
    let row_len = image.width as usize * image.pixel_format.bytes_per_pixel();

    for row in 0..image.height as usize {
        let start = row * image.stride;
        let end = start + row_len;
        writer.write_all(&image.data[start..end])?;
    }

    Ok(())
}

fn validate_dimensions(dimensions: Dimensions) -> Result<()> {
    if dimensions.width == 0 || dimensions.height == 0 {
        return Err(ImageError::InvalidHeader {
            reason: "width and height must be greater than zero",
        });
    }

    Ok(())
}

fn is_whitespace(byte: u8) -> bool {
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r' | 0x0b | 0x0c)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_token_skips_whitespace_and_comments() {
        let mut parser = HeaderParser::new(b" \n# comment\nP6 2");

        assert_eq!(parser.next_token().unwrap(), b"P6");
        assert_eq!(parser.next_token().unwrap(), b"2");
    }

    #[test]
    fn next_token_treats_vertical_tab_and_form_feed_as_whitespace() {
        let mut parser = HeaderParser::new(b"\x0bP5\x0c2");

        assert_eq!(parser.next_token().unwrap(), b"P5");
        assert_eq!(parser.next_token().unwrap(), b"2");
    }

    #[test]
    fn next_token_ends_comment_at_carriage_return() {
        let mut parser = HeaderParser::new(b"# comment\rP6");

        assert_eq!(parser.next_token().unwrap(), b"P6");
    }

    #[test]
    fn next_u32_parses_integer_token() {
        let mut parser = HeaderParser::new(b"255");

        assert_eq!(parser.next_u32("max value").unwrap(), 255);
    }

    #[test]
    fn next_u32_rejects_invalid_integer_token() {
        let mut parser = HeaderParser::new(b"abc");
        let error = parser.next_u32("width").unwrap_err();

        assert_eq!(error, ImageError::InvalidHeader { reason: "width" });
    }

    #[test]
    fn next_u8_sample_parses_sample_within_max_value() {
        let mut parser = HeaderParser::new(b"128");

        assert_eq!(parser.next_u8_sample(255).unwrap(), 128);
    }

    #[test]
    fn next_u8_sample_rejects_sample_above_max_value() {
        let mut parser = HeaderParser::new(b"256");
        let error = parser.next_u8_sample(255).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "sample value exceeds max value"
            }
        );
    }

    #[test]
    fn next_max_value_accepts_spec_range() {
        let mut parser = HeaderParser::new(b"65535");

        assert_eq!(parser.next_max_value().unwrap(), 65_535);
    }

    #[test]
    fn next_max_value_rejects_zero() {
        let mut parser = HeaderParser::new(b"0");
        let error = parser.next_max_value().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "max value must be between 1 and 65535"
            }
        );
    }

    #[test]
    fn next_max_value_rejects_value_above_spec_range() {
        let mut parser = HeaderParser::new(b"65536");
        let error = parser.next_max_value().unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "max value must be between 1 and 65535"
            }
        );
    }

    #[test]
    fn has_more_tokens_skips_comments_before_checking() {
        let mut parser = HeaderParser::new(b"  # comment\n");

        assert!(!parser.has_more_tokens());
    }

    #[test]
    fn consume_raster_separator_consumes_single_whitespace() {
        let mut parser = HeaderParser::new(b"255\nabc");

        assert_eq!(parser.next_token().unwrap(), b"255");
        parser.consume_raster_separator().unwrap();
        assert_eq!(parser.position(), 4);
    }

    #[test]
    fn consume_raster_separator_consumes_crlf_as_one_separator() {
        let mut parser = HeaderParser::new(b"255\r\nabc");

        assert_eq!(parser.next_token().unwrap(), b"255");
        parser.consume_raster_separator().unwrap();
        assert_eq!(parser.position(), 5);
    }

    #[test]
    fn read_bitmap_header_reads_magic_and_dimensions() {
        let mut parser = HeaderParser::new(b"P1\n3 2\n");

        assert_eq!(
            read_bitmap_header(&mut parser, b"P1").unwrap(),
            Dimensions {
                width: 3,
                height: 2
            }
        );
    }

    #[test]
    fn read_bitmap_header_rejects_unexpected_magic() {
        let mut parser = HeaderParser::new(b"P4\n1 1\n");
        let error = read_bitmap_header(&mut parser, b"P1").unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn read_any_sample_header_reads_magic_dimensions_and_max_value() {
        let mut parser = HeaderParser::new(b"P6\n2 1\n255\n");

        assert_eq!(
            read_any_sample_header(&mut parser, b"P6").unwrap(),
            (
                Dimensions {
                    width: 2,
                    height: 1
                },
                255
            )
        );
    }

    #[test]
    fn read_any_sample_header_accepts_max_value_above_255() {
        let mut parser = HeaderParser::new(b"P6\n1 1\n256\n");

        assert_eq!(
            read_any_sample_header(&mut parser, b"P6").unwrap(),
            (
                Dimensions {
                    width: 1,
                    height: 1
                },
                256
            )
        );
    }

    #[test]
    fn read_any_sample_header_rejects_zero_max_value() {
        let mut parser = HeaderParser::new(b"P6\n1 1\n0\n");
        let error = read_any_sample_header(&mut parser, b"P6").unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "max value must be between 1 and 65535"
            }
        );
    }

    #[test]
    fn validate_image_view_accepts_matching_format() {
        let data = [1, 2, 3, 4, 5, 6];
        let image = ImageView::new(2, 1, PixelFormat::Rgb8, 6, &data).unwrap();

        assert!(validate_image_view(image, PixelFormat::Rgb8).is_ok());
    }

    #[test]
    fn validate_image_view_rejects_unexpected_format() {
        let data = [1];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let error = validate_image_view(image, PixelFormat::Rgb8).unwrap_err();

        assert_eq!(
            error,
            ImageError::UnsupportedPixelFormat {
                pixel_format: PixelFormat::Gray8
            }
        );
    }

    #[test]
    fn read_ascii_samples_reads_expected_samples() {
        let mut parser = HeaderParser::new(b"1 2 3");

        assert_eq!(
            read_ascii_samples_with_max_value(&mut parser, 3, 255).unwrap(),
            [1, 2, 3]
        );
    }

    #[test]
    fn reject_trailing_tokens_rejects_extra_sample() {
        let mut parser = HeaderParser::new(b"1");
        let error = reject_trailing_tokens(&mut parser).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidData {
                reason: "too many samples"
            }
        );
    }

    #[test]
    fn write_sample_header_with_max_value_writes_custom_max_value() {
        let mut output = Vec::new();

        write_sample_header_with_max_value(&mut output, "P2", 2, 1, 15).unwrap();

        assert_eq!(output, b"P2\n2 1\n15\n");
    }

    #[test]
    fn normalize_sample_to_u8_scales_to_full_range() {
        assert_eq!(normalize_sample_to_u8(0, 15), 0);
        assert_eq!(normalize_sample_to_u8(5, 15), 85);
        assert_eq!(normalize_sample_to_u8(10, 15), 170);
        assert_eq!(normalize_sample_to_u8(15, 15), 255);
    }

    #[test]
    fn normalize_sample_to_u8_rounds_to_nearest() {
        assert_eq!(normalize_sample_to_u8(5, 10), 128);
    }

    #[test]
    fn write_bitmap_header_writes_magic_and_dimensions() {
        let data = [1];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let mut output = Vec::new();

        write_bitmap_header(&mut output, "P4", image).unwrap();

        assert_eq!(output, b"P4\n1 1\n");
    }

    #[test]
    fn write_packed_rows_writes_only_pixel_bytes_from_strided_rows() {
        let data = [1, 2, 99, 3, 4, 88];
        let image = ImageView::new(2, 2, PixelFormat::Gray8, 3, &data).unwrap();
        let mut output = Vec::new();

        write_packed_rows(&mut output, image).unwrap();

        assert_eq!(output, [1, 2, 3, 4]);
    }

    #[test]
    fn packed_raster_len_returns_required_byte_count() {
        let dimensions = Dimensions {
            width: 2,
            height: 3,
        };

        assert_eq!(
            packed_raster_len(dimensions, PixelFormat::Rgb8).unwrap(),
            18
        );
    }

    #[test]
    fn raster_slice_returns_only_current_image_raster() {
        let data = b"P5\n2 1\n255\n\x10\x20P5\n1 1\n255\n\x30";
        let mut parser = HeaderParser::new(data);
        let (dimensions, _) = read_any_sample_header(&mut parser, b"P5").unwrap();
        parser.consume_raster_separator().unwrap();

        assert_eq!(
            raster_slice(data, &parser, dimensions, PixelFormat::Gray8).unwrap(),
            b"\x10\x20"
        );
    }

    #[test]
    fn raster_slice_rejects_short_raster() {
        let data = b"P5\n2 1\n255\n\x10";
        let mut parser = HeaderParser::new(data);
        let (dimensions, _) = read_any_sample_header(&mut parser, b"P5").unwrap();
        parser.consume_raster_separator().unwrap();
        let error = raster_slice(data, &parser, dimensions, PixelFormat::Gray8).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 2,
                actual: 1
            }
        );
    }
}
