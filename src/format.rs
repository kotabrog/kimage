use std::io::{Cursor, Read, Write};

use crate::codecs::{NetpbmImage, PamImage, bmp, pam, pbm, pgm, pnm, ppm};
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

/// A format-specific image representation that preserves native file values.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum NativeImage {
    Netpbm(NetpbmImage),
    Pam(PamImage),
}

impl NativeImage {
    /// Converts this native image into the generic normalized image buffer.
    pub fn to_image(&self) -> Result<Image> {
        match self {
            Self::Netpbm(image) => image.to_image(),
            Self::Pam(image) => image.to_image(),
        }
    }
}

/// Decodes an image by detecting its format from the input bytes.
pub fn decode<R: Read>(reader: &mut R) -> Result<Image> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    decode_from_slice(&data)
}

/// Decodes a native image by detecting its format from the input bytes.
///
/// BMP does not currently have a native representation and returns
/// `ImageError::UnsupportedFormat`.
pub fn decode_native<R: Read>(reader: &mut R) -> Result<NativeImage> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    decode_native_from_slice(&data)
}

/// Decodes all native images by detecting the stream format from the input bytes.
///
/// This supports multi-image PNM binary streams (P4, P5, P6) and PAM (P7).
/// Empty input returns an empty vector. Plain PNM formats (P1, P2, P3) and BMP
/// return `ImageError::UnsupportedFormat`.
pub fn decode_all_native<R: Read>(reader: &mut R) -> Result<Vec<NativeImage>> {
    let mut data = Vec::new();
    reader.read_to_end(&mut data)?;
    decode_all_native_from_slice(&data)
}

/// Encodes a native image by using the format represented by the image value.
pub fn encode_native<W: Write>(writer: &mut W, image: &NativeImage) -> Result<()> {
    match image {
        NativeImage::Netpbm(image) => pnm::encode_native(writer, image),
        NativeImage::Pam(image) => pam::encode_native(writer, image),
    }
}

