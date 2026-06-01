# kimage

`kimage` is a small Rust image IO crate built step by step with the Rust standard library.

The first goal is to define a minimal crate structure and then grow support from simple formats such as PPM and BMP before considering PNG.

## Basic Usage

Use `kimage::decode` to read a supported image by detecting its magic number:

```rust
let image = kimage::decode(&mut reader)?;
```

Use `kimage::decode_native` when you need format-specific values such as Netpbm
`maxval`, PAM tuple metadata, or BMP header fields:

```rust
let native = kimage::decode_native(&mut reader)?;
let image = native.to_image()?;
```

Use `kimage::decode_all_native` to read multi-image binary PNM or PAM streams:

```rust
let images = kimage::decode_all_native(&mut reader)?;
```

Use `kimage::encode` with an explicit `EncodeFormat` to write a generic image
view:

```rust
kimage::encode(
    &mut writer,
    image.as_view(),
    kimage::EncodeFormat::Bmp(kimage::codecs::bmp::BmpEncodeOptions::default()),
)?;
```

Use `kimage::encode_native` and `kimage::encode_all_native` to write native
format-specific values:

```rust
kimage::encode_native(&mut writer, &native)?;
kimage::encode_all_native(&mut writer, &images)?;
```

For BMP, `encode_native` writes native header fields as provided instead of
normalizing them. Use `BmpImage::validate_file_layout` when you want to check
whether the BMP header fields and pixel array describe a consistent file layout
before writing.

For format-specific behavior, use the modules under `kimage::codecs`.

## Cargo Features

Default features enable all currently supported format families:

```toml
default = ["bmp", "netpbm"]
```

Available features:

- `bmp`: enables BMP codec support
- `netpbm`: enables PBM, PGM, PPM, PNM, and PAM codec support

## Current Format Support

The current implementations intentionally cover only small, early subsets of each format.

Top-level `kimage::decode` supports PBM P1/P4, PGM P2/P5, PPM P3/P6, PAM P7, and BMP.
Top-level `kimage::decode_native` supports PBM P1/P4, PGM P2/P5, PPM P3/P6, PAM P7, and BMP.
Top-level `kimage::decode_all_native` supports multi-image PBM P4, PGM P5, PPM P6, and PAM P7 streams.
Top-level `kimage::encode` supports PBM P1/P4, PGM P2/P5, PPM P3/P6, PAM P7, and BMP.
Top-level `kimage::encode_native` supports binary PBM P4, PGM P5, PPM P6, PAM P7, and BMP.
Top-level `kimage::encode_all_native` supports binary PBM P4, PGM P5, PPM P6, and PAM P7.

### PBM / PGM / PPM

Supported:

- ASCII PBM P1 expanded to `Gray8`
- ASCII PGM P2 with `maxval = 1..=65535`
- ASCII PPM P3 with `maxval = 1..=65535`
- binary PBM P4 expanded to `Gray8`
- binary PGM P5 with `maxval = 1..=65535`
- binary PPM P6 with `maxval = 1..=65535`
- PGM/PPM samples normalized to `Gray8` / `Rgb8` for `maxval < 256`
- PGM/PPM samples normalized to `Gray16` / `Rgb16` for `maxval >= 256`
- native PGM/PPM APIs that preserve `maxval` and sample values
- conversion APIs between `NetpbmImage` and `Image` / `ImageView`
- PNM decode APIs that auto-detect P1 through P6 and encode APIs that select P1 through P6
- normalized and native multi-image streams for binary PBM P4, PGM P5, and PPM P6

### PAM

Supported:

- PAM P7 native decode and encode
- PAM P7 normalized and native multi-image streams
- `BLACKANDWHITE`, `GRAYSCALE`, `RGB`, `BLACKANDWHITE_ALPHA`, `GRAYSCALE_ALPHA`, `RGB_ALPHA`, and unknown `TUPLTYPE` in native APIs
- `BLACKANDWHITE`, `GRAYSCALE`, `RGB`, `BLACKANDWHITE_ALPHA`, `GRAYSCALE_ALPHA`, and `RGB_ALPHA` conversion to `Image`
- explicit PAM tuple type selection for `ImageView` encode

Unsupported:

- unknown or missing `TUPLTYPE` conversion to `Image`

### BMP

Supported:

