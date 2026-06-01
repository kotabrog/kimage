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
generic encode、または `ImageView + BmpEncodeOptions` から `BmpImage` を構築する場合は、
このクレートが生成する BMP として常に 0 を設定する。
`decode_native` では入力 file の値を native representation に保持する。
`encode_native` では、native representation に保持された値を原則そのまま書く。
decode 時に 0 以外だった場合でも、画像データの解釈には使わず無視する。

`bfSize` は仕様上 BMP file 全体の byte size を表す。
generic encode、または `ImageView + BmpEncodeOptions` から `BmpImage` を構築する場合は、
実際に出力する file size を設定する。
`encode_native` では、native representation に保持された値を原則そのまま書く。
decode 時は、pixel array の解釈には `bfSize` ではなく `bfOffBits` と DIB header の情報を使う。
`bfSize` が実データ長と一致しない場合でも、それだけでは不正とはしない。
ただし、`bfOffBits` と DIB header から必要になる pixel data が入力内に収まらない場合は不正な入力として扱う。

`bfOffBits` は、file 先頭から pixel array までの byte offset を表す。
decode 時は `bfOffBits` を pixel array の開始位置として使う。
`bfOffBits` が `BITMAPFILEHEADER`、DIB header、color masks、color table など、
pixel array より前に必要な領域の途中を指す場合は不正な header として扱う。
`bfOffBits` が入力長を超える場合も不正な header として扱う。
`bfOffBits` と pixel array の間に unknown gap bytes がある場合、
generic decode ではその gap bytes を無視し、native representation にも保持しない。
そのため、decode では `bfOffBits >= expected_min_pixel_offset` を受け入れるが、
`BmpImage::validate_file_layout` では `bfOffBits == expected_min_pixel_offset` を要求する。
`validate_file_layout` は、native representation が保持している領域だけで
`encode_native` した場合に file layout が整合するかを確認するためである。
初期版の 24-bit `BI_RGB` + `BITMAPINFOHEADER` では、
`expected_min_pixel_offset` は `14 + 40 = 54` である。
8-bit indexed color BMP では、次の値になる。

```text
14 + 40 + color_table_entry_count * 4
```

後続版で color masks を保持する場合は、
その byte size も加えた値を `expected_min_pixel_offset` とする。

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
初期版では `biSize == 40` の `BITMAPINFOHEADER` のみを generic decode / encode 対象にする。
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

`biHeight < 0` の top-down BMP では、`biCompression` は `BI_RGB` または `BI_BITFIELDS` でなければならない。
そのため、`BI_RLE8` / `BI_RLE4` と top-down BMP の組み合わせは不正な header として扱う。

`biPlanes` は target device の color plane 数を表す。
この field は device-dependent bitmap の planar color format に由来する歴史的な field である。
BMP の DIB header では仕様上 1 でなければならない。
`biPlanes != 1` は不正な header として扱う。

`biBitCount` は 1 pixel あたりの bit 数を表す。
初期版では 24-bit `BI_RGB` を対象にする。
扱う bit depth は次の通りである。

| `biBitCount` | 内容 | 扱い |
| ---: | --- | --- |
| 0 | encoded image format 側で bit depth が決まる | 対象外 |
| 1 | indexed color | 後続版で追加予定 |
| 4 | indexed color | 後続版で追加予定 |
| 8 | indexed color | generic decode 対象、native decode / encode 対象 |
| 16 | true color | 後続版で追加予定 |
| 24 | true color | generic decode / encode 対象、native decode / encode 対象 |
| 32 | true color | 後続版で追加予定 |

上記以外の `biBitCount` は unsupported format として扱う。
`biBitCount == 0` は `BI_JPEG` / `BI_PNG` 向けの値だが、
このクレートでは `BI_JPEG` / `BI_PNG` を対象外にする。

24-bit `BI_RGB` の場合、1 pixel は file 上で次の順に並ぶ。

```text
B G R
```

1-bit / 4-bit / 8-bit BMP は indexed color として扱う。
pixel array には RGB 値ではなく color table の index が入る。
color table の entry 数は `biClrUsed` で決まる。

16-bit / 32-bit BMP は true color として扱う。
`BI_BITFIELDS` / `BI_ALPHABITFIELDS` の場合は、
pixel value の各 bit を color mask に従って channel value として解釈する。
color mask の有無と置き場所は `biCompression` で決まる。

