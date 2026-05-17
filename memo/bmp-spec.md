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

このクレートでは、初期版 / 後続版とも `bfType` は `BM` のみ扱う。

`bfReserved1` と `bfReserved2` は仕様上 0 でなければならない。
encode 時は常に 0 を書く。
decode 時に 0 以外だった場合でも、画像データの解釈には使わず無視する。

`bfSize` は仕様上 BMP file 全体の byte size を表す。
encode 時は、実際に出力する file size を書く。
decode 時は、pixel array の解釈には `bfSize` ではなく `bfOffBits` と DIB header の情報を使う。
`bfSize` が実データ長と一致しない場合でも、それだけでは不正とはしない。
ただし、`bfOffBits` と DIB header から必要になる pixel data が入力内に収まらない場合は不正な入力として扱う。

`bfOffBits` は、file 先頭から pixel array までの byte offset を表す。
decode 時は `bfOffBits` を pixel array の開始位置として使う。
`bfOffBits` が `BITMAPFILEHEADER`、DIB header、color masks、color table など、
pixel array より前に必要な領域の途中を指す場合は不正な header として扱う。
`bfOffBits` が入力長を超える場合も不正な header として扱う。

## DIB header

`BITMAPFILEHEADER` の直後には DIB header が置かれる。
DIB header の先頭 field は header size であり、この値によって header の種類を判別する。

初期版では `BITMAPINFOHEADER` のみを decode / encode 対象にする。
`BITMAPV4HEADER` / `BITMAPV5HEADER` は、`BITMAPINFOHEADER` の拡張 header として後続版で追加する。
DIB header は先頭の header size field で種類を判別できるため、
後から V4 / V5 を追加しても基本設計は変えない。

扱う DIB header は次の通りである。

| header | size | 扱い |
| --- | ---: | --- |
| `BITMAPINFOHEADER` | 40 | 初期版の対象 |
| `BITMAPV4HEADER` | 108 | 後続版で追加予定 |
| `BITMAPV5HEADER` | 124 | 後続版で追加予定 |

次の DIB header は対象外にする。

| header | 扱い |
| --- | --- |
| `BITMAPCOREHEADER` | 対象外 |
| OS/2 BMP 固有 header | 対象外 |

対象 header は、Windows BMP の実用上の中心である `BITMAPINFOHEADER` から始め、
その拡張である `BITMAPV4HEADER` / `BITMAPV5HEADER` を後から追加する。
`BITMAPCOREHEADER` と OS/2 固有 header は、field layout や color table の構造が
`BITMAPINFOHEADER` 以降と異なり、古い互換形式としての性格が強いため対象外にする。

## BITMAPINFOHEADER fields

`BITMAPINFOHEADER` は、file 上では次の field 順で並ぶ。

| field | 内容 |
| --- | --- |
| `biSize` | DIB header size |
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

`biSize` は DIB header の byte size を表す。
初期版では `biSize == 40` の `BITMAPINFOHEADER` のみを扱う。
`biSize == 108` の `BITMAPV4HEADER` と `biSize == 124` の `BITMAPV5HEADER` は後続版で追加する。
その他の DIB header size は unsupported format として扱う。

`biWidth` は正の値を対象にする。
`biWidth <= 0` は不正な header として扱う。

`biHeight` は、正負で raster の行方向を表す。

- `biHeight > 0`: bottom-up BMP
- `biHeight < 0`: top-down BMP
- `biHeight == 0`: 不正な header

bottom-up BMP では、file 上の最初の row が画像の一番下の row である。
top-down BMP では、file 上の最初の row が画像の一番上の row である。

初期版で top-down BMP を decode 対象に含めるかは未定。
RLE compression と top-down の組み合わせは仕様上不可とする。

`biPlanes` は 1 を期待する。
`biPlanes != 1` は不正な header として扱う。

`biBitCount` は 1 pixel あたりの bit 数を表す。
初期版では 24-bit `BI_RGB` を対象にする。
扱う bit depth は次の通りである。

| `biBitCount` | 内容 | 扱い |
| ---: | --- | --- |
| 1 | indexed color | 後続版で追加予定 |
| 4 | indexed color | 後続版で追加予定 |
| 8 | indexed color | 後続版で追加予定 |
| 16 | true color | 後続版で追加予定 |
| 24 | true color | 初期版の対象 |
| 32 | true color | 後続版で追加予定 |

`biBitCount == 0` は対象外にする。

