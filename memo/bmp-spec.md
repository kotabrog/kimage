# BMP 仕様メモ

このドキュメントは、BMP format の仕様を、ファイルの中身が追える形でまとめる。

BMP は Windows 系で使われる raster image format である。
Netpbm と比べると、header の種類、bit depth、palette、compression、color mask、metadata の組み合わせが多い。

## 形式の全体像

BMP file は、おおむね次の順番で構成される。

```text
BITMAPFILEHEADER
DIB header
[color masks]
[color table]
pixel array
[color profile]
```

各領域の意味は次の通りである。

| 領域 | 内容 |
| --- | --- |
| `BITMAPFILEHEADER` | BMP file 全体の file header |
| DIB header | 画像サイズ、bit depth、compression などを表す header |
| color masks | 16-bit / 32-bit pixel の channel bit mask |
| color table | indexed color BMP の palette |
| pixel array | 実際の raster data |
| color profile | V5 header で参照される linked / embedded profile data |

pixel array の開始位置は、`BITMAPFILEHEADER` の `bfOffBits` に書かれている。
そのため、DIB header の直後から必ず pixel data が始まるとは限らない。

## BITMAPFILEHEADER

BMP file の先頭には `BITMAPFILEHEADER` が置かれる。

主な fields は次の通りである。

| field | 内容 |
| --- | --- |
| `bfType` | BMP signature |
| `bfSize` | file size |
| `bfReserved1` | reserved field |
| `bfReserved2` | reserved field |
| `bfOffBits` | pixel array の開始 offset |

このクレートでは、完成予定版でも `bfType` は `BM` のみ扱う。

`bfReserved1` と `bfReserved2` は 0 を期待する。
0 以外の場合の扱いは未定。

`bfSize` と実データ長が一致しない場合の厳密な扱いは未定。
ただし、pixel array が必要量に満たない入力は不正な入力として扱う。

## DIB header

`BITMAPFILEHEADER` の直後には DIB header が置かれる。
DIB header の先頭 field は header size であり、この値によって header の種類を判別する。

完成予定版では、次の DIB header を対象にする。

| header | size | 扱い |
| --- | ---: | --- |
| `BITMAPINFOHEADER` | 40 | 対象 |
| `BITMAPV4HEADER` | 108 | 対象 |
| `BITMAPV5HEADER` | 124 | 対象 |

次の DIB header は対象外にする。

| header | 扱い |
| --- | --- |
| `BITMAPCOREHEADER` | 対象外 |
| OS/2 BMP 固有 header | 対象外 |

## DIB header の基本 fields

`BITMAPINFOHEADER` 以降の header では、主に次の情報を持つ。

| field | 内容 |
| --- | --- |
| `biWidth` | 画像の幅 |
| `biHeight` | 画像の高さと行方向 |
| `biPlanes` | plane 数 |
| `biBitCount` | 1 pixel あたりの bit 数 |
| `biCompression` | compression method |
| `biSizeImage` | pixel array size |
| `biXPelsPerMeter` | horizontal resolution |
| `biYPelsPerMeter` | vertical resolution |
| `biClrUsed` | color table の entry 数 |
| `biClrImportant` | important color count |

`biWidth` は正の値を対象にする。
`biWidth <= 0` は不正な header として扱う。

`biHeight` は、正負で raster の行方向を表す。

- `biHeight > 0`: bottom-up BMP
- `biHeight < 0`: top-down BMP
- `biHeight == 0`: 不正な header

`biPlanes` は 1 を期待する。

## 画像方向

BMP には bottom-up と top-down がある。

bottom-up BMP では、file 上の最初の row が画像の一番下の row である。
top-down BMP では、file 上の最初の row が画像の一番上の row である。

完成予定版では、bottom-up BMP と top-down BMP の両方を扱う。

RLE compression と top-down の組み合わせは仕様上不可とする。

## scan line

非圧縮 BMP の raster row は 4 byte boundary に揃える。
各行末には padding byte が入ることがある。

padding byte は pixel value として扱わない。

- decode 時は padding byte を読み飛ばす。
- encode 時の padding byte の扱いは未定。

## bit depth

完成予定版では、次の bit depth を対象にする。

| `biBitCount` | 内容 |
| ---: | --- |
| 1 | indexed color |
| 4 | indexed color |
| 8 | indexed color |
| 16 | true color |
| 24 | true color |
| 32 | true color |

`biBitCount == 0` は対象外にする。

## pixel array

pixel array は、画像の raster data である。
開始位置は `bfOffBits` に従う。

24-bit `BI_RGB` の場合、1 pixel は file 上で次の順に並ぶ。

```text
B G R
```

32-bit `BI_RGB` の場合、1 pixel は file 上で次の順に並ぶ。

```text
B G R unused
```

indexed color BMP では、pixel array には RGB 値ではなく color table の index が入る。

16-bit / 32-bit bitfields BMP では、pixel value の各 bit を color mask に従って channel value として解釈する。

## color table

1-bit / 4-bit / 8-bit BMP は color table を使う。
color table は pixel array の前に置かれる。

16-bit / 24-bit / 32-bit BMP でも、palette device 用の optional color table を持つことがある。
この場合、`biClrUsed` が color table の entry 数を表す。
完成予定版で optional color table を保持するかは未定。

color table entry は `RGBQUAD` として扱う。
file 上の byte order は次の通りである。

```text
B G R reserved
```