`biCompression` は compression method を表す。
初期版の generic decode / encode では `BI_RGB` のみを対象にする。
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
| `BI_CMYK` / `BI_CMYKRLE8` / `BI_CMYKRLE4` | 対象外 |
| video / FOURCC 系 value | 対象外 |
| その他の未対応 value | 対象外 |

`biCompression` には、GDI の BMP file で使われる値のほか、
Windows CE 由来の拡張、CMYK 系、video frame 用の FOURCC value などが存在する。
このクレートでは、BMP image file として扱う範囲に絞る。
初期版では `BI_RGB` 以外の compression は unsupported format として扱う。
そのため、現時点では RLE compression と top-down BMP の組み合わせも、
unsupported format として扱う。
後続版で RLE を decode 対象に追加する時点で、
RLE compression と top-down BMP の組み合わせを不正な header として扱う。

`BI_RGB` の pixel array は非圧縮 raster data である。

非圧縮 BMP の raster row は 4 byte boundary に揃える。
row の pixel byte 数が 4 の倍数でない場合、row 末尾に padding byte が入る。
decode 時は padding byte を読み飛ばす。
このクレートの encode では padding byte を 0 で書く。

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
generic encode、または `ImageView + BmpEncodeOptions` から `BmpImage` を構築する場合は、
計算した pixel array size を `biSizeImage` に設定する。
`encode_native` では、native representation に保持された値を原則そのまま書く。
出力される BMP の file layout として `biSizeImage` が整合しているか確認したい場合は、
`BmpImage::validate_file_layout` を使う。

`biXPelsPerMeter` と `biYPelsPerMeter` は resolution metadata を表す。
ここでの resolution は、画像の pixel 数ではなく、1 meter あたりの pixel 数で表す pixel density である。
generic `Image` への decode では、resolution metadata を保持しない。

`biClrUsed` は color table の entry 数を表す。
1-bit / 4-bit / 8-bit BMP は color table を使う。
color table の entry 数は、`biClrUsed` が 0 でない場合は `biClrUsed` に従う。
`biClrUsed == 0` の場合は bit depth から決まる標準の最大 entry 数に従う。
`biClrUsed` が bit depth から決まる最大 entry 数より大きい場合は不正な header として扱う。

pixel index が color table の範囲外を参照する場合は不正な raster として扱う。

`biClrImportant` は、表示に重要な color table entry 数を表す。
`biClrImportant == 0` は、すべての color table entry が重要であることを表す。
この field は palette device 向けの hint であり、
generic `Image` への decode では画像データの解釈には使わない。

## color table

color table は indexed color BMP の palette である。
color table は pixel array の前に置かれる。

8-bit indexed color BMP は generic decode 対象、native decode / encode 対象とする。
generic decode では color table を使って `PixelFormat::Rgb8` に展開する。

8-bit indexed color BMP の color table entry 数は次のように決める。

- `biClrUsed == 0`: bit depth から決まる最大 entry 数を使う。8-bit では 256 entries。
- `biClrUsed != 0`: `biClrUsed` entries を使う。

`biClrUsed` が bit depth から決まる最大 entry 数より大きい場合は不正な header として扱う。

color table entry は `RGBQUAD` として扱う。
`RGBQUAD` は 4 byte の color table entry であり、file 上の byte order は次の通りである。

```text
B G R reserved
```

`reserved` byte は BMP 仕様上 0 でなければならない。
`decode_native` では `reserved` の値を native representation に保持する。
`validate_file_layout` では `reserved == 0` を確認する。
generic decode では `reserved` byte を alpha として扱わず、RGB のみを画像データに反映する。

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
- 8-bit indexed color BMP: color table を使って `Rgb8` に展開する
- 1-bit / 4-bit indexed color BMP: 後続版で検討
- 16-bit true color BMP: 未定
- 32-bit true color BMP: 未定
- alpha 付き BMP: 未定
- color profile を持つ BMP: 色変換は行わない

初期版の generic decode 対象は次の範囲に固定する。

- `bfType == "BM"`
- DIB header は `BITMAPINFOHEADER`
- `biWidth > 0`
- `biHeight != 0`
- `biPlanes == 1`
- `biBitCount == 24`
- `biCompression == BI_RGB`
- output は `PixelFormat::Rgb8`

