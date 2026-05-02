# Codex Project Overview

## 目的

このプロジェクトでは、Rust の標準ライブラリを中心に、画像ファイルを読み書きする小さな画像IOクレートを自作する。

最初から PNG や JPEG の完全対応を目指すのではなく、単純な形式から段階的に実装し、画像IOに必要な内部設計を理解しながら育てる。

## 最初の到達点

まずは次の範囲を MVP とする。

- `Image` / `ImageView` / `PixelFormat` などの共通画像型を用意する
- endian 読み書きなどの小さな binary IO helper を用意する
- PPM P6 の読み込みと保存に対応する
- 必要に応じて PGM P5、BMP へ広げる

初期のピクセル形式は `Rgb8` を中心にし、必要になってから `Gray8` や `Rgba8` を追加する。

## 設計方針

画像データは `Vec<u8>` 単体では扱わず、幅、高さ、ピクセル形式を合わせて保持する。

想定する中心型は次のようなもの。

```rust
pub struct Image {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub data: Vec<u8>,
}

pub struct ImageView<'a> {
    pub width: u32,
    pub height: u32,
    pub pixel_format: PixelFormat,
    pub stride: usize,
    pub data: &'a [u8],
}

pub enum PixelFormat {
    Gray8,
    Rgb8,
    Rgba8,
}
```

読み込みでは所有する `Image` を返し、保存では利用者の既存バッファを借りられるように `ImageView` を受け取る設計を基本にする。

## モジュール構成の案

初期構成は次のように小さく始める。

```text
src/
    lib.rs
    image.rs
    error.rs
    io.rs
    codecs/
        mod.rs
        ppm.rs
        bmp.rs
```

PNG 対応へ進む段階で、必要に応じて次を追加する。

```text
src/
    checksum.rs
    bitstream.rs
    zlib.rs
    deflate.rs
    codecs/
        png/
            mod.rs
            chunk.rs
            filter.rs
            encoder.rs
            decoder.rs
```

## 実装順序

推奨する順序は次の通り。

1. 共通画像型とエラー型を定義する
2. binary IO helper を実装する
3. PPM P6 encoder / decoder を実装する
4. PGM P5 に対応して `Gray8` を追加する
5. BMP encoder / decoder を実装する
6. PNG の前準備として CRC32 と Adler-32 を実装する
7. PNG encoder の最小版を実装する
8. PNG decoder の最小版を実装する

PNG の最小版では、最初は filter type 0 と deflate stored block のみを扱う方針でよい。

## 後回しにするもの

初期段階では次の対応は行わない。

- JPEG
- TIFF
- AVIF
- ICC color management
- serde 対応
- rayon による並列化
- bytemuck 的な高速 cast
- 本格的な deflate 圧縮

これらは PPM/BMP と共通画像型が固まった後に検討する。