color table の entry 数は、`biClrUsed` が 0 でない場合は `biClrUsed` に従う。
`biClrUsed == 0` の場合は bit depth から決まる標準の最大 entry 数に従う。

pixel index が color table の範囲外を参照する場合は不正な raster として扱う。

color table entry の `reserved` byte を alpha として扱うかは未定。

## compression

完成予定版では、次の compression を対象にする。

| compression | 内容 |
| --- | --- |
| `BI_RGB` | uncompressed RGB / indexed color |
| `BI_BITFIELDS` | RGB bit masks |
| `BI_ALPHABITFIELDS` | RGBA bit masks / Windows CE 由来の拡張 |
| `BI_RLE8` | 8-bit indexed color RLE |
| `BI_RLE4` | 4-bit indexed color RLE |

次の compression は対象外にする。

| compression | 扱い |
| --- | --- |
| `BI_JPEG` | 対象外 |
| `BI_PNG` | 対象外 |
| その他の未対応 value | 対象外 |

## color masks

16-bit / 32-bit BMP では、color mask によって pixel value 内の bit 配置を表すことがある。

`BI_BITFIELDS` では RGB color mask を扱う。
`BI_ALPHABITFIELDS` では RGBA color mask を扱う。
ただし、`BI_ALPHABITFIELDS` は Windows desktop GDI の標準的な `BITMAPINFOHEADER`
compression value ではなく、Windows CE 由来の拡張として扱う。

color mask は、DIB header の種類によって置き場所が変わる。

- `BITMAPINFOHEADER` + `BI_BITFIELDS` / `BI_ALPHABITFIELDS`: DIB header の直後
- `BITMAPV4HEADER`: DIB header 内
- `BITMAPV5HEADER`: DIB header 内

mask が重複している場合は不正な header として扱う。
各 mask の set bit が連続していない場合は不正な header として扱う。
必要な color mask が欠けている場合は不正な header として扱う。

mask から取り出した channel value の正規化方法は未定。

## alpha

BMP の alpha の扱いは、header と compression によって意味が変わる。

明示的な alpha mask がある BMP は alpha 付き画像として扱う予定である。

alpha mask がない 32-bit `BI_RGB` は、上位 byte を未使用 byte として扱う予定である。
この場合、decode output を `Rgb8` にするか `Rgba8` にするかは未定。

indexed color の color table entry に含まれる `reserved` byte を alpha として扱うかは未定。

## RLE

BMP には indexed color 用の RLE compression がある。

- `BI_RLE8`: 8-bit indexed color BMP の圧縮形式
- `BI_RLE4`: 4-bit indexed color BMP の圧縮形式

RLE では、encoded mode と absolute mode がある。
また、次の escape を扱う必要がある。

- end-of-line
- end-of-bitmap
- delta

完成予定版で RLE を decode 対象に含める。
RLE を encode 対象に含めるかは未定。

## color management

`BITMAPV4HEADER` / `BITMAPV5HEADER` は、色空間や gamma に関する fields を持つ。
linked / embedded ICC profile data は `BITMAPV5HEADER` の profile fields で参照される。

generic `Image` への decode では、色空間変換や gamma 補正を行わない予定である。

ICC profile data を保持する native BMP API を用意するかは未定。
色空間情報をどこまで公開 API として扱うかも未定。

## metadata

BMP header には resolution fields がある。

- `biXPelsPerMeter`
- `biYPelsPerMeter`

generic `Image` への decode では、resolution metadata を保持しない予定である。

encode 時に resolution を指定できるようにするかは未定。
指定しない場合の既定値は未定。

application-specific data を保持するかは未定。

## decode output

generic `Image` への decode output は未定部分がある。

現時点の予定は次の通りである。

- 24-bit `BI_RGB`: `Rgb8`
- indexed color BMP: 未定
- 16-bit true color BMP: 未定
- 32-bit true color BMP: 未定
- alpha 付き BMP: 未定
- color profile を持つ BMP: 色変換は行わない

## encode input

BMP encode の完成予定仕様は未定部分がある。

現時点の予定は次の通りである。

- `Rgb8`: encode 対象
- `Rgba8`: encode 対象に含めるか未定
- `Gray8`: encode 対象に含めるか未定
- `Gray16` / `Rgb16` / `Rgba16` / gray alpha formats: 未定

`Rgb8` encode の既定形式は 24-bit `BI_RGB` とする予定である。
その他の encode format selection API は未定。

## multi-image

BMP は単一画像形式として扱う。

BMP の `decode_all` / `decode_all_native` は対象外にする予定である。
1ファイル内に複数の BMP file を連結した入力は標準形式として扱わない。

## native representation

BMP native representation を用意するかは未定。

native representation を用意する場合は、少なくとも次の情報を保持する候補がある。

- DIB header 種類
- bit depth
- compression
- color table
- color masks
- resolution
- color profile metadata
- raster data

## 不正データの扱い

次の入力は不正な入力として扱う。

- header が途中で終わる
- pixel data が header から計算される必要量に満たない
- `bfOffBits` が header / color table / mask fields の途中を指す
- `biWidth <= 0`
- `biHeight == 0`
- RLE compression と top-down の組み合わせ
- `biPlanes != 1`
- pixel index が color table の範囲外を参照する
- mask が重複している
- mask の set bit が連続していない
- 必要な mask が欠けている

`bfSize` と実データ長の不一致をどこまで許容するかは未定。

対応外の DIB header、bit depth、compression は unsupported format として扱う。
