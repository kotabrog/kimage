# kimage

`kimage` is a small Rust image IO crate built step by step with the Rust standard library.

The first goal is to define a minimal crate structure and then grow support from simple formats such as PPM and BMP before considering PNG.

## Current Format Support

The current implementations intentionally cover only small, early subsets of each format.

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
- native multi-image streams for binary PBM P4, PGM P5, and PPM P6

Unsupported:

- PAM P7

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

### BMP

Run the BMP roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example bmp_roundtrip
```

The example writes `target/examples/bmp_roundtrip.bmp`.