8-bit indexed color BMP では、color table の `RGBQUAD` entries を使って
`PixelFormat::Rgb8` に展開する。
color table entry の `reserved` byte は alpha として扱わず、画像データに反映しない。

24-bit `BI_RGB` では、file 上の pixel は `B G R` の順に並ぶ。
decode ではこれを `Rgb8` の `R G B` に変換する。

初期版の generic decode では、metadata は保持しない。
`bfReserved1` / `bfReserved2`、`bfSize`、`biSizeImage`、`biXPelsPerMeter`、`biYPelsPerMeter`、`biClrImportant` は、
generic `Image` の pixel data には反映しない。

## encode input

BMP encode の完成予定仕様は未定部分がある。

BMP encode は、`ImageView` と `BmpEncodeOptions` から BMP native representation を構築し、
その native representation を file bytes に書き出す構成にする。

```text
ImageView + BmpEncodeOptions
    -> BmpImage
    -> BMP file bytes
```

`BmpEncodeOptions` は、出力する BMP 表現を選ぶための設定である。
file size や pixel array offset など、選択した BMP 表現から一意に決まる header field は直接指定させない。
`BmpEncodeOptions` の各 field は、原則として `Option<T>` ではなく具体値を持つ。
未指定の場合の値は `Default` で表現する。
これは、native representation 構築時に未指定状態を残さず、
常に確定した encode 方針として扱うためである。

BMP の codec-level encode は、PNM / PAM と同様に `encode` の引数で encode 指定を受け取る。
そのため、BMP では `bmp::encode(writer, image, BmpEncodeOptions)` とする。
Top-level `EncodeFormat` は、generic `ImageView` から各 format の native representation
を構築するための encode 指定を保持する。
PNM は subformat の選択だけで十分なため `PnmEncodeFormat` を使う。
PAM は tuple type の選択だけで十分なため `PamEncodeTupleType` を使う。
BMP は pixel encoding、orientation、resolution metadata など複数の設定を持つため
`BmpEncodeOptions` を使う。
BMP は `EncodeFormat::Bmp(BmpEncodeOptions)` とし、
top-level `encode` は `BmpEncodeOptions` を `bmp::encode` に渡す。

generic encode は、`PixelFormat::Rgb8` の `ImageView` を対象にする。
row order は `BmpEncodeOptions` の orientation に従い、既定値は bottom-up とする。

初期版の `BmpEncodeOptions` は、次の設定を持つ。

| option | 内容 | 初期値 |
| --- | --- | --- |
| pixel encoding | pixel array の表現 | 24-bit `BI_RGB` |
| orientation | row order | bottom-up |
| `biXPelsPerMeter` | horizontal resolution metadata | 0 |
| `biYPelsPerMeter` | vertical resolution metadata | 0 |

pixel encoding は次を持つ。

- `Rgb24`
- `Indexed8 { color_table }`
- `AutoIndexed8OrRgb24`

`Rgb24` は `PixelFormat::Rgb8` の `ImageView` を受け付け、
24-bit `BI_RGB` の `BITMAPINFOHEADER` BMP を生成する。
color table は持たず、`biBitCount == 24`、`biClrUsed == 0`、
`bfOffBits == 14 + 40` とする。

`Indexed8 { color_table }` は、指定された color table を使って
8-bit indexed `BI_RGB` の `BITMAPINFOHEADER` BMP を生成する。
`color_table.len()` は `1..=256` とし、各 `RGBQUAD` entry の `reserved` は 0 でなければならない。
入力画像の各 RGB 値は color table 内の RGB 値と完全一致する必要がある。
一致する entry がない場合は encode error とし、近似色への変換や量子化はしない。
同じ RGB 値が color table に複数ある場合は、最初の entry を使う。
`biBitCount == 8`、`biClrUsed == color_table.len()`、
`bfOffBits == 14 + 40 + color_table.len() * 4` とする。
pixel array は palette index byte と row padding を含む。

`AutoIndexed8OrRgb24` は、入力画像を可逆に 8-bit indexed BMP で表現できる場合だけ
8-bit indexed BMP を生成し、できない場合は 24-bit BMP に fallback する。
入力画像内の unique RGB 色が 256 色以下なら、画像走査順に初出の色を追加した
deterministic な color table を生成し、8-bit indexed BMP を出力する。
unique RGB 色が 257 色以上なら `Rgb24` と同じ 24-bit BMP を出力する。
この mode では量子化、dithering、近似色変換は行わないため、常に可逆変換である。
生成する color table entry の `reserved` は常に 0 とする。

