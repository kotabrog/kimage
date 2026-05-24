use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kimage::codecs::bmp::{
    self, BmpColorTableEntry, BmpDibHeader, BmpFileHeader, BmpImage, BmpInfoHeader,
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/bmp_indexed8_native_roundtrip.bmp";
const PNG_OUTPUT_PATH: &str = "target/examples/bmp_indexed8_native_roundtrip.png";
const PPM_OUTPUT_PATH: &str = "target/examples/bmp_indexed8_native_roundtrip.ppm";

fn main() -> kimage::Result<()> {
    fs::create_dir_all("target/examples")?;

    let image = indexed8_checker_bmp(WIDTH, HEIGHT)?;
    image.validate_file_layout()?;

    let path = Path::new(OUTPUT_PATH);
    let mut file = File::create(path)?;
    bmp::encode_native(&mut file, &image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = bmp::decode_native(&mut BufReader::new(file))?;

    assert_eq!(decoded, image);
    decoded.validate_file_layout()?;

    println!(
        "8-bit indexed BMP native roundtrip succeeded: {}",
        path.display()
    );
    try_convert_bmp_to_png(path, Path::new(PNG_OUTPUT_PATH));

    Ok(())
}

fn indexed8_checker_bmp(width: u32, height: u32) -> kimage::Result<BmpImage> {
    let color_table = vec![
        BmpColorTableEntry {
            blue: 255,
            green: 255,
            red: 255,
            reserved: 0,
        },
        BmpColorTableEntry {
            blue: 0,
            green: 0,
            red: 0,
            reserved: 0,
        },
        BmpColorTableEntry {
            blue: 32,
            green: 160,
            red: 240,
            reserved: 0,
        },
        BmpColorTableEntry {
            blue: 220,
            green: 80,
            red: 40,
            reserved: 0,
        },
    ];
    let row_size = aligned_indexed8_row_size(width);
    let pixel_array = checker_index_pixels(width, height, row_size);
    let pixel_offset = 14 + 40 + color_table.len() as u32 * 4;
    let file_size = pixel_offset + pixel_array.len() as u32;

    Ok(BmpImage {
        file_header: BmpFileHeader {
            file_size,
            reserved1: 0,
            reserved2: 0,
            pixel_offset,
        },
        dib_header: BmpDibHeader::BitmapInfoHeader(BmpInfoHeader {
            width: i32::try_from(width).map_err(|_| {
                kimage::ImageError::ImageDimensionsTooLarge {
                    width,
                    height,
                    bytes_per_pixel: 1,
                }
            })?,
            height: i32::try_from(height).map_err(|_| {
                kimage::ImageError::ImageDimensionsTooLarge {
                    width,
                    height,
                    bytes_per_pixel: 1,
                }
            })?,
            planes: 1,
            bits_per_pixel: 8,
            compression: 0,
            image_size: pixel_array.len() as u32,
            x_pixels_per_meter: 0,
            y_pixels_per_meter: 0,
            colors_used: color_table.len() as u32,
            important_colors: 0,
        }),
        color_masks: Vec::new(),
        color_table,
        pixel_array,
    })
}

fn aligned_indexed8_row_size(width: u32) -> usize {
    let row_len = width as usize;
    row_len.div_ceil(4) * 4
}

fn checker_index_pixels(width: u32, height: u32, row_size: usize) -> Vec<u8> {
    let row_len = width as usize;
    let padding_len = row_size - row_len;
    let mut pixels = Vec::with_capacity(row_size * height as usize);

    for y in (0..height).rev() {
        for x in 0..width {
            let checker = ((x / 8) + (y / 8)) % 4;
            pixels.push(checker as u8);
        }
        pixels.extend(std::iter::repeat_n(0, padding_len));
    }

    pixels
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
