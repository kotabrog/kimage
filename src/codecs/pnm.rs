use std::io::{Cursor, Read};

use crate::codecs::{pbm, pgm, ppm};
use crate::{Image, ImageError, Result};

use super::NetpbmImage;

/// Decodes a PBM, PGM, or PPM image by detecting the P1..P6 magic number.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    Image::try_from(decode_native(reader)?)
}

/// Decodes a native PBM, PGM, or PPM image by detecting the P1..P6 magic number.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NetpbmImage> {
    let data = read_all(reader)?;

    match magic(&data)? {
        b"P1" => pbm::decode_ascii_native(&mut Cursor::new(data)),
        b"P2" => pgm::decode_ascii_native(&mut Cursor::new(data)),
        b"P3" => ppm::decode_ascii_native(&mut Cursor::new(data)),
        b"P4" => pbm::decode_native(&mut Cursor::new(data)),
        b"P5" => pgm::decode_native(&mut Cursor::new(data)),
        b"P6" => ppm::decode_native(&mut Cursor::new(data)),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

/// Decodes all native images from a binary PBM, PGM, or PPM multi-image stream.
pub fn decode_all_native<R: Read>(reader: &mut R) -> Result<Vec<NetpbmImage>> {
    let data = read_all(reader)?;
    if data.is_empty() {
        return Ok(Vec::new());
    }

    match magic(&data)? {
        b"P4" => pbm::decode_all_native(&mut Cursor::new(data)),
        b"P5" => pgm::decode_all_native(&mut Cursor::new(data)),
        b"P6" => ppm::decode_all_native(&mut Cursor::new(data)),
        b"P1" | b"P2" | b"P3" => Err(ImageError::UnsupportedFormat),
        _ => Err(ImageError::UnsupportedFormat),
    }
}

fn read_all<R: Read>(reader: &mut R) -> Result<Vec<u8>> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    Ok(data)
}

fn magic(data: &[u8]) -> Result<&[u8]> {
    if data.len() < 2 {
        return Err(ImageError::UnsupportedFormat);
    }

    Ok(&data[..2])
}

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::PixelFormat;

    use super::*;

    #[test]
    fn decode_native_detects_pbm_p1() {
        let image = decode_native(&mut Cursor::new(b"P1\n2 1\n0 1\n")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pbm {
                width: 2,
                height: 1,
                data: vec![0, 1]
            }
        );
    }

    #[test]
    fn decode_native_detects_pgm_p2() {
        let image = decode_native(&mut Cursor::new(b"P2\n2 1\n15\n0 15\n")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 15,
                data: vec![0, 15]
            }
        );
    }

    #[test]
    fn decode_native_detects_ppm_p3() {
        let image = decode_native(&mut Cursor::new(b"P3\n1 1\n15\n15 0 5\n")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 15,
                data: vec![15, 0, 5]
            }
        );
    }

    #[test]
    fn decode_native_detects_pbm_p4() {
        let image = decode_native(&mut Cursor::new(b"P4\n2 1\n\x80")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pbm {
                width: 2,
                height: 1,
                data: vec![1, 0]
            }
        );
    }

    #[test]
    fn decode_native_detects_pgm_p5() {
        let image = decode_native(&mut Cursor::new(b"P5\n2 1\n255\n\0\xff")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Pgm {
                width: 2,
                height: 1,
                maxval: 255,
                data: vec![0, 255]
            }
        );
    }

    #[test]
    fn decode_native_detects_ppm_p6() {
        let image = decode_native(&mut Cursor::new(b"P6\n1 1\n255\n\xff\0\x80")).unwrap();

        assert_eq!(
            image,
            NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![255, 0, 128]
            }
        );
    }

    #[test]
    fn decode_returns_normalized_image() {
        let image = decode(&mut Cursor::new(b"P3\n1 1\n15\n15 0 5\n")).unwrap();

        assert_eq!(image.width, 1);
        assert_eq!(image.height, 1);
        assert_eq!(image.pixel_format, PixelFormat::Rgb8);
        assert_eq!(image.data, [255, 0, 85]);
    }

    #[test]
    fn decode_all_native_reads_pbm_p4_multi_image_stream() {
        let input = b"P4\n1 1\n\0P4\n1 1\n\x80";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![0]
                },
                NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![1]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_reads_pgm_p5_multi_image_stream() {
        let input = b"P5\n1 1\n255\n\0P5\n1 1\n255\n\xff";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![0]
                },
                NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![255]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_reads_ppm_p6_multi_image_stream() {
        let input = b"P6\n1 1\n255\n\0\0\0P6\n1 1\n255\n\xff\xff\xff";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NetpbmImage::Ppm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![0, 0, 0]
                },
                NetpbmImage::Ppm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![255, 255, 255]
                }
            ]
        );
    }

    #[test]
    fn decode_all_native_returns_empty_vec_for_empty_input() {
        let images = decode_all_native(&mut Cursor::new([])).unwrap();

        assert!(images.is_empty());
    }

    #[test]
    fn decode_all_native_rejects_plain_formats() {
        let error = decode_all_native(&mut Cursor::new(b"P1\n1 1\n0\n")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_rejects_mixed_subformat_stream() {
        let error = decode_all_native(&mut Cursor::new(b"P5\n1 1\n255\n\0P6\n1 1\n255\n\0\0\0"))
            .unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_unsupported_magic() {
        let error = decode_native(&mut Cursor::new(b"P7\n")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_rejects_short_input() {
        let error = decode_native(&mut Cursor::new(b"P")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }
}
