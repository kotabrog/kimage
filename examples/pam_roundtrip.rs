use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kimage::{ImageView, PixelFormat, codecs::pam};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/pam_roundtrip.pam";
const PNG_OUTPUT_PATH: &str = "target/examples/pam_roundtrip.png";

fn main() -> kimage::Result<()> {
    let pixels = gradient_rgb8(WIDTH, HEIGHT);
    let stride = WIDTH as usize * PixelFormat::Rgb8.bytes_per_pixel();
    let image = ImageView::new(WIDTH, HEIGHT, PixelFormat::Rgb8, stride, &pixels)?;

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    pam::encode(&mut file, image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = pam::decode(&mut BufReader::new(file))?;

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);
    assert_eq!(decoded.pixel_format, PixelFormat::Rgb8);
    assert_eq!(decoded.data, pixels);

    println!("PAM roundtrip succeeded: {}", path.display());
    try_convert_to_png(path, Path::new(PNG_OUTPUT_PATH));

    Ok(())
}

fn try_convert_to_png(input: &Path, output: &Path) {
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
