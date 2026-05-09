# BMP 仕様メモ

このドキュメントは、BMP codec の完成予定版で扱う仕様範囲をまとめる。

## 対象形式

- Windows BMP file format を対象にする。
- ファイル先頭は `BITMAPFILEHEADER` とし、signature は `BM` のみ扱う。
- `BITMAPFILEHEADER` の reserved fields は 0 を期待する。
- pixel data の開始位置は `bfOffBits` に従う。
- DIB header は次を対象にする。
  - `BITMAPINFOHEADER`
  - `BITMAPV4HEADER`
  - `BITMAPV5HEADER`
- `BITMAPCOREHEADER` は対象外にする。
- OS/2 BMP 固有の拡張は対象外にする。

## 画像方向

- bottom-up BMP を扱う。
- top-down BMP を扱う。
- `biHeight > 0` の場合は bottom-up として扱う。
- `biHeight < 0` の場合は top-down として扱う。
- `biHeight == 0` は不正な header として扱う。
- `biWidth <= 0` は不正な header として扱う。

## scan line

- BMP の raster row は 4 byte boundary に揃える。
- 各行末の padding byte は pixel value として扱わない。
- decode 時は padding byte を読み飛ばす。
- encode 時は padding byte を 0 で埋める。

## pixel format

完成予定版では、次の bit depth を対象にする。

- 1-bit indexed color
- 4-bit indexed color
- 8-bit indexed color
- 16-bit true color
- 24-bit true color
- 32-bit true color

`biBitCount == 0` は対象外にする。

## color table

- 1-bit / 4-bit / 8-bit BMP は color table を使って `Rgb8` または `Rgba8` 相当の画像として扱う。
- color table entry は `RGBQUAD` として扱う。
- color table の byte order は `B`, `G`, `R`, `reserved` として扱う。
- color table の entry 数は、`biClrUsed` が 0 でない場合は `biClrUsed` に従う。
- `biClrUsed == 0` の場合は bit depth から決まる標準の最大 entry 数に従う。
- pixel index が color table の範囲外を参照する場合は不正な raster として扱う。

## compression

完成予定版では、次の compression を対象にする。

- `BI_RGB`
- `BI_BITFIELDS`
- `BI_ALPHABITFIELDS`
- `BI_RLE8`
- `BI_RLE4`

次の compression は対象外にする。

- `BI_JPEG`
- `BI_PNG`
- その他の未対応 compression value

## 16-bit / 32-bit color masks

- `BI_BITFIELDS` では RGB color mask を扱う。
- `BI_ALPHABITFIELDS` では RGBA color mask を扱う。
- `BITMAPV4HEADER` / `BITMAPV5HEADER` に含まれる color mask を扱う。
- `BITMAPINFOHEADER` で `BI_BITFIELDS` または `BI_ALPHABITFIELDS` の場合は、DIB header 直後の mask fields を扱う。
- mask が重複している場合は不正な header として扱う。
- 必要な color mask が欠けている場合は不正な header として扱う。
- mask から取り出した channel value は、対応する `PixelFormat` の full range に正規化して扱う。

## alpha

- 明示的な alpha mask がある BMP は alpha 付き画像として扱う。
- alpha mask がない 32-bit `BI_RGB` は、上位 byte を未使用 byte として扱う。
- alpha mask がない 32-bit `BI_RGB` を decode する場合、出力は `Rgb8` とする。
- alpha mask がある 32-bit BMP を decode する場合、出力は `Rgba8` とする。
- indexed color の color table entry に含まれる reserved byte は、通常は alpha として扱わない。

## RLE

- `BI_RLE8` は 8-bit indexed color BMP の圧縮形式として扱う。
- `BI_RLE4` は 4-bit indexed color BMP の圧縮形式として扱う。
- encoded mode と absolute mode を扱う。
- end-of-line, end-of-bitmap, delta escape を扱う。
- RLE 展開後の index が color table の範囲外を参照する場合は不正な raster として扱う。
- RLE 展開結果が header の width / height と一致しない場合は不正な raster として扱う。
- top-down RLE は対象外にする。

## color management

- `BITMAPV4HEADER` / `BITMAPV5HEADER` の color space fields は header として読み取れる範囲にする。
- ICC profile data は、generic `Image` への decode では色変換に使わない。
- generic `Image` への decode では gamma 補正や色空間変換を行わない。
- 色空間情報を保持する native BMP API は、この仕様メモでは必須範囲に含めない。

## metadata

- resolution fields は BMP header として読み取れる範囲にする。
- generic `Image` への decode では resolution metadata を保持しない。
- encode 時に resolution を指定しない場合は 0 を書く。
- application-specific data は保持しない。

## decode output

- 1-bit / 4-bit / 8-bit indexed color BMP は、color table 展開後の `Rgb8` として扱う。
- alpha を持つ indexed color BMP は対象外にする。
- 16-bit / 24-bit / 32-bit RGB BMP は `Rgb8` として扱う。
- 明示 alpha を持つ 32-bit BMP は `Rgba8` として扱う。
- 16-bit BMP は channel value を 8-bit full range に正規化して扱う。
- BMP の色空間情報による変換は行わない。

## encode input

- `Rgb8` は BMP として encode できる。
- `Rgba8` は BMP として encode できる。
- `Gray8` / `Gray16` / `Rgb16` / `Rgba16` / gray alpha formats は直接の BMP encode 対象外にする。
- `Rgb8` encode の既定形式は 24-bit `BI_RGB` とする。
- `Rgba8` encode の既定形式は 32-bit alpha mask 付き BMP とする。

## multi-image

- BMP は単一画像形式として扱う。
- BMP の `decode_all` / `decode_all_native` は対象外にする。
- 1ファイル内に複数の BMP file を連結した入力は標準形式として扱わない。

## 不正データの扱い

- header が途中で終わる場合は不正な入力として扱う。
- pixel data が header から計算される必要量に満たない場合は不正な入力として扱う。
- header の file size と実データの範囲が矛盾する場合は不正な header として扱う。
- `bfOffBits` が header / color table / mask fields の途中を指す場合は不正な header として扱う。
- 対応外の DIB header、bit depth、compression は unsupported format として扱う。