利用側の記述を簡単にするため、必要に応じて `BmpEncodeOptions::new()` と
`with_pixel_encoding` / `with_orientation` / `with_resolution` のような builder-style method を追加する。
ただし、基本形は `Default` と struct update syntax で表現できるようにする。

後続版で option として追加する候補は次の通りである。

- `Rgba8`
- `Gray8`
- `Gray16` / `Rgb16` / `Rgba16` / gray alpha formats
- lossy indexed color quantization
- 16-bit / 32-bit `BI_BITFIELDS`
- `BITMAPV4HEADER` / `BITMAPV5HEADER`
- color space metadata
- ICC profile data

`BmpEncodeOptions` で指定できるものは、利用側が自然に選びたい出力形式や metadata に限定する。
次のような派生 field は、native representation 構築時に計算する。

- `bfSize`
- `bfOffBits`
- `biSize`
- `biBitCount`
- `biCompression`
- `biSizeImage`
- `biClrUsed`

`biPlanes` は仕様上常に 1 を書く。
`ImageView + BmpEncodeOptions` から `BmpImage` を構築する場合、
`biPlanes`、`bfReserved1`、`bfReserved2` は仕様上の既定値として 0 / 1 を設定する。

## native representation

BMP native representation は、generic `Image` への変換で失われる BMP 固有情報を保持するために用意する。

native representation で保持する対象は以下である。

- `BITMAPFILEHEADER` fields
- DIB header fields
- color masks
- color table
- pixel array

8-bit indexed color BMP の native representation では、
`color_table` に `RGBQUAD` entries を保持し、
`pixel_array` に index data と row padding を含む file 上の pixel array を保持する。

BMP native representation は top-level native API にも追加し、
`decode_native` / `encode_native` では `NativeImage::Bmp(BmpImage)` として扱う。
`encode_native` は native representation の field をできるだけそのまま書き出す API であり、
generic encode のような正規化 API ではない。
利用側が native field を矛盾する形に変更した場合、出力 BMP が仕様上不正になる可能性がある。
ただし、このクレートが未対応としている構造や、安全に書き出せない buffer 長不足などはエラーにする。
出力される BMP の file layout が整合しているか確認したい場合は、
`BmpImage::validate_file_layout` を使う。
この validation は、このクレートが現在対応している BMP 構造の範囲で、
`bfOffBits`、`bfSize`、`biSizeImage`、pixel array length などの整合性を検査する。
`BI_RGB` の `biSizeImage == 0` は BMP 仕様上許容されるため、
`validate_file_layout` でも整合した layout として扱う。
`validate_file_layout` では、現在対応している BMP 構造について次を確認する。

- `bfOffBits == expected_min_pixel_offset`
- `bfSize == bfOffBits + pixel_array.len()`
- `BI_RGB` では、`biSizeImage == 0` または `biSizeImage == pixel_array.len()`
- `pixel_array.len()` が row size と height から計算した必要量と一致する
- color table entry の `reserved == 0`

`decode_native` は unknown gap bytes を保持しないため、
`decode_native` で得た `BmpImage` が常に `validate_file_layout` を通るとは限らない。

V4 / V5 対応を追加する場合は、次も保持対象に加える。

- V4 / V5 color space fields
- V5 color profile fields / profile data

unknown gap bytes や未解釈の application-specific data は保持しない。
byte-for-byte roundtrip は目標にしない。

## 不正データの扱い

次の入力は不正な入力として扱う。

- header が途中で終わる
- pixel data が header から計算される必要量に満たない
- `bfOffBits` が header / color table / mask fields の途中を指す
- `bfOffBits` が入力長を超える
- `biWidth <= 0`
- `biHeight == 0`
- RLE compression と top-down の組み合わせ
- `biPlanes != 1`
- pixel index が color table の範囲外を参照する
- mask が重複している
- mask の set bit が連続していない
- 必要な mask が欠けている

対応外の DIB header、bit depth、compression は unsupported format として扱う。
`bfSize` と実データ長の不一致は、それだけでは不正な入力として扱わない。
