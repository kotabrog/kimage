# Codex Project Overview

## 目的

このプロジェクトでは、Rust の標準ライブラリを中心に、画像ファイルを読み書きする小さな画像IOクレートを自作する。

最初から PNG や JPEG の完全対応を目指すのではなく、単純な形式から段階的に実装し、画像IOに必要な内部設計を理解しながら育てる。

## 現在の状態

初期のクレート構成と、基本的な画像表現は実装済み。

- `Image`
- `ImageView`
- `PixelFormat`
- `ImageError`
- endian 読み書き helper

現在対応している形式:

- PPM P6
  - `Rgb8`
  - `maxval = 255`
- PGM P5
  - `Gray8`
  - `maxval = 255`
- BMP
  - 24-bit uncompressed bottom-up
  - `BITMAPINFOHEADER`

各形式には roundtrip example があり、`target/examples/` に画像ファイルを書き出して確認できる。

## 次の目標

次は Netpbm 系フォーマットの基本対応を揃える。

初回リリース前の目標:

- PPM P6 / P3
- PGM P5 / P2
- PBM P4 / P1

初回リリースでは `u8` ベースの基本対応を優先する。

16-bit samples、`maxval > 255`、複数画像 stream、PAM は後回しにする。

## 設計方針

画像データは `Vec<u8>` 単体では扱わず、幅、高さ、ピクセル形式を合わせて保持する。

中心型は次の通り。

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

読み込みでは所有する `Image` を返し、保存では利用者の既存バッファを借りられるように `ImageView` を受け取る。

PBM は初回リリースでは `Gray8` に展開する方針とし、`Bitmap1` のような 1bit 専用表現は後回しにする。

## モジュール構成

現在の構成:

```text
src/
    lib.rs
    image.rs
    error.rs
    io.rs
    codecs/
        mod.rs
        ppm.rs
        pgm.rs
        bmp.rs
```

次に追加する候補:

```text
src/
    codecs/
        netpbm.rs
        pbm.rs
```

`netpbm.rs` には、PPM / PGM / PBM で共通するヘッダ parser や ASCII token parser を置く想定。

## 後回しにするもの

初回リリースまでは次の対応は行わない。

- PNG
- JPEG
- TIFF
- AVIF
- ICC color management
- serde 対応
- rayon による並列化
- bytemuck 的な高速 cast
- 本格的な deflate 圧縮
- 16-bit samples
- `maxval > 255`
- Netpbm の複数画像 stream
- PAM

これらは Netpbm 基本対応と初回リリース後に検討する。
