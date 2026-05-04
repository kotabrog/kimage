use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use kimage::codecs::{NetpbmImage, pgm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const MAXVAL: u16 = 65_535;
const OUTPUT_PATH: &str = "target/examples/pgm16_roundtrip.pgm";

fn main() -> kimage::Result<()> {
    let pixels = gradient_gray16(WIDTH, HEIGHT);
    let image = NetpbmImage::Pgm {
        width: WIDTH,
        height: HEIGHT,
        maxval: MAXVAL,
        data: pixels,
    };

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    pgm::encode_native(&mut file, &image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = pgm::decode_native(&mut BufReader::new(file))?;

    assert_eq!(decoded, image);

    println!("16-bit PGM roundtrip succeeded: {}", path.display());

    Ok(())
}

fn gradient_gray16(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 2);
    let scale = width.saturating_add(height).saturating_sub(2);

    for y in 0..height {
        for x in 0..width {
            let value = ((x + y) * u32::from(MAXVAL))
                .checked_div(scale)
                .unwrap_or(0) as u16;
            pixels.extend_from_slice(&value.to_le_bytes());
        }
    }

    pixels
}
