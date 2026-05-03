# kimage

`kimage` is a small Rust image IO crate built step by step with the Rust standard library.

The first goal is to define a minimal crate structure and then grow support from simple formats such as PPM and BMP before considering PNG.

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
