# kimage

`kimage` is a small Rust image IO crate built step by step with the Rust standard library.

The first goal is to define a minimal crate structure and then grow support from simple formats such as PPM and BMP before considering PNG.

## Current Format Support

The current implementations intentionally cover only small, early subsets of each format.

### PPM / PGM

Supported:

- binary PPM P6 with `Rgb8` and `maxval = 255`
- binary PGM P5 with `Gray8` and `maxval = 255`

Unsupported:

- ASCII Netpbm formats such as PPM P3 and PGM P2
- 16-bit PPM/PGM samples
- PBM

### BMP

Supported:

- uncompressed 24-bit bottom-up BMP
- `BITMAPINFOHEADER`

Unsupported:

- top-down BMP
- palette BMP
- compressed BMP
- BMP bit depths other than 24-bit
- alpha channels
- color profiles and metadata

## Examples

### PPM

Run the PPM roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example ppm_roundtrip
```

The example writes `target/examples/ppm_roundtrip.ppm`.

### PGM

Run the PGM roundtrip example to write and read back a small grayscale gradient image:

```sh
cargo run --example pgm_roundtrip
```

The example writes `target/examples/pgm_roundtrip.pgm`.

### BMP

Run the BMP roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example bmp_roundtrip
```

The example writes `target/examples/bmp_roundtrip.bmp`.