- uncompressed 24-bit bottom-up and top-down BMP
- generic decode for uncompressed 8-bit indexed BMP
- generic encode for uncompressed 8-bit indexed BMP with an explicit color table
- generic encode that automatically uses 8-bit indexed BMP when the input has
  256 or fewer colors, otherwise falling back to 24-bit BMP
- native decode and encode for uncompressed 8-bit indexed BMP
- `BITMAPINFOHEADER`
- native BMP APIs that preserve BMP header fields and pixel array data
- `BmpImage::validate_file_layout` for checking native BMP file layout consistency

Notes:

- BMP native encode preserves native fields as much as possible and is not a
  normalization API.
- BMP native APIs do not guarantee byte-for-byte roundtrips. Unknown gap bytes
  before the pixel array are not preserved.

Unsupported:

- generic decode for 1-bit and 4-bit palette BMP
- compressed BMP
- lossy palette quantization
- BMP bit depths other than 8-bit indexed BMP and 24-bit RGB BMP
- alpha channels
- color profiles and metadata

## Examples

### Top-level API

Run the top-level encode roundtrip example to write and read back PNM, PAM, and BMP files using `kimage::encode` and `kimage::decode`:

```sh
cargo run --example top_level_encode_roundtrip
```

The example writes `target/examples/top_level_encode_roundtrip.ppm`, `target/examples/top_level_encode_roundtrip.pam`, and `target/examples/top_level_encode_roundtrip.bmp`. If `pamtopng` is available, it also writes `target/examples/top_level_encode_roundtrip_pam.png`.

### PBM

Run the ASCII PBM P1 roundtrip example to write and read back a small black-and-white checker image:

```sh
cargo run --example pbm_ascii_roundtrip
```

The example writes `target/examples/pbm_ascii_roundtrip.pbm`.

Run the binary PBM P4 roundtrip example:

```sh
cargo run --example pbm_roundtrip
```

The example writes `target/examples/pbm_roundtrip.pbm`.

### PGM

Run the ASCII PGM P2 roundtrip example:

```sh
cargo run --example pgm_ascii_roundtrip
```

The example writes `target/examples/pgm_ascii_roundtrip.pgm`.

Run the binary PGM P5 roundtrip example to write and read back a small grayscale gradient image:

```sh
cargo run --example pgm_roundtrip
```

The example writes `target/examples/pgm_roundtrip.pgm`.

Run the 16-bit binary PGM P5 roundtrip example with `maxval = 65535`:

```sh
cargo run --example pgm16_roundtrip
```

The example writes `target/examples/pgm16_roundtrip.pgm`.

### PPM

Run the ASCII PPM P3 roundtrip example:

```sh
cargo run --example ppm_ascii_roundtrip
```

The example writes `target/examples/ppm_ascii_roundtrip.ppm`.

Run the binary PPM P6 roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example ppm_roundtrip
```

The example writes `target/examples/ppm_roundtrip.ppm`.

Run the 16-bit binary PPM P6 roundtrip example with `maxval = 65535`:

```sh
cargo run --example ppm16_roundtrip
```

The example writes `target/examples/ppm16_roundtrip.ppm`.

Run the binary PPM P6 multi-image roundtrip example:

```sh
cargo run --example ppm_multi_image_roundtrip
```

The example writes `target/examples/ppm_multi_image_roundtrip.ppm`.

### PNM

Run the PNM roundtrip example to write and read back a small RGB gradient image using PPM P6 output:

```sh
cargo run --example pnm_roundtrip
```

The example writes `target/examples/pnm_roundtrip.ppm`.

### PAM

Run the PAM P7 roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example pam_roundtrip
```

The example writes `target/examples/pam_roundtrip.pam`.

### BMP

Run the BMP roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example bmp_roundtrip
```

The example writes `target/examples/bmp_roundtrip.bmp`.

Run the native 8-bit indexed BMP roundtrip example to write and read back a
palette BMP:

```sh
cargo run --example bmp_indexed8_native_roundtrip
```

The example writes `target/examples/bmp_indexed8_native_roundtrip.bmp`. If
ImageMagick (`magick`) or Netpbm (`bmptoppm` and `pnmtopng`) is available, it
also writes `target/examples/bmp_indexed8_native_roundtrip.png`.
