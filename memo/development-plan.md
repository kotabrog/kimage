# Development Plan

## ブランチ方針

開発は `develop` を中心に進め、機能ごとに小さなブランチを切る。

ブランチ名は Conventional Commits に近い短い prefix を使う。

```text
feat/image-core
feat/binary-io
feat/ppm-codec
feat/pgm-codec
feat/bmp-codec
```

## 1. feat/image-core

画像IOクレートの共通データ構造を定義する。

このブランチでは codec 実装には入らず、PPM/BMP/PNG が共通して利用する土台だけを作る。

想定する作業範囲:

- `src/image.rs`
  - `Image`
  - `ImageView<'a>`
  - `PixelFormat`
  - `PixelFormat::channels()`
  - `PixelFormat::bytes_per_pixel()`
  - `Image::new(...)`
  - `Image::as_view()`
- `src/error.rs`
  - `ImageError`
  - buffer length validation 用のエラー
  - unsupported format / unsupported pixel format 用のエラー
- `src/lib.rs`
  - module export
  - project setup 用の最小テストを実用的な型テストへ置き換える

テスト方針:

- `PixelFormat` のチャンネル数を検証する
- `PixelFormat` の bytes per pixel を検証する
- `Image::new` の正常系を検証する
- `Image::new` のバッファ長不一致を検証する
- `Image::as_view` が所有バッファを借用ビューとして返すことを検証する

## 2. feat/binary-io

画像フォーマット実装で共通して使う binary IO helper を追加する。

PPM はほぼ不要だが、BMP 以降で endian 読み書きが必要になるため、先に小さく用意する。

想定する作業範囲:

- `src/io.rs`
  - `read_u16_le`
  - `read_u32_le`
  - `read_u16_be`
  - `read_u32_be`
  - `write_u16_le`
  - `write_u32_le`
  - `write_u16_be`
  - `write_u32_be`

テスト方針:

- little-endian の読み書きを検証する
- big-endian の読み書きを検証する
- 短い入力で `std::io::Error` が返ることを検証する

## 3. feat/ppm-codec

最初の画像フォーマットとして PPM P6 を読み書きする。

最初は `PixelFormat::Rgb8` のみを扱う。

想定する作業範囲:

- `src/codecs/mod.rs`
- `src/codecs/ppm.rs`
- PPM P6 decoder
- PPM P6 encoder
- 必要であれば format enum の最小追加

テスト方針:

- 小さな PPM P6 を `Image` に decode できること
- `ImageView` を PPM P6 として encode できること
- decode -> encode の roundtrip を検証する
- magic number、サイズ、max value、バッファ長の異常系を分けて検証する

## 4. feat/pgm-codec

PGM P5 を追加し、`PixelFormat::Gray8` の実用性を確認する。

想定する作業範囲:

- PGM P5 decoder
- PGM P5 encoder
- `Gray8` の保存・読み込み経路の検証

## 5. feat/bmp-codec

BMP の最小対応を追加する。

まずは非圧縮の 24-bit BMP を対象にする。

想定する作業範囲:

- BMP header parser
- BGR/RGB 変換
- row padding 対応
- top-down / bottom-up の扱いを明示

## 後続候補

BMP まで固まった後に、PNG の最小 encoder へ進む。

PNG に入る前に必要になる候補:

- `checksum.rs`
  - CRC32
  - Adler-32
- `bitstream.rs`
- `zlib.rs`
- `deflate.rs`

PNG の最初の目標は、filter type 0 と deflate stored block のみで有効な PNG を保存すること。
