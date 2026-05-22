# Development Plan

このファイルは Codex との作業プランを一時的に保持するためのメモである。
完了済みの大きな履歴は残さず、現在検討中の BMP 対応に必要な作業だけを書く。

## 現在の作業対象

現在は BMP 対応の仕様を整理している。
一次資料は `memo/bmp-spec.md` とする。

現行実装は、24-bit `BI_RGB` の bottom-up `BITMAPINFOHEADER` BMP を
`PixelFormat::Rgb8` として decode / encode できる段階である。

次の目標は、BMP の初期対応範囲を仕様として固めたうえで、
実装をその仕様に合わせて小さく修正すること。

## 仕様整理で決まったこと

- 初期版の generic decode / encode は `BITMAPINFOHEADER` を対象にする。
- 初期版の bit depth は 24-bit `BI_RGB` を対象にする。
- top-down BMP は仕様上の初期対応範囲に含める。
- generic decode の出力は `PixelFormat::Rgb8` とする。
- BMP encode は `ImageView` と `BmpEncodeOptions` から BMP native representation を構築し、その native representation を file bytes に書き出す構成にする。
- `BmpEncodeOptions` は `Option<T>` field ではなく具体値を持ち、未指定値は `Default` で表現する。
- BMP の codec-level encode は `bmp::encode(writer, image, BmpEncodeOptions)` とする。
- top-level `EncodeFormat` は `EncodeFormat::Bmp(BmpEncodeOptions)` とする。
- PNM / PAM は既存の軽い encode 指定を維持する。
  - PNM: `PnmEncodeFormat`
  - PAM: `PamEncodeTupleType`
- `bfSize` と実データ長の不一致は、それだけでは不正な入力として扱わない。
- 未対応 DIB header、bit depth、compression は `UnsupportedFormat` として扱う。

## まだ決めること

### BMP native representation

`BmpImage` が何を保持するかを決める。

候補:

- 意味的な BMP 表現だけを持ち、`bfSize` / `bfOffBits` / `biSizeImage` などの派生 field は encode 時に計算する。
- file header / DIB header field をより直接的に持つ。

現時点の方針としては、`BmpEncodeOptions` で派生 field を直接指定させないため、
`BmpImage` も派生 field を持ちすぎない形が自然である。

決めたいこと:

- `BmpImage` を public API にするか、初期実装では `pub(crate)` に留めるか。
- `BmpImage` に file header field を保持するか。
- `BmpImage` に DIB header field を保持するか。
- encode 用 native representation と将来の decode_native 用 representation を同じ型にするか。

### BMP native API

BMP を `NativeImage` に追加するかを決める。

候補:

- 初期版では `encode` 用の内部 native representation だけを導入し、top-level `decode_native` / `encode_native` には BMP を追加しない。
- `NativeImage::Bmp(BmpImage)` を追加し、BMP の native encode を top-level API からも扱えるようにする。

現時点では、まず codec 内部の native representation と `bmp::encode` の整理を優先し、
top-level native API への露出は後で決めるのが安全である。

### エラー分類

仕様と現行実装にズレがあるため、実装前に境界を確認する。

- 未対応 DIB header size は `UnsupportedFormat`
- 未対応 bit depth は `UnsupportedFormat`
- 未対応 compression は `UnsupportedFormat`
- 壊れた header は `InvalidHeader`
- pixel data 不足は `InvalidBufferLength` または `InvalidData`
- dimensions / row size / file size の overflow は `ImageDimensionsTooLarge`

### top-level encode API

`EncodeFormat::Bmp(BmpEncodeOptions)` に変更する。
互換性は不要なので、既存の unit variant `EncodeFormat::Bmp` は残さない。

決めたいこと:

- `BmpEncodeOptions` / `BmpOrientation` / `BmpPixelEncoding` を `kimage::codecs::bmp` から使う形でよいか。
- top-level re-export に含めるか。
- `BmpEncodeOptions::new()` や builder-style method を最初から実装するか。

## 実装ステップ案

### 1. BMP decode を仕様へ寄せる

- 24-bit `BI_RGB` top-down decode を追加する。
- `bfSize > input.len()` を、それだけでは拒否しないようにする。
- 未対応 DIB header / bit depth / compression のエラーを `UnsupportedFormat` に寄せる。
- `bfOffBits` と pixel data 必要量の validation を維持する。

### 2. BMP encode options を導入する

- `BmpEncodeOptions`
- `BmpPixelEncoding::Rgb24`
- `BmpOrientation::{BottomUp, TopDown}`
- `Default` 実装
- 必要なら `new` / `with_orientation` / `with_resolution`

### 3. BMP encode を options 引数付き API にする

- `bmp::encode(writer, image, options)` に変更する。
- `EncodeFormat::Bmp(BmpEncodeOptions)` に変更する。
- top-level `encode` から `bmp::encode` に options を渡す。
- examples / README / tests を新 API に合わせる。

### 4. BMP native representation を導入する

- `ImageView + BmpEncodeOptions -> BmpImage` の変換を追加する。
- `bmp::encode` は `BmpImage` を構築してから書き出す流れにする。
- 派生 field は native representation 構築時または encode 時に一貫して計算する。

### 5. CI を通す

- `make fmt-check`
- `make lint`
- `make test`
- 必要に応じて `make ci`
