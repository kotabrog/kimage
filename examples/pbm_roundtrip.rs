use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use kimage::{ImageView, PixelFormat, codecs::pbm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/pbm_roundtrip.pbm";

fn main() -> kimage::Result<()> {
    let pixels = checker_gray8(WIDTH, HEIGHT);
    let stride = WIDTH as usize * PixelFormat::Gray8.bytes_per_pixel();
    let image = ImageView::new(WIDTH, HEIGHT, PixelFormat::Gray8, stride, &pixels)?;

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    pbm::encode(&mut file, image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = pbm::decode(&mut BufReader::new(file))?;

    assert_eq!(decoded.width, WIDTH);
    assert_eq!(decoded.height, HEIGHT);
    assert_eq!(decoded.pixel_format, PixelFormat::Gray8);
    assert_eq!(decoded.data, pixels);

    println!("PBM roundtrip succeeded: {}", path.display());

    Ok(())
}

fn checker_gray8(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize);

    for y in 0..height {
        for x in 0..width {
            let value = if (x / 8 + y / 8) % 2 == 0 { 255 } else { 0 };
            pixels.push(value);
        }
    }

    pixels
}
