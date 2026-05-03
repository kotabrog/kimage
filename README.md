# kimage

`kimage` is a small Rust image IO crate built step by step with the Rust standard library.

The first goal is to define a minimal crate structure and then grow support from simple formats such as PPM and BMP before considering PNG.

## Current Format Support

The current implementations intentionally cover only small, early subsets of each format.

### PPM / PGM

Supported:

- binary PPM P6 with `Rgb8` and `maxval = 255`
- binary PGM P5 with `Gray8` and `maxval = 255`
- ASCII PPM P3 with `Rgb8` and `maxval = 255`
- ASCII PGM P2 with `Gray8` and `maxval = 255`
- binary PBM P4 expanded to `Gray8`
- ASCII PBM P1 expanded to `Gray8`

Unsupported:

- 16-bit PPM/PGM samples
- `maxval` values other than 255 for PPM/PGM

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

Run the binary PPM P6 roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example ppm_roundtrip
```

The example writes `target/examples/ppm_roundtrip.ppm`.

Run the ASCII PPM P3 roundtrip example:

```sh
cargo run --example ppm_ascii_roundtrip
```

The example writes `target/examples/ppm_ascii_roundtrip.ppm`.

### PGM

Run the binary PGM P5 roundtrip example to write and read back a small grayscale gradient image:

```sh
cargo run --example pgm_roundtrip
```

The example writes `target/examples/pgm_roundtrip.pgm`.

Run the ASCII PGM P2 roundtrip example:

```sh
cargo run --example pgm_ascii_roundtrip
```

The example writes `target/examples/pgm_ascii_roundtrip.pgm`.

### PBM

Run the binary PBM P4 roundtrip example to write and read back a small black-and-white checker image:

```sh
cargo run --example pbm_roundtrip
```

The example writes `target/examples/pbm_roundtrip.pbm`.

Run the ASCII PBM P1 roundtrip example:

```sh
cargo run --example pbm_ascii_roundtrip
```

The example writes `target/examples/pbm_ascii_roundtrip.pbm`.

### BMP

Run the BMP roundtrip example to write and read back a small RGB gradient image:

```sh
cargo run --example bmp_roundtrip
```

The example writes `target/examples/bmp_roundtrip.bmp`.
