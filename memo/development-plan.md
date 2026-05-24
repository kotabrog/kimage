# Development Plan

このファイルは Codex との作業プランを一時的に保持するためのメモである。
完了済みの大きな履歴は残さず、現在の BMP 対応に必要な作業だけを書く。

## 現在の作業対象

現在は BMP 初期対応を実装中である。
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
- `image_view_to_bmp_native`
- `NativeImage::Bmp(BmpImage)`
- `EncodeFormat::Bmp(BmpEncodeOptions)`

## 次に確認すること

- `BmpImage` の public field 構成がこのブランチ内の実装に十分か確認する。
- `BmpImage` が保持する `pixel_array` は file 上の BGR + padding 込み data として扱う方針でよいか確認する。
- `encode_native` で `bfSize` / `biSizeImage` の不一致をどの程度 strict に拒否するか確認する。
- `decode_all_native` で BMP を引き続き `UnsupportedFormat` にする方針でよいか確認する。
- README と examples の説明が新 API に追従しているか確認する。

## 実装確認

- `make fmt-check`
- `make lint`
- `make test`
- 必要に応じて `make ci`
