# Development Plan

このファイルは Codex との作業プランを一時的に保持するためのメモである。
完了済みの大きな履歴は残さず、現在の BMP 対応に必要な作業だけを書く。

## 現在の作業対象

現在は BMP 初期対応の仕様と実装を確認中である。
一次資料は `memo/bmp-spec.md` とする。

現行の実装方針:

- 初期版の generic decode / encode は `BITMAPINFOHEADER` を対象にする。
- generic decode の bit depth は 1-bit / 4-bit / 8-bit indexed `BI_RGB`、24-bit `BI_RGB`、16-bit / 32-bit `BI_BITFIELDS` を対象にする。
- generic encode の bit depth は 1-bit / 4-bit / 8-bit indexed `BI_RGB`、24-bit `BI_RGB`、16-bit / 32-bit `BI_BITFIELDS` を対象にする。
- native decode は 1-bit / 4-bit / 8-bit indexed `BI_RGB`、24-bit `BI_RGB`、16-bit / 32-bit `BI_BITFIELDS` を対象にする。
- native encode は 1-bit / 4-bit / 8-bit indexed `BI_RGB`、24-bit `BI_RGB`、16-bit / 32-bit `BI_BITFIELDS` を対象にする。
- generic encode は、明示 color table による indexed encode と、
  256 色以下なら最小 indexed bit depth、257 色以上なら 24-bit に fallback する auto mode を持つ。
- bottom-up / top-down BMP を扱う。
- generic decode の出力は `PixelFormat::Rgb8` とする。
- BMP encode は `ImageView` と `BmpEncodeOptions` から `BmpImage` を構築し、その native representation を file bytes に書き出す。
- BMP の codec-level encode は `bmp::encode(writer, image, BmpEncodeOptions)` とする。
- top-level `EncodeFormat` は `EncodeFormat::Bmp(BmpEncodeOptions)` とする。
- BMP も top-level native API に追加し、`NativeImage::Bmp(BmpImage)` として扱う。
- `bfSize` と実データ長の不一致は、それだけでは不正な入力として扱わない。
- native encode は native representation の field をできるだけそのまま書き出す。
- native field の file layout 整合性確認には `BmpImage::validate_file_layout` を使う。
- 未対応 DIB header、bit depth、compression は `UnsupportedFormat` として扱う。

## 現在の実装内容

- `BmpEncodeOptions`
- `BmpPixelEncoding::{Rgb24, Indexed1, Indexed4, Indexed8, Bitfields16, Bitfields32, AutoIndexedOrRgb24}`
- `BmpOrientation::{BottomUp, TopDown}`
- `BmpImage`
- `BmpFileHeader`
- `BmpDibHeader::BitmapInfoHeader`
- `BmpInfoHeader`
- `BmpColorTableEntry`
- `bmp::decode_native`
- `bmp::encode_native`
- `BmpImage::validate_file_layout`
- 1-bit / 4-bit indexed BMP の generic decode / encode、native decode / encode
- 8-bit indexed BMP の generic decode / encode、native decode / encode
- 16-bit / 32-bit `BI_BITFIELDS` BMP の generic decode / encode、native decode / encode
- `image_view_to_bmp_native`
- `NativeImage::Bmp(BmpImage)`
- `EncodeFormat::Bmp(BmpEncodeOptions)`

## 次に確認すること

- `make ci` が通る状態を維持する。
- `bmp-spec.md` の初期対応範囲と実装・テストにズレがないか最終確認する。

## 今後の BMP 対応計画

### 1. BMP native API の仕上げ

- このブランチの初期 BMP native API を一区切りにできる状態にする。

### 2. file layout 周りの仕様固定

- unknown gap bytes を保持しない方針を前提に、`validate_file_layout` の判定基準を明確に保つ。
- color table / color masks を追加する前に、`bfOffBits` の最小値計算方針を整理する。
- 可能なら小さな unit test を追加して file layout の境界を固定する。

### 3. 8-bit indexed color BMP の native decode

- 実装済み。

### 4. 8-bit indexed color BMP の generic decode

- 実装済み。

### 5. 1-bit / 4-bit indexed color BMP decode

- 実装済み。

### 6. 16-bit / 32-bit `BI_BITFIELDS`

- 実装済み。

### 7. `BITMAPV4HEADER` / `BITMAPV5HEADER`

- V4 / V5 固有 field を native representation に保持するかを決める。
- generic decode では当面、色空間変換や gamma 補正を行わない方針を維持する。
- ICC profile data を native API で保持するかを検討する。

### 8. RLE decode

- `BI_RLE8` / `BI_RLE4` の decode 対応を検討する。
- encoded mode、absolute mode、end-of-line、end-of-bitmap、delta escape を扱う。
- RLE compression と top-down BMP の組み合わせは不正な header として扱う。

## 実装確認

- `make fmt-check`
- `make lint`
- `make test`
- 必要に応じて `make ci`