24-bit `BI_RGB` の場合、1 pixel は file 上で次の順に並ぶ。

```text
B G R
```

indexed color BMP では、pixel array には RGB 値ではなく color table の index が入る。
16-bit / 32-bit bitfields BMP では、pixel value の各 bit を color mask に従って channel value として解釈する。

`biCompression` は compression method を表す。
初期版では `BI_RGB` のみを対象にする。
扱う compression は次の通りである。

| compression | 内容 | 扱い |
| --- | --- | --- |
| `BI_RGB` | uncompressed RGB / indexed color | 初期版の対象 |
| `BI_BITFIELDS` | RGB bit masks | 後続版で追加予定 |
| `BI_ALPHABITFIELDS` | RGBA bit masks / Windows CE 由来の拡張 | 後続版で追加予定 |
| `BI_RLE8` | 8-bit indexed color RLE | 後続版で追加予定 |
| `BI_RLE4` | 4-bit indexed color RLE | 後続版で追加予定 |

次の compression は対象外にする。

| compression | 扱い |
| --- | --- |
| `BI_JPEG` | 対象外 |
| `BI_PNG` | 対象外 |
| その他の未対応 value | 対象外 |

`BI_RGB` の pixel array は非圧縮 raster data である。
開始位置は `bfOffBits` に従う。

非圧縮 BMP の raster row は 4 byte boundary に揃える。
各行末には padding byte が入ることがある。
padding byte は pixel value として扱わない。

- decode 時は padding byte を読み飛ばす。
- encode 時の padding byte の扱いは未定。

`BI_BITFIELDS` では RGB color mask を扱う。
`BI_ALPHABITFIELDS` では RGBA color mask を扱う。
ただし、`BI_ALPHABITFIELDS` は Windows desktop GDI の標準的な `BITMAPINFOHEADER`
compression value ではなく、Windows CE 由来の拡張として扱う。

`BITMAPINFOHEADER` で `BI_BITFIELDS` または `BI_ALPHABITFIELDS` の場合、
color mask は DIB header の直後に置かれる。
mask が重複している場合は不正な header として扱う。
各 mask の set bit が連続していない場合は不正な header として扱う。
必要な color mask が欠けている場合は不正な header として扱う。
mask から取り出した channel value の正規化方法は未定。

RLE では、encoded mode と absolute mode がある。
また、次の escape を扱う必要がある。

- end-of-line
- end-of-bitmap
- delta

RLE を decode 対象に含めるかは後続版で決める。
RLE を encode 対象に含めるかは未定。

`biSizeImage` は pixel array size を表す。
`BI_RGB` では 0 の場合がある。
decode 時は、pixel array の必要量を header 情報から計算する。
`biSizeImage` が 0 または実際の必要量と一致しない場合でも、それだけでは不正とはしない。
ただし、pixel array が必要量に満たない入力は不正な入力として扱う。

`biXPelsPerMeter` と `biYPelsPerMeter` は resolution metadata を表す。
generic `Image` への decode では、resolution metadata を保持しない。
encode 時に resolution を指定できるようにするかは未定。
指定しない場合の既定値は未定。

`biClrUsed` は color table の entry 数を表す。
1-bit / 4-bit / 8-bit BMP は color table を使う。
color table は pixel array の前に置かれる。

color table entry は `RGBQUAD` として扱う。
file 上の byte order は次の通りである。

```text
B G R reserved
```

color table の entry 数は、`biClrUsed` が 0 でない場合は `biClrUsed` に従う。
`biClrUsed == 0` の場合は bit depth から決まる標準の最大 entry 数に従う。

pixel index が color table の範囲外を参照する場合は不正な raster として扱う。

color table entry の `reserved` byte を alpha として扱うかは未定。

`biClrImportant` は important color count を表す。
generic `Image` への decode では、この値を画像データの解釈には使わない。

## BITMAPV4HEADER / BITMAPV5HEADER

`BITMAPV4HEADER` / `BITMAPV5HEADER` は後続版で追加する。

V4 / V5 header は、`BITMAPINFOHEADER` の fields の後ろに、
color masks、color space、gamma、ICC profile などの fields を追加する。
generic `Image` への decode では、色空間変換や gamma 補正を行わない予定である。
ICC profile data を保持する native BMP API を用意するかは未定。
色空間情報をどこまで公開 API として扱うかも未定。

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

対応外の DIB header、bit depth、compression は unsupported format として扱う。
