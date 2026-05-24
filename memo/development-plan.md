# Development Plan

このファイルは Codex との作業プランを一時的に保持するためのメモである。
完了済みの大きな履歴は残さず、現在の BMP 対応に必要な作業だけを書く。

## 現在の作業対象

現在は BMP 初期対応の仕様と実装を確認中である。
一次資料は `memo/bmp-spec.md` とする。

現行の実装方針:

- 初期版の generic decode / encode は `BITMAPINFOHEADER` を対象にする。
- bit depth は 24-bit `BI_RGB` を対象にする。
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
- `BmpPixelEncoding::Rgb24`
- `BmpOrientation::{BottomUp, TopDown}`
- `BmpImage`
- `BmpFileHeader`
- `BmpDibHeader::BitmapInfoHeader`
- `BmpInfoHeader`
- `BmpColorTableEntry`
- `bmp::decode_native`
- `bmp::encode_native`
- `BmpImage::validate_file_layout`
- `image_view_to_bmp_native`
- `NativeImage::Bmp(BmpImage)`
- `EncodeFormat::Bmp(BmpEncodeOptions)`

## 次に確認すること

- `make ci` が通る状態を維持する。
- README に `validate_file_layout` の説明を追加するか確認する。
- `bmp-spec.md` の初期対応範囲と実装・テストにズレがないか最終確認する。

## 実装確認

- `make fmt-check`
- `make lint`
- `make test`
- 必要に応じて `make ci`
