use std::fmt;

use crate::PixelFormat;

/// Error type used by image decoding, encoding, and buffer validation.
#[derive(Debug)]
pub enum ImageError {
    /// A lower-level IO operation failed.
    Io { source: std::io::Error },
    /// The image header is malformed.
    InvalidHeader { reason: &'static str },
    /// The image payload is malformed.
    InvalidData { reason: &'static str },
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

impl PartialEq for ImageError {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Io { source: a }, Self::Io { source: b }) => a.kind() == b.kind(),
            (Self::InvalidHeader { reason: a }, Self::InvalidHeader { reason: b }) => a == b,
            (Self::InvalidData { reason: a }, Self::InvalidData { reason: b }) => a == b,
            (
                Self::InvalidBufferLength {
                    expected: a_expected,
                    actual: a_actual,
                },
                Self::InvalidBufferLength {
                    expected: b_expected,
                    actual: b_actual,
                },
            ) => a_expected == b_expected && a_actual == b_actual,
            (
                Self::InvalidStride {
                    minimum: a_minimum,
                    actual: a_actual,
                },
                Self::InvalidStride {
                    minimum: b_minimum,
                    actual: b_actual,
                },
            ) => a_minimum == b_minimum && a_actual == b_actual,
            (
                Self::ImageDimensionsTooLarge {
                    width: a_width,
                    height: a_height,
                    bytes_per_pixel: a_bytes_per_pixel,
                },
                Self::ImageDimensionsTooLarge {
                    width: b_width,
                    height: b_height,
                    bytes_per_pixel: b_bytes_per_pixel,
                },
            ) => {
                a_width == b_width && a_height == b_height && a_bytes_per_pixel == b_bytes_per_pixel
            }
            (Self::UnsupportedFormat, Self::UnsupportedFormat) => true,
            (
                Self::UnsupportedPixelFormat { pixel_format: a },
                Self::UnsupportedPixelFormat { pixel_format: b },
            ) => a == b,
            _ => false,
        }
    }
}

/// Convenient result type for this crate.
pub type Result<T> = std::result::Result<T, ImageError>;

impl fmt::Display for ImageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { source } => write!(f, "io error: {source}"),
            Self::InvalidHeader { reason } => write!(f, "invalid image header: {reason}"),
            Self::InvalidData { reason } => write!(f, "invalid image data: {reason}"),
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

impl std::error::Error for ImageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source } => Some(source),
            _ => None,
        }
    }
}

impl From<std::io::Error> for ImageError {
    fn from(source: std::io::Error) -> Self {
        Self::Io { source }
    }
}

#[cfg(test)]
mod tests {
    use std::error::Error;
    use std::io::{Error as IoError, ErrorKind};

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

    #[test]
    fn io_error_preserves_source() {
        let error = ImageError::from(IoError::new(ErrorKind::PermissionDenied, "denied"));
        let source = error.source().unwrap().downcast_ref::<IoError>().unwrap();

        assert_eq!(source.kind(), ErrorKind::PermissionDenied);
        assert_eq!(source.to_string(), "denied");
    }
}
