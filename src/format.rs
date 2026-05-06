use std::io::{Cursor, Read};

use crate::codecs::{bmp, pam, pbm, pgm, ppm};
use crate::{Image, ImageError, Result};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ImageFormat {
    PbmAscii,
    PgmAscii,
    PpmAscii,
    PbmBinary,
    PgmBinary,
    PpmBinary,
    Pam,
    Bmp,
}

/// Decodes an image by detecting its format from the input bytes.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    decode_from_slice(&data)
}

fn decode_from_slice(data: &[u8]) -> Result<Image> {
    match detect_format(data)? {
        ImageFormat::PbmAscii => pbm::decode_ascii(&mut Cursor::new(data)),
        ImageFormat::PgmAscii => pgm::decode_ascii(&mut Cursor::new(data)),
        ImageFormat::PpmAscii => ppm::decode_ascii(&mut Cursor::new(data)),
        ImageFormat::PbmBinary => pbm::decode(&mut Cursor::new(data)),
        ImageFormat::PgmBinary => pgm::decode(&mut Cursor::new(data)),
        ImageFormat::PpmBinary => ppm::decode(&mut Cursor::new(data)),
        ImageFormat::Pam => pam::decode(&mut Cursor::new(data)),
        ImageFormat::Bmp => bmp::decode(&mut Cursor::new(data)),
    }
}

fn detect_format(data: &[u8]) -> Result<ImageFormat> {
    match data {
        [b'P', b'1', ..] => Ok(ImageFormat::PbmAscii),
        [b'P', b'2', ..] => Ok(ImageFormat::PgmAscii),
        [b'P', b'3', ..] => Ok(ImageFormat::PpmAscii),
        [b'P', b'4', ..] => Ok(ImageFormat::PbmBinary),
        [b'P', b'5', ..] => Ok(ImageFormat::PgmBinary),
        [b'P', b'6', ..] => Ok(ImageFormat::PpmBinary),
        [b'P', b'7', ..] => Ok(ImageFormat::Pam),
        [b'B', b'M', ..] => Ok(ImageFormat::Bmp),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::PixelFormat;

    use super::*;

    #[test]
    fn decode_detects_pbm_ascii() {
        let image = decode(&mut Cursor::new(b"P1\n1 1\n0\n")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [255]);
    }

    #[test]
    fn decode_detects_pgm_ascii() {
        let image = decode(&mut Cursor::new(b"P2\n1 1\n15\n15\n")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [255]);
    }

    #[test]
    fn decode_detects_ppm_ascii() {
        let image = decode(&mut Cursor::new(b"P3\n1 1\n15\n15 0 5\n")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 85]);
    }

    #[test]
    fn decode_detects_pbm_binary() {
        let image = decode(&mut Cursor::new(b"P4\n1 1\n\x80")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [0]);
    }

    #[test]
    fn decode_detects_pgm_binary() {
        let image = decode(&mut Cursor::new(b"P5\n1 1\n255\n\x80")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [128]);
    }

    #[test]
    fn decode_detects_ppm_binary() {
        let image = decode(&mut Cursor::new(b"P6\n1 1\n255\n\xff\0\x80")).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 128]);
    }

    #[test]
    fn decode_detects_pam() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\xff\0\x80";
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 128]);
    }

    #[test]
    fn decode_detects_bmp() {
        let input = [
            0x42, 0x4d, 0x3a, 0, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0, 0x28, 0, 0, 0, 1, 0, 0, 0, 1, 0,
            0, 0, 1, 0, 0x18, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0x80, 0, 0xff, 0,
        ];
        let image = decode(&mut Cursor::new(input)).unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 128]);
    }

    #[test]
    fn decode_rejects_empty_input() {
        let error = decode(&mut Cursor::new([])).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_short_magic() {
        let error = decode(&mut Cursor::new(b"P")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_unknown_magic() {
        let error = decode(&mut Cursor::new(b"ZZ")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }
}
