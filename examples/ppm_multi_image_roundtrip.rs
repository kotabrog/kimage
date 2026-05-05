use std::fs::{self, File};
use std::io::BufReader;
use std::path::Path;
use std::process::Command;

use kimage::codecs::{NetpbmImage, ppm};

const WIDTH: u32 = 64;
const HEIGHT: u32 = 64;
const OUTPUT_PATH: &str = "target/examples/ppm_multi_image_roundtrip.ppm";
const SPLIT_PATTERN: &str = "target/examples/frame_%d.ppm";

fn main() -> kimage::Result<()> {
    let images = vec![
        NetpbmImage::Ppm {
            width: WIDTH,
            height: HEIGHT,
            maxval: 255,
            data: gradient_rgb8(WIDTH, HEIGHT, [0, 0, 128]),
        },
        NetpbmImage::Ppm {
            width: WIDTH,
            height: HEIGHT,
            maxval: 255,
            data: gradient_rgb8(WIDTH, HEIGHT, [128, 0, 0]),
        },
    ];

    let path = Path::new(OUTPUT_PATH);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = File::create(path)?;
    ppm::encode_all_native(&mut file, &images)?;
    drop(file);

    let file = File::open(path)?;
    let decoded = ppm::decode_all_native(&mut BufReader::new(file))?;

    assert_eq!(decoded, images);

    println!("PPM multi-image roundtrip succeeded: {}", path.display());
    split_with_pamsplit(path);

    Ok(())
}

fn split_with_pamsplit(path: &Path) {
    match Command::new("pamsplit")
        .arg(path)
        .arg(SPLIT_PATTERN)
        .status()
    {
        Ok(status) if status.success() => {
            println!("pamsplit wrote split frames: {SPLIT_PATTERN}");
        }
        Ok(status) => {
            println!(
                "pamsplit was found but failed with status {status}; split frames were not written"
            );
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            println!("pamsplit was not found; split frames were not written");
        }
        Err(error) => {
            println!("pamsplit could not run: {error}; split frames were not written");
        }
    }
}

fn gradient_rgb8(width: u32, height: u32, base: [u8; 3]) -> Vec<u8> {
    let mut pixels = Vec::with_capacity(width as usize * height as usize * 3);

    for y in 0..height {
        for x in 0..width {
            let red = base[0].saturating_add(scale_to_u8(x, width) / 2);
            let green = base[1].saturating_add(scale_to_u8(y, height) / 2);
            let blue = base[2];

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
