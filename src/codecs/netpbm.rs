use std::io::Write;

use crate::{ImageError, ImageView, PixelFormat, Result};

pub(crate) const MAX_VALUE: u32 = 255;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Dimensions {
    pub(crate) width: u32,
    pub(crate) height: u32,
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

            while self.position < self.data.len() && self.data[self.position] != b'\n' {
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

pub(crate) fn read_sample_header(
    parser: &mut HeaderParser<'_>,
    expected_magic: &[u8],
) -> Result<Dimensions> {
    let dimensions = read_bitmap_header(parser, expected_magic)?;
    let max_value = parser.next_u32("max value")?;

    if max_value != MAX_VALUE {
        return Err(ImageError::InvalidHeader {
            reason: "only max value 255 is supported",
        });
    }

    Ok(dimensions)
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

pub(crate) fn read_ascii_samples(
    parser: &mut HeaderParser<'_>,
    expected: usize,
) -> Result<Vec<u8>> {
    let mut pixels = Vec::with_capacity(expected);

    for _ in 0..expected {
        pixels.push(parser.next_u8_sample(MAX_VALUE)?);
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

pub(crate) fn write_sample_header<W: Write>(
    writer: &mut W,
    magic: &str,
    image: ImageView<'_>,
) -> Result<()> {
    writeln!(writer, "{magic}")?;
    writeln!(writer, "{} {}", image.width, image.height)?;
    writeln!(writer, "{MAX_VALUE}")?;

    Ok(())
}

pub(crate) fn write_bitmap_header<W: Write>(
    writer: &mut W,
    magic: &str,
    image: ImageView<'_>,
) -> Result<()> {
    writeln!(writer, "{magic}")?;
    writeln!(writer, "{} {}", image.width, image.height)?;

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
    matches!(byte, b' ' | b'\t' | b'\n' | b'\r')
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
    fn read_sample_header_reads_magic_dimensions_and_max_value() {
        let mut parser = HeaderParser::new(b"P6\n2 1\n255\n");

        assert_eq!(
            read_sample_header(&mut parser, b"P6").unwrap(),
            Dimensions {
                width: 2,
                height: 1
            }
        );
    }

    #[test]
    fn read_sample_header_rejects_unsupported_max_value() {
        let mut parser = HeaderParser::new(b"P6\n1 1\n256\n");
        let error = read_sample_header(&mut parser, b"P6").unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidHeader {
                reason: "only max value 255 is supported"
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

        assert_eq!(read_ascii_samples(&mut parser, 3).unwrap(), [1, 2, 3]);
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
    fn write_sample_header_writes_magic_dimensions_and_max_value() {
        let data = [1];
        let image = ImageView::new(1, 1, PixelFormat::Gray8, 1, &data).unwrap();
        let mut output = Vec::new();

        write_sample_header(&mut output, "P5", image).unwrap();

        assert_eq!(output, b"P5\n1 1\n255\n");
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
}
