use crate::{ImageError, Result};

/// Pixel formats supported by the core image container.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PixelFormat {
    Gray8,
    Gray16,
    Rgb8,
    Rgb16,
    Rgba8,
}

impl PixelFormat {
    /// Returns the number of color channels in this pixel format.
    pub const fn channels(self) -> usize {
        match self {
            Self::Gray8 => 1,
            Self::Gray16 => 1,
            Self::Rgb8 => 3,
            Self::Rgb16 => 3,
            Self::Rgba8 => 4,
        }
    }

    /// Returns the number of bytes used by one pixel in this pixel format.
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Gray8 => 1,
            Self::Gray16 => 2,
            Self::Rgb8 => 3,
            Self::Rgb16 => 6,
            Self::Rgba8 => 4,
        }
    }
}

/// An owned, tightly packed image buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub data: Vec<u8>,
}

impl Image {
    /// Creates an image and validates that the buffer length matches its dimensions.
    pub fn new(width: u32, height: u32, pixel_format: PixelFormat, data: Vec<u8>) -> Result<Self> {
        let expected = packed_buffer_len(width, height, pixel_format)?;
        let actual = data.len();

        if actual != expected {
            return Err(ImageError::InvalidBufferLength { expected, actual });
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            data,
        })
    }

    /// Returns the number of bytes in a tightly packed row.
    pub fn stride(&self) -> usize {
        row_len(self.width, self.pixel_format)
    }

    /// Borrows this image as a read-only image view.
    pub fn as_view(&self) -> ImageView<'_> {
        ImageView {
            width: self.width,
            height: self.height,
            pixel_format: self.pixel_format,
            stride: self.stride(),
            data: &self.data,
        }
    }
}

/// A borrowed image buffer with an explicit row stride.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageView<'a> {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub stride: usize,
    pub data: &'a [u8],
}

impl<'a> ImageView<'a> {
    /// Creates an image view and validates that the buffer covers every row.
    pub fn new(
        width: u32,
        height: u32,
        pixel_format: PixelFormat,
        stride: usize,
        data: &'a [u8],
    ) -> Result<Self> {
        let minimum_stride = row_len(width, pixel_format);

        if stride < minimum_stride {
            return Err(ImageError::InvalidStride {
                minimum: minimum_stride,
                actual: stride,
            });
        }

        let expected = strided_buffer_len(width, height, pixel_format, stride)?;
        let actual = data.len();

        if actual < expected {
            return Err(ImageError::InvalidBufferLength { expected, actual });
        }

        Ok(Self {
            width,
            height,
            pixel_format,
            stride,
            data,
        })
    }
}

fn row_len(width: u32, pixel_format: PixelFormat) -> usize {
    width as usize * pixel_format.bytes_per_pixel()
}

fn packed_buffer_len(width: u32, height: u32, pixel_format: PixelFormat) -> Result<usize> {
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|pixels| pixels.checked_mul(pixel_format.bytes_per_pixel()))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: pixel_format.bytes_per_pixel(),
        })
}

fn strided_buffer_len(
    width: u32,
    height: u32,
    pixel_format: PixelFormat,
    stride: usize,
) -> Result<usize> {
    if height == 0 {
        return Ok(0);
    }

    let row = row_len(width, pixel_format);

    (height as usize - 1)
        .checked_mul(stride)
        .and_then(|previous_rows| previous_rows.checked_add(row))
        .ok_or(ImageError::ImageDimensionsTooLarge {
            width,
            height,
            bytes_per_pixel: pixel_format.bytes_per_pixel(),
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pixel_format_channels_returns_channel_count() {
        assert_eq!(PixelFormat::Gray8.channels(), 1);
        assert_eq!(PixelFormat::Gray16.channels(), 1);
        assert_eq!(PixelFormat::Rgb8.channels(), 3);
        assert_eq!(PixelFormat::Rgb16.channels(), 3);
        assert_eq!(PixelFormat::Rgba8.channels(), 4);
    }

    #[test]
    fn pixel_format_bytes_per_pixel_returns_byte_count() {
        assert_eq!(PixelFormat::Gray8.bytes_per_pixel(), 1);
        assert_eq!(PixelFormat::Gray16.bytes_per_pixel(), 2);
        assert_eq!(PixelFormat::Rgb8.bytes_per_pixel(), 3);
        assert_eq!(PixelFormat::Rgb16.bytes_per_pixel(), 6);
        assert_eq!(PixelFormat::Rgba8.bytes_per_pixel(), 4);
    }

    #[test]
    fn image_new_accepts_matching_buffer_length() {
        let image = Image::new(2, 2, PixelFormat::Rgb8, vec![0; 12]).unwrap();

        assert_eq!(image.width, 2);
        assert_eq!(image.height, 2);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data.len(), 12);
    }

    #[test]
    fn image_new_rejects_invalid_buffer_length() {
        let error = Image::new(2, 2, PixelFormat::Rgb8, vec![0; 11]).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 12,
                actual: 11
            }
        );
    }

    #[test]
    fn image_stride_returns_packed_row_length() {
        let image = Image::new(3, 2, PixelFormat::Rgba8, vec![0; 24]).unwrap();

        assert_eq!(image.stride(), 12);
    }

    #[test]
    fn image_as_view_borrows_image_buffer() {
        let image = Image::new(2, 2, PixelFormat::Rgba8, vec![1; 16]).unwrap();
        let view = image.as_view();

        assert_eq!(view.width, 2);
        assert_eq!(view.height, 2);
        assert_eq!(view.pixel_format, PixelFormat::Rgba8);
        assert_eq!(view.stride, 8);
        assert_eq!(view.data, image.data.as_slice());
    }

    #[test]
    fn image_view_new_accepts_matching_strided_buffer() {
        let data = [0; 16];
        let view = ImageView::new(2, 2, PixelFormat::Rgb8, 8, &data).unwrap();

        assert_eq!(view.stride, 8);
        assert_eq!(view.data.len(), 16);
    }

    #[test]
    fn image_view_new_rejects_stride_smaller_than_row_length() {
        let data = [0; 12];
        let error = ImageView::new(2, 2, PixelFormat::Rgb8, 5, &data).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidStride {
                minimum: 6,
                actual: 5
            }
        );
    }

    #[test]
    fn image_view_new_rejects_short_strided_buffer() {
        let data = [0; 13];
        let error = ImageView::new(2, 2, PixelFormat::Rgb8, 8, &data).unwrap_err();

        assert_eq!(
            error,
            ImageError::InvalidBufferLength {
                expected: 14,
                actual: 13
            }
        );
    }
}
