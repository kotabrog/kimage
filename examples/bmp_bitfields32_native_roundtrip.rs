use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kimage::PixelFormat;
use kimage::codecs::bmp::{self, BmpDibHeader, BmpFileHeader, BmpImage, BmpInfoHeader};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/bmp_bitfields32_native_roundtrip.bmp";
const PNG_OUTPUT_PATH: &str = "target/examples/bmp_bitfields32_native_roundtrip.png";
const PPM_OUTPUT_PATH: &str = "target/examples/bmp_bitfields32_native_roundtrip.ppm";

fn main() -> kimage::Result<()> {
    fs::create_dir_all("target/examples")?;

    let image = bitfields32_gradient_bmp(WIDTH, HEIGHT)?;
    image.validate_file_layout()?;

    let path = Path::new(OUTPUT_PATH);
    let mut file = File::create(path)?;
    bmp::encode_native(&mut file, &image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = bmp::decode_native(&mut BufReader::new(file))?;

    assert_eq!(decoded, image);
    decoded.validate_file_layout()?;

    let generic = decoded.to_image()?;
    assert_eq!(generic.width, WIDTH);
    assert_eq!(generic.height, HEIGHT);
    assert_eq!(generic.pixel_format, PixelFormat::Rgb8);

    println!(
        "32-bit BI_BITFIELDS BMP native roundtrip succeeded: {}",
        path.display()
    );
    try_convert_bmp_to_png(path, Path::new(PNG_OUTPUT_PATH));

    Ok(())
}

fn bitfields32_gradient_bmp(width: u32, height: u32) -> kimage::Result<BmpImage> {
    let row_size = width as usize * 4;
    let pixel_array = bitfields32_gradient_pixels(width, height);
    let pixel_offset = 14 + 40 + 3 * 4;
    let file_size = pixel_offset + pixel_array.len() as u32;

    Ok(BmpImage {
        file_header: BmpFileHeader {
            file_size,
            reserved1: 0,
            reserved2: 0,
            pixel_offset,
        },
        dib_header: BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
            width: bmp_i32_dimension(width, width, height)?,
            height: bmp_i32_dimension(height, width, height)?,
            planes: 1,
            bits_per_pixel: 32,
            compression: 3,
            image_size: (row_size * height as usize) as u32,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: 0,
            important_colors: 0,
        }),
        color_masks: vec![0x00ff0000, 0x0000ff00, 0x000000ff],
        color_table: Vec::new(),
        pixel_array,
    })
}

fn bitfields32_gradient_pixels(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 4);

    for y in (0..height).rev() {
        for x in 0..width {
            let red = scale_to_u8(x, width);
            let green = scale_to_u8(y, height);
            let blue = if ((x / 8) + (y / 8)) % 2 == 0 {
                224
            } else {
                48
            };

            pixels.extend_from_slice(&[blue, green, red, 0]);
        }
    }

    pixels
}

fn scale_to_u8(value: u32, limit: u32) -> u8 {
    if limit <= 1 {
        return 0;
    }

    ((value * 255) / (limit - 1)) as u8
}

fn bmp_i32_dimension(value: u32, width: u32, height: u32) -> kimage::Result<i32> {
    i32::try_from(value).map_err(|_| kimage::ImageError::ImageDimensionsTooLarge {
        width,
        height,
        bytes_per_pixel: PixelFormat::Rgb8.bytes_per_pixel(),
    })
}

fn try_convert_bmp_to_png(input: &Path, output: &Path) {
    if try_convert_with_magick(input, output) {
        return;
    }

    if try_convert_with_netpbm(input, output) {
        return;
    }

    println!("No BMP-to-PNG converter found; PNG conversion skipped");
}

fn try_convert_with_magick(input: &Path, output: &Path) -> bool {
    match Command::new("magick").arg(input).arg(output).output() {
        Ok(result) if result.status.success() => {
            println!("BMP converted to PNG: {}", output.display());
            true
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            eprintln!("magick failed; trying another converter: {}", stderr.trim());
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            eprintln!("magick could not run; trying another converter: {error}");
            false
        }
    }
}

fn try_convert_with_netpbm(input: &Path, output: &Path) -> bool {
    let ppm = Path::new(PPM_OUTPUT_PATH);
    let bmptoppm = Command::new("bmptoppm").arg(input).output();
    let Ok(bmptoppm) = bmptoppm else {
        return false;
    };

    if !bmptoppm.status.success() {
        let stderr = String::from_utf8_lossy(&bmptoppm.stderr);
        eprintln!("bmptoppm failed; PNG conversion skipped: {}", stderr.trim());
        return false;
    }

    if let Err(error) = fs::write(ppm, bmptoppm.stdout) {
        eprintln!("bmptoppm succeeded but PPM write failed: {error}");
        return false;
    }

    let pnmtopng = Command::new("pnmtopng").arg(ppm).output();
    match pnmtopng {
        Ok(result) if result.status.success() => {
            if let Err(error) = fs::write(output, result.stdout) {
                eprintln!("pnmtopng succeeded but PNG write failed: {error}");
                return false;
            }

            println!("BMP converted to PNG: {}", output.display());
            true
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            eprintln!("pnmtopng failed; PNG was not written: {}", stderr.trim());
            false
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => false,
        Err(error) => {
            eprintln!("pnmtopng could not run; PNG conversion skipped: {error}");
            false
        }
    }
}