/// Encodes native images as a multi-image stream.
///
/// All images must belong to the same native format family. Netpbm streams must
/// also use the same PBM, PGM, or PPM subformat.
pub fn encode_all_native<W: Write>(writer: &mut W, images: &[NativeImage]) -> Result<()> {
    let Some(first) = images.first() else {
        return Ok(());
    };

    let mut buffer = Vec::new();
    match first {
        NativeImage::Netpbm(_) => {
            let images = images
                .iter()
                .map(|image| match image {
                    NativeImage::Netpbm(image) => Ok(image.clone()),
                    NativeImage::Pam(_) => Err(ImageError::UnsupportedFormat),
                })
                .collect::<Result<Vec<_>>>()?;
            pnm::encode_all_native(&mut buffer, &images)?;
        }
        NativeImage::Pam(_) => {
            let images = images
                .iter()
                .map(|image| match image {
                    NativeImage::Pam(image) => Ok(image.clone()),
                    NativeImage::Netpbm(_) => Err(ImageError::UnsupportedFormat),
                })
                .collect::<Result<Vec<_>>>()?;
            pam::encode_all_native(&mut buffer, &images)?;
        }
    }

    writer.write_all(&buffer)?;
    Ok(())
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

fn decode_native_from_slice(data: &[u8]) -> Result<NativeImage> {
    match detect_format(data)? {
        ImageFormat::PbmAscii => Ok(NativeImage::Netpbm(pbm::decode_ascii_native(
            &mut Cursor::new(data),
        )?)),
        ImageFormat::PgmAscii => Ok(NativeImage::Netpbm(pgm::decode_ascii_native(
            &mut Cursor::new(data),
        )?)),
        ImageFormat::PpmAscii => Ok(NativeImage::Netpbm(ppm::decode_ascii_native(
            &mut Cursor::new(data),
        )?)),
        ImageFormat::PbmBinary => Ok(NativeImage::Netpbm(pbm::decode_native(&mut Cursor::new(
            data,
        ))?)),
        ImageFormat::PgmBinary => Ok(NativeImage::Netpbm(pgm::decode_native(&mut Cursor::new(
            data,
        ))?)),
        ImageFormat::PpmBinary => Ok(NativeImage::Netpbm(ppm::decode_native(&mut Cursor::new(
            data,
        ))?)),
        ImageFormat::Pam => Ok(NativeImage::Pam(pam::decode_native(&mut Cursor::new(
            data,
        ))?)),
        ImageFormat::Bmp => Err(ImageError::UnsupportedFormat),
    }
}

fn decode_all_native_from_slice(data: &[u8]) -> Result<Vec<NativeImage>> {
    if data.is_empty() {
        return Ok(Vec::new());
    }

    match detect_format(data)? {
        ImageFormat::PbmBinary | ImageFormat::PgmBinary | ImageFormat::PpmBinary => {
            pnm::decode_all_native(&mut Cursor::new(data)).map(|images| {
                images
                    .into_iter()
                    .map(NativeImage::Netpbm)
                    .collect::<Vec<_>>()
            })
        }
        ImageFormat::Pam => pam::decode_all_native(&mut Cursor::new(data))
            .map(|images| images.into_iter().map(NativeImage::Pam).collect::<Vec<_>>()),
        ImageFormat::PbmAscii
        | ImageFormat::PgmAscii
        | ImageFormat::PpmAscii
        | ImageFormat::Bmp => Err(ImageError::UnsupportedFormat),
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
    use crate::codecs::{NetpbmImage, PamTupleType};

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

    #[test]
    fn decode_native_detects_pbm_ascii() {
        let image = decode_native(&mut Cursor::new(b"P1\n1 1\n0\n")).unwrap();

        assert_eq!(
            image,
            NativeImage::Netpbm(NetpbmImage::Pbm {
                width: 1,
                height: 1,
                data: vec![0],
            })
        );
    }

    #[test]
    fn decode_native_detects_pgm_binary() {
        let image = decode_native(&mut Cursor::new(b"P5\n1 1\n15\n\x0f")).unwrap();

        assert_eq!(
            image,
            NativeImage::Netpbm(NetpbmImage::Pgm {
                width: 1,
                height: 1,
                maxval: 15,
                data: vec![15],
            })
        );
    }

    #[test]
    fn decode_native_detects_ppm_binary() {
        let image = decode_native(&mut Cursor::new(b"P6\n1 1\n255\n\xff\0\x80")).unwrap();

        assert_eq!(
            image,
            NativeImage::Netpbm(NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![255, 0, 128],
            })
        );
    }

    #[test]
    fn decode_native_detects_pam() {
        let input = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\xff\0\x80";
        let image = decode_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            image,
            NativeImage::Pam(PamImage {
                width: 1,
                height: 1,
                depth: 3,
                maxval: 255,
                tuple_type: Some(PamTupleType::Rgb),
                data: vec![255, 0, 128],
            })
        );
    }

    #[test]
    fn native_image_to_image_converts_wrapped_image() {
        let image = NativeImage::Netpbm(NetpbmImage::Pgm {
            width: 1,
            height: 1,
            maxval: 15,
            data: vec![15],
        })
        .to_image()
        .unwrap();

        assert_eq!(image.pixel_format, PixelFormat::Gray8);
        assert_eq!(image.data, [255]);
    }

    #[test]
    fn decode_native_rejects_bmp() {
        let input = [
            0x42, 0x4d, 0x3a, 0, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0, 0x28, 0, 0, 0, 1, 0, 0, 0, 1, 0,
            0, 0, 1, 0, 0x18, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0x80, 0, 0xff, 0,
        ];
        let error = decode_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_returns_empty_vec_for_empty_input() {
        let images = decode_all_native(&mut Cursor::new([])).unwrap();

        assert!(images.is_empty());
    }

    #[test]
    fn decode_all_native_detects_pbm_binary_stream() {
        let input = b"P4\n1 1\n\x80P4\n1 1\n\0";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NativeImage::Netpbm(NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![1],
                }),
                NativeImage::Netpbm(NetpbmImage::Pbm {
                    width: 1,
                    height: 1,
                    data: vec![0],
                }),
            ]
        );
    }

    #[test]
    fn decode_all_native_detects_pgm_binary_stream() {
        let input = b"P5\n1 1\n15\n\x0fP5\n1 1\n15\n\0";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NativeImage::Netpbm(NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 15,
                    data: vec![15],
                }),
                NativeImage::Netpbm(NetpbmImage::Pgm {
                    width: 1,
                    height: 1,
                    maxval: 15,
                    data: vec![0],
                }),
            ]
        );
    }

    #[test]
    fn decode_all_native_detects_ppm_binary_stream() {
        let input = b"P6\n1 1\n255\n\xff\0\x80P6\n1 1\n255\n\0\x80\xff";
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NativeImage::Netpbm(NetpbmImage::Ppm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![255, 0, 128],
                }),
                NativeImage::Netpbm(NetpbmImage::Ppm {
                    width: 1,
                    height: 1,
                    maxval: 255,
                    data: vec![0, 128, 255],
                }),
            ]
        );
    }

    #[test]
    fn decode_all_native_detects_pam_stream() {
        let image = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\xff\0\x80";
        let input = [image.as_slice(), image.as_slice()].concat();
        let images = decode_all_native(&mut Cursor::new(input)).unwrap();

        assert_eq!(
            images,
            [
                NativeImage::Pam(PamImage {
                    width: 1,
                    height: 1,
                    depth: 3,
                    maxval: 255,
                    tuple_type: Some(PamTupleType::Rgb),
                    data: vec![255, 0, 128],
                }),
                NativeImage::Pam(PamImage {
                    width: 1,
                    height: 1,
                    depth: 3,
                    maxval: 255,
                    tuple_type: Some(PamTupleType::Rgb),
                    data: vec![255, 0, 128],
                }),
            ]
        );
    }

    #[test]
    fn decode_all_native_rejects_plain_pnm() {
        let error = decode_all_native(&mut Cursor::new(b"P1\n1 1\n0\n")).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_rejects_bmp() {
        let input = [
            0x42, 0x4d, 0x3a, 0, 0, 0, 0, 0, 0, 0, 0x36, 0, 0, 0, 0x28, 0, 0, 0, 1, 0, 0, 0, 1, 0,
            0, 0, 1, 0, 0x18, 0, 0, 0, 0, 0, 4, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            0, 0, 0x80, 0, 0xff, 0,
        ];
        let error = decode_all_native(&mut Cursor::new(input)).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn decode_all_native_rejects_mixed_pnm_binary_stream() {
        let error = decode_all_native(&mut Cursor::new(b"P5\n1 1\n255\n\0P6\n1 1\n255\n\0\0\0"))
            .unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
    }

    #[test]
    fn encode_native_writes_netpbm_image() {
        let image = NativeImage::Netpbm(NetpbmImage::Pgm {
            width: 1,
            height: 1,
            maxval: 15,
            data: vec![15],
        });
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(decode_native(&mut Cursor::new(output)).unwrap(), image);
    }

    #[test]
    fn encode_native_writes_pam_image() {
        let image = NativeImage::Pam(PamImage {
            width: 1,
            height: 1,
            depth: 3,
            maxval: 255,
            tuple_type: Some(PamTupleType::Rgb),
            data: vec![255, 0, 128],
        });
        let mut output = Vec::new();

        encode_native(&mut output, &image).unwrap();

        assert_eq!(decode_native(&mut Cursor::new(output)).unwrap(), image);
    }

    #[test]
    fn encode_all_native_writes_empty_stream_for_empty_slice() {
        let mut output = Vec::new();

        encode_all_native(&mut output, &[]).unwrap();

        assert!(output.is_empty());
    }

    #[test]
    fn encode_all_native_writes_netpbm_stream() {
        let images = [
            NativeImage::Netpbm(NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![255, 0, 128],
            }),
            NativeImage::Netpbm(NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![0, 128, 255],
            }),
        ];
        let mut output = Vec::new();

        encode_all_native(&mut output, &images).unwrap();

        assert_eq!(decode_all_native(&mut Cursor::new(output)).unwrap(), images);
    }

    #[test]
    fn encode_all_native_writes_pam_stream() {
        let image = NativeImage::Pam(PamImage {
            width: 1,
            height: 1,
            depth: 3,
            maxval: 255,
            tuple_type: Some(PamTupleType::Rgb),
            data: vec![255, 0, 128],
        });
        let images = [image.clone(), image];
        let mut output = Vec::new();

        encode_all_native(&mut output, &images).unwrap();

        assert_eq!(decode_all_native(&mut Cursor::new(output)).unwrap(), images);
    }

    #[test]
    fn encode_all_native_rejects_mixed_native_families_without_writing() {
        let images = [
            NativeImage::Netpbm(NetpbmImage::Pgm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![128],
            }),
            NativeImage::Pam(PamImage {
                width: 1,
                height: 1,
                depth: 1,
                maxval: 255,
                tuple_type: Some(PamTupleType::Grayscale),
                data: vec![128],
            }),
        ];
        let mut output = b"unchanged".to_vec();

        let error = encode_all_native(&mut output, &images).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
        assert_eq!(output, b"unchanged");
    }

    #[test]
    fn encode_all_native_rejects_mixed_netpbm_subformats_without_writing() {
        let images = [
            NativeImage::Netpbm(NetpbmImage::Pgm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![128],
            }),
            NativeImage::Netpbm(NetpbmImage::Ppm {
                width: 1,
                height: 1,
                maxval: 255,
                data: vec![128, 0, 255],
            }),
        ];
        let mut output = b"unchanged".to_vec();

        let error = encode_all_native(&mut output, &images).unwrap_err();

        assert_eq!(error, ImageError::UnsupportedFormat);
        assert_eq!(output, b"unchanged");
    }
}
