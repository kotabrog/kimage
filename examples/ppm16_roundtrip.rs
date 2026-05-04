use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;

use kimage::codecs::{NetpbmImage, ppm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const MAXVAL: u16 = 65_535;
const OUTPUT_PATH: &str = "target/examples/ppm16_roundtrip.ppm";

fn main() -> kimage::Result<()> {
    let pixels = gradient_rgb16(WIDTH, HEIGHT);
    let image = NetpbmImage::Ppm {
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
    ppm::encode_native(&mut file, &image)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = ppm::decode_native(&mut BufReader::new(file))?;

    assert_eq!(decoded, image);

    println!("16-bit PPM roundtrip succeeded: {}", path.display());

    Ok(())
}

fn gradient_rgb16(width: u32, height: u32) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 6);

    for y in 0..height {
        for x in 0..width {
            let red = scale_to_u16(x, width);
            let green = scale_to_u16(y, height);
            let blue = MAXVAL / 2;

            pixels.extend_from_slice(&red.to_le_bytes());
            pixels.extend_from_slice(&green.to_le_bytes());
            pixels.extend_from_slice(&blue.to_le_bytes());
        }
    }

    pixels
}

fn scale_to_u16(value: u32, limit: u32) -> u16 {
    if limit <= 1 {
        return 0;
    }

    ((value * u32::from(MAXVAL)) / (limit - 1)) as u16
}
