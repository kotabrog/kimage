use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kimage::{
    EncodeFormat, Image, PixelFormat,
    codecs::{PamEncodeTupleType, pnm::PnmEncodeFormat},
    decode, encode,
};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const PNM_OUTPUT_PATH: &str = "target/examples/top_level_encode_roundtrip.ppm";
const PAM_OUTPUT_PATH: &str = "target/examples/top_level_encode_roundtrip.pam";
const PAM_PNG_OUTPUT_PATH: &str = "target/examples/top_level_encode_roundtrip_pam.png";
const BMP_OUTPUT_PATH: &str = "target/examples/top_level_encode_roundtrip.bmp";

fn main() -> kimage::Result<()> {
    fs::create_dir_all("target/examples")?;

    let pixels = gradient_rgb8(WIDTH, HEIGHT);
    let image = Image::new(WIDTH, HEIGHT, PixelFormat::Rgb8, pixels)?;

    roundtrip(
        &image,
        PNM_OUTPUT_PATH,
        EncodeFormat::Pnm(PnmEncodeFormat::PpmBinary),
    )?;
    roundtrip(
        &image,
        PAM_OUTPUT_PATH,
        EncodeFormat::Pam(PamEncodeTupleType::Rgb),
    )?;
    roundtrip(&image, BMP_OUTPUT_PATH, EncodeFormat::Bmp)?;

    println!("Top-level encode roundtrip succeeded:");
    println!("  {}", PNM_OUTPUT_PATH);
    println!("  {}", PAM_OUTPUT_PATH);
    println!("  {}", BMP_OUTPUT_PATH);
    try_convert_pam_to_png(Path::new(PAM_OUTPUT_PATH), Path::new(PAM_PNG_OUTPUT_PATH));

    Ok(())
}

fn roundtrip(image: &Image, output_path: &str, format: EncodeFormat) -> kimage::Result<()> {
    let path = Path::new(output_path);
    let mut file = File::create(path)?;
    encode(&mut file, image.as_view(), format)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = decode(&mut BufReader::new(file))?;

    assert_eq!(decoded.width, image.width);
    assert_eq!(decoded.height, image.height);
    assert_eq!(decoded.pixel_format, image.pixel_format);
    assert_eq!(decoded.data, image.data);

    Ok(())
}

fn gradient_rgb8(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);

    for y in 0..height {
        for x in 0..width {
            let red = scale_to_u8(x, width);
            let green = scale_to_u8(y, height);
            let blue = 128;

            pixels.extend_from_slice(&[red, green, blue]);
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

fn try_convert_pam_to_png(input: &Path, output: &Path) {
    let result = Command::new("pamtopng").arg(input).output();

    match result {
        Ok(result) if result.status.success() => {
            if let Err(error) = fs::write(output, result.stdout) {
                eprintln!("pamtopng succeeded but PNG write failed: {error}");
                return;
            }

            println!("PAM converted to PNG: {}", output.display());
        }
        Ok(result) => {
            let stderr = String::from_utf8_lossy(&result.stderr);
            eprintln!("pamtopng failed; PNG was not written: {}", stderr.trim());
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("pamtopng not found; PNG conversion skipped");
        }
        Err(error) => {
            eprintln!("pamtopng could not run; PNG conversion skipped: {error}");
        }
    }
}
