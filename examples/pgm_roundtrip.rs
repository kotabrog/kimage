use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use kimage::{ImageView, PixelFormat, codecs::pgm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/pgm_roundtrip.pgm";

fn main() -> kimage::Result<()> {
    let pixels = gradient_gray8(WIDTH, HEIGHT);
    let stride = WIDTH as usize * PixelFormat::Gray8.bytes_per_pixel();
    let image = ImageView::new(WIDTH, HEIGHT, PixelFormat::Gray8, stride, &pixels)?;

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    pgm::encode(&mut file, image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = pgm::decode(&mut BufReader::new(file))?;

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);
    assert_eq!(decoded.pixel_format, PixelFormat::Gray8);
    assert_eq!(decoded.data, pixels);

    println!("PGM roundtrip succeeded: {}", path.display());

    Ok(())
}

fn gradient_gray8(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    let scale = width.saturating_add(height).saturating_sub(2);

    for y in 0..height {
        for x in 0..width {
            let value = ((x + y) * 255).checked_div(scale).unwrap_or(0) as u8;
            pixels.push(value);
        }
    }

    pixels
}
