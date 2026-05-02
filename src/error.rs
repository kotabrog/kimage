use std::fmt;

use crate::PixelFormat;

/// Error type used by image decoding, encoding, and buffer validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImageError {
    /// The provided pixel buffer does not match the expected size.
    InvalidBufferLength { expected: usize, actual: usize },
    /// The provided row stride is too small for the image width and pixel format.
    InvalidStride { minimum: usize, actual: usize },
    /// The image dimensions overflow the platform address size.
    ImageDimensionsTooLarge {
        width: u32,
        height: u32,
        bytes_per_pixel: usize,
    },
    /// The image format is not supported by this crate.
    UnsupportedFormat,
    /// The pixel format is not supported by a specific operation.
    UnsupportedPixelFormat { pixel_format: PixelFormat },
}

/// Convenient result type for this crate.
pub type Result<T> = std::result::Result<T, ImageError>;

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBufferLength { expected, actual } => {
                write!(
                    f,
                    "invalid buffer length: expected {expected} bytes, got {actual} bytes"
                )
            }
            Self::InvalidStride { minimum, actual } => {
                write!(
                    f,
                    "invalid row stride: expected at least {minimum} bytes, got {actual} bytes"
                )
            }
            Self::ImageDimensionsTooLarge {
                width,
                height,
                bytes_per_pixel,
            } => {
                write!(
                    f,
                    "image dimensions are too large: {width}x{height} at {bytes_per_pixel} bytes per pixel"
                )
            }
            Self::UnsupportedFormat => f.write_str("unsupported image format"),
            Self::UnsupportedPixelFormat { pixel_format } => {
                write!(f, "unsupported pixel format: {pixel_format:?}")
            }
        }
    }
}

impl std::error::Error for ImageError {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_invalid_buffer_length_includes_expected_and_actual() {
        let error = ImageError::InvalidBufferLength {
            expected: 12,
            actual: 10,
        };

        assert_eq!(
            error.to_string(),
            "invalid buffer length: expected 12 bytes, got 10 bytes"
        );
    }
}
