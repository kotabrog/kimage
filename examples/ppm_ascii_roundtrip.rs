use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use kimage::{ImageView, PixelFormat, codecs::ppm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/ppm_ascii_roundtrip.ppm";

fn main() -> kimage::Result<()> {
    let pixels = gradient_rgb8(WIDTH, HEIGHT);
    let stride = WIDTH as usize * PixelFormat::Rgb8.bytes_per_pixel();
    let image = ImageView::new(WIDTH, HEIGHT, PixelFormat::Rgb8, stride, &pixels)?;

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    ppm::encode_ascii(&mut file, image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = ppm::decode_ascii(&mut BufReader::new(file))?;

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);
    assert_eq!(decoded.pixel_format, PixelFormat::Rgb8);
    assert_eq!(decoded.data, pixels);

    println!("ASCII PPM roundtrip succeeded: {}", path.display());

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
