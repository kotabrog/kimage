# Codex Project Overview

## 目的

このプロジェクトでは、Rust の標準ライブラリを中心に、画像ファイルを読み書きする小さな画像IOクレートを自作する。

最初から PNG や JPEG の完全対応を目指すのではなく、単純な形式から段階的に実装し、画像IOに必要な内部設計を理解しながら育てる。

## 現在の状態

初期のクレート構成、基本的な画像表現、Netpbm の基本形式対応は実装済み。

- `Image`
- `ImageView`
- `PixelFormat`
- `ImageError`
- endian 読み書き helper
- Netpbm 共通 parser / helper

現在対応している形式:

- PBM P1 / P4
  - 内部では `Gray8` に展開
  - PBM の `0 = white`, `1 = black` を `Gray8` の `255 = white`, `0 = black` に変換
- PGM P2 / P5
  - `Gray8`
  - `maxval = 255`
- PPM P3 / P6
  - `Rgb8`
  - `maxval = 255`
- BMP
  - 24-bit uncompressed bottom-up
  - `BITMAPINFOHEADER`

各形式には roundtrip example があり、`target/examples/` に画像ファイルを書き出して確認できる。

## 次の目標

次は Netpbm 系フォーマットについて、仕様上の残りを実装計画に落とし込み、初回リリース前に対応範囲を広げる。

対象:

- parser の仕様追従
- PGM / PPM の `maxval` 1..=65535
- PGM / PPM の 16-bit sample
- Netpbm の `maxval` を保持する native image 型
- native image から汎用 `Image` への正規化変換
- 複数画像を連結した Netpbm stream
- PAM P7

README とリリース準備は、Netpbm の残作業が完了してから行う。

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

既存の `Image` は、正規化済みの汎用画像バッファとして扱う。読み込みでは所有する `Image` を返し、保存では利用者の既存バッファを借りられるように `ImageView` を受け取る。

Netpbm では、仕様上の `maxval` やsubformatを保持するために、別途 native image 型を追加する。

```rust
pub enum NetpbmImage {
    Pbm {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    Pgm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
    Ppm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
}
```

`NetpbmImage` はファイル仕様に近い保持型とする。PGM / PPM は `maxval` を保持し、sample値は正規化しない。PBM は `maxval` を持たず、data は `0 = white`, `1 = black` のNetpbm仕様値として保持する。

既存の `decode` / `decode_ascii` は、native decode の結果を正規化して `Image` を返すAPIとして維持する。追加APIとして `decode_native` / `encode_native` を用意し、Netpbmとしての `maxval` を保持した読み書きを可能にする。

PBM は引き続き `Gray8` に展開する。`Bitmap1` のような1bit専用表現は、必要性が明確になるまで追加しない。

PGM / PPM の 16-bit sample 対応では、正規化済み `Image` 用に `PixelFormat::Gray16` / `PixelFormat::Rgb16` を追加する。`Image::data` は `Vec<u8>` のまま維持し、`Gray16` / `Rgb16` の内部byte orderは little-endian に統一する。Netpbm の binary sample はファイル上 big-endian なので、decode / encode の境界で変換する。

正規化では、PGM / PPM の `0..maxval` を `0..255` または `0..65535` にスケーリングする。色空間変換や gamma 補正は行わない。

PAM P7 は、PGM / PPM の 16-bit 対応と multi-image API が固まった後に、改めて詳細計画を立てる。可能であれば広く対応したいが、alpha 付き tuple type を扱う場合は `GrayAlpha8` / `GrayAlpha16` / `Rgba16` などの追加が必要になる可能性がある。

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
        netpbm.rs
        pbm.rs
        pgm.rs
        ppm.rs
        bmp.rs
```

追加候補:

```text
src/
    codecs/
        pam.rs
        pnm.rs
```

`netpbm.rs` には、PBM / PGM / PPM / PAM で共通する token parser、header helper、raster length helper を置く。

`pam.rs` には PAM P7 の個別実装を置く。

`pnm.rs` は、P1..P6 を magic number で自動判別する上位APIとして追加を検討する。形式別APIは `pbm.rs` / `pgm.rs` / `ppm.rs` に残し、`pnm.rs` は事前にsubformatを判定したくない利用者向けの入口にする。native API と正規化済み API の両方を用意する。

## Netpbm 仕様メモ

- PBM / PGM / PPM / PAM は、それぞれ同一subformatの画像を区切りなしで複数連結できる。
- Plain PBM / PGM / PPM は仕様上、1ファイル1画像として扱う。
- `pbm::decode_all_native`, `pgm::decode_all_native`, `ppm::decode_all_native` は同一subformatのstreamを扱う。
- P1 / P2 / P3 の複数画像風入力は、仕様重視でエラーにする。
- `pnm::decode_all_native` は最初のmagic numberでsubformatを決め、そのsubformatのstreamとして読む。異なるsubformatの混在streamを標準対応しない。
- Netpbm の whitespace は space, TAB, CR, LF, VT, FF。
- コメントは `#` から次の CR または LF の直前まで。
- PBM P4 は1bit/pixelで、各行は8bit単位に詰める。余ったbitは don't care。
- PBM は `0 = white`, `1 = black`。
- PGM / PPM の `maxval` は 1..=65535。
- PGM / PPM の binary sample は、`maxval < 256` なら1 byte、`maxval >= 256` なら2 bytes big-endian。
- `NetpbmImage` では PGM / PPM sample値を正規化せず保持する。`maxval >= 256` のsampleは内部 little-endian の `u16` として持つ。
- 汎用 `Image` へ変換する場合のみ、target bit depth のfull rangeへ正規化する。
- PGM / PPM の sample 値は仕様上 BT.709 gamma transfer function に基づくが、このクレートでは色空間変換や gamma 補正は行わない。
- PAM P7 は `WIDTH`, `HEIGHT`, `DEPTH`, `MAXVAL`, `ENDHDR` を持つ別形式。`TUPLTYPE` は任意だが、画像として扱うには重要。
- PAM の `BLACKANDWHITE` は `0 = black`, `1 = white` で、PBMとは値の意味が逆。

## 後回しにするもの

Netpbm の仕様追従が固まるまでは、次の対応は行わない。

- PNG
- JPEG
- TIFF
- AVIF
- ICC color management
- serde 対応
- rayon による並列化
- bytemuck 的な高速 cast
- 本格的な deflate 圧縮

これらは Netpbm 対応と初回リリース後に検討する。
