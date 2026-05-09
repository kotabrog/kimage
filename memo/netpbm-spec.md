# Netpbm 仕様メモ

このドキュメントは、Netpbm 系 format の仕様を、ファイルの中身が追える形でまとめる。

Netpbm は単一の画像形式名ではなく、単純な raster image format の集合を指す。
このクレートでは、PBM / PGM / PPM / PAM を Netpbm 系として扱う。

## 形式の全体像

Netpbm 系には、次の形式がある。

| magic number | 形式 | 名前 | 内容 | raster |
| --- | --- | --- | --- | --- |
| `P1` | PBM | Portable Bitmap | 白黒2値画像 | ASCII |
| `P2` | PGM | Portable Graymap | グレースケール画像 | ASCII |
| `P3` | PPM | Portable Pixmap | RGB画像 | ASCII |
| `P4` | PBM | Portable Bitmap | 白黒2値画像 | binary |
| `P5` | PGM | Portable Graymap | グレースケール画像 | binary |
| `P6` | PPM | Portable Pixmap | RGB画像 | binary |
| `P7` | PAM | Portable Arbitrary Map | 汎用tuple画像 | binary |

`P1` から `P6` までをまとめて PNM と呼ぶことがある。
PAM は `P7` のみを使う、PNM より一般化された形式である。

## 共通の文字ルール

PNM の header と ASCII raster では、token を whitespace で区切る。

Netpbm の whitespace は次の文字である。

- space
- TAB
- CR
- LF
- VT
- FF

コメントは `#` から始まり、次の CR または LF の直前まで続く。
コメントは whitespace と同じように token の区切りとして扱う。

binary raster の中身は raw data なので、whitespace や `#` を特別扱いしない。

## PNM の基本構造

PNM の基本構造は次の形である。

```text
P?
width height
[maxval]
raster
```

実際には改行位置は固定ではなく、header の各項目は whitespace で区切られていればよい。

### magic number

先頭の2文字で形式を表す。

- `P1`: ASCII PBM
- `P2`: ASCII PGM
- `P3`: ASCII PPM
- `P4`: binary PBM
- `P5`: binary PGM
- `P6`: binary PPM

### width / height

`width` と `height` は画像サイズを表す正の10進整数である。

- `width > 0`
- `height > 0`

### maxval

`maxval` は sample の最大値を表す。
PGM と PPM だけが持つ。
PBM には `maxval` はない。

`maxval` の範囲は次の通りである。

```text
1 <= maxval <= 65535
```

`maxval < 256` の binary PGM / PPM では、1 sample は 1 byte で表す。
`maxval >= 256` の binary PGM / PPM では、1 sample は 2 bytes big-endian で表す。

ASCII PGM / PPM では、各 sample は 0 から `maxval` までの10進整数で表す。

## PBM

PBM は白黒2値画像である。
sample は white または black のどちらかを表す。

PBM では値の意味は次の通りである。

- `0`: white
- `1`: black

### P1: ASCII PBM

P1 の構造は次の形である。

```text
P1
width height
bits...
```

raster は `0` または `1` の ASCII token を `width * height` 個並べる。
token は whitespace で区切る。
コメントを含めることができる。

例:

```text
P1
# 3x2 bitmap
3 2
0 1 0
1 0 1
```

### P4: binary PBM

P4 の構造は次の形である。

```text
P4
width height
binary bits...
```

header の最後の whitespace の直後から binary raster が始まる。

raster は 1 pixel を 1 bit で表す。
各行は byte 単位に詰める。
1 byte の中では、左の pixel から順に most significant bit へ入る。
行末で余った bit は don't care であり、次の行の pixel として扱わない。

必要 byte 数は次の通りである。

```text
bytes_per_row = (width + 7) / 8
raster_size = bytes_per_row * height
```

## PGM

PGM はグレースケール画像である。
1 pixel は 1 sample で表す。
sample は 0 から `maxval` までの値を取る。

値の意味は次の通りである。

- `0`: black
- `maxval`: white

PGM の sample は仕様上 BT.709 transfer function に基づくとされる。
ただし、実運用では線形値や sRGB 値、または明るさ以外の数値として使われることも多い。
このクレートでは、generic `Image` への変換時に色空間変換や gamma 補正は行わず、数値範囲の正規化のみ行う。

### P2: ASCII PGM

P2 の構造は次の形である。

```text
P2
width height
maxval
gray samples...
```

raster は 0 から `maxval` までの ASCII 10進整数を `width * height` 個並べる。
token は whitespace で区切る。
コメントを含めることができる。

例:

```text
P2
3 2
255
0 128 255
255 128 0
```

### P5: binary PGM

P5 の構造は次の形である。

```text
P5
width height
maxval
binary gray samples...
```

header の最後の whitespace の直後から binary raster が始まる。

sample size は `maxval` によって決まる。

- `maxval < 256`: 1 sample は 1 byte
- `maxval >= 256`: 1 sample は 2 bytes big-endian

必要 byte 数は次の通りである。

```text
sample_bytes = if maxval < 256 { 1 } else { 2 }
raster_size = width * height * sample_bytes
```

## PPM

PPM は RGB 画像である。
1 pixel は red, green, blue の 3 samples で表す。
各 sample は 0 から `maxval` までの値を取る。

値の意味は次の通りである。

- `0`: その channel の最小値
- `maxval`: その channel の最大値

PPM の sample は仕様上 BT.709 transfer function に基づくとされる。
ただし、実運用では線形値や sRGB 値として使われることも多い。
このクレートでは、generic `Image` への変換時に色空間変換や gamma 補正は行わず、数値範囲の正規化のみ行う。

### P3: ASCII PPM

P3 の構造は次の形である。

```text
P3
width height
maxval
red green blue samples...
```

raster は RGB の順で、ASCII 10進整数を `width * height * 3` 個並べる。
token は whitespace で区切る。
コメントを含めることができる。

例:

```text
P3
2 1
255
255 0 0   0 0 255
```

### P6: binary PPM

P6 の構造は次の形である。

```text
P6
width height
maxval
binary rgb samples...
```

header の最後の whitespace の直後から binary raster が始まる。

sample は RGB の順で並ぶ。
sample size は `maxval` によって決まる。

- `maxval < 256`: 1 sample は 1 byte
- `maxval >= 256`: 1 sample は 2 bytes big-endian

必要 byte 数は次の通りである。

```text
sample_bytes = if maxval < 256 { 1 } else { 2 }
raster_size = width * height * 3 * sample_bytes
```

## PAM

PAM は PNM より汎用的な Netpbm 形式である。
magic number は常に `P7` で、header は行単位の key-value 形式で書く。
raster は常に binary である。

PAM の基本構造は次の形である。

```text
P7
WIDTH width
HEIGHT height
DEPTH depth
MAXVAL maxval
TUPLTYPE tuple_type
ENDHDR
binary samples...
```

`ENDHDR` の改行直後から binary raster が始まる。

### PAM header fields

PAM header では、次の項目を扱う。

#### WIDTH

画像の幅を表す正の10進整数である。

```text
WIDTH > 0
```

#### HEIGHT

画像の高さを表す正の10進整数である。

```text
HEIGHT > 0
```

#### DEPTH

1 pixel あたりの sample 数を表す正の10進整数である。

例:

- `DEPTH 1`: グレースケールや白黒
- `DEPTH 2`: グレースケール + alpha
- `DEPTH 3`: RGB
- `DEPTH 4`: RGB + alpha

#### MAXVAL

sample の最大値を表す。
範囲は PGM / PPM と同じである。

```text
1 <= MAXVAL <= 65535
```

`MAXVAL < 256` の場合、1 sample は 1 byte で表す。
`MAXVAL >= 256` の場合、1 sample は 2 bytes big-endian で表す。

#### TUPLTYPE

tuple の意味を表す文字列である。
PAM 仕様上は任意項目だが、画像として意味を解釈するためには重要である。

このクレートでは、次の `TUPLTYPE` を画像変換対象として扱う。

- `BLACKANDWHITE`
- `GRAYSCALE`
- `RGB`
- `BLACKANDWHITE_ALPHA`
- `GRAYSCALE_ALPHA`
- `RGB_ALPHA`

unknown `TUPLTYPE` や未指定の `TUPLTYPE` は native API では保持できる。
generic `Image` への変換では、意味が明確な上記 tuple type のみ扱う。

`TUPLTYPE` は複数行に分けて書ける。
複数行がある場合は、それらを連結した値として扱う。

#### ENDHDR

header の終端を表す。
`ENDHDR` の次から binary raster が始まる。
`ENDHDR` がない PAM は不正な header として扱う。

### PAM raster

PAM raster は常に binary である。
ASCII PAM はない。

sample は pixel ごとに `DEPTH` 個並ぶ。
必要 byte 数は次の通りである。

```text
sample_bytes = if MAXVAL < 256 { 1 } else { 2 }
raster_size = WIDTH * HEIGHT * DEPTH * sample_bytes
```

`MAXVAL >= 256` の sample は file 上では 2 bytes big-endian である。

### PAM tuple type の値の意味

`BLACKANDWHITE` は PBM と値の意味が逆である。

- `0`: black
- `1`: white

`GRAYSCALE` は PGM と同じである。

- `0`: black
- `MAXVAL`: white

`RGB` は PPM と同じである。

- sample order は red, green, blue

alpha 付き tuple type では、最後の sample を alpha として扱う。

- `0`: transparent
- `MAXVAL`: opaque

## multi-image stream

Netpbm の binary format は、同一 subformat の画像を区切りなしで複数連結できる。

このクレートでは、multi-image stream は次の形式を対象にする。

- PBM P4
- PGM P5
- PPM P6
- PAM P7

P1 / P2 / P3 の plain format は仕様上、1ファイル1画像として扱う。
そのため、このクレートでも P1 / P2 / P3 の multi-image stream は扱わない。

PNM の multi-image stream は、最初の magic number で subformat を決める。
途中で異なる subformat が現れる入力は、標準的な stream として扱わない。

## native representation

PNM / PAM の file 上の sample 値は、generic `Image` へ変換するときに正規化されることがある。
元の `maxval` や sample 値を保持したい場合は native representation を使う。

### PBM native

PBM native は、white / black の値を PBM の意味のまま保持する。

- `0`: white
- `1`: black

### PGM / PPM native

PGM / PPM native は、`maxval` と sample 値を保持する。
sample 値は正規化しない。

`maxval >= 256` の sample は、crate 内部では little-endian の `u16` として保持する。
file との境界では big-endian と相互変換する。

### PAM native

PAM native は、`WIDTH`, `HEIGHT`, `DEPTH`, `MAXVAL`, `TUPLTYPE`, raster data を保持する。
unknown `TUPLTYPE` や未指定の `TUPLTYPE` も保持できる。

`MAXVAL >= 256` の sample は、crate 内部では little-endian の `u16` として保持する。
file との境界では big-endian と相互変換する。

## generic Image への変換

generic `Image` では、扱いやすい pixel format に変換する。

PBM は `Gray8` に変換する。

- PBM `0` は `255` に変換する。
- PBM `1` は `0` に変換する。

PGM は `maxval` に応じて変換する。

- `maxval < 256`: `Gray8`
- `maxval >= 256`: `Gray16`

PPM は `maxval` に応じて変換する。

- `maxval < 256`: `Rgb8`
- `maxval >= 256`: `Rgb16`

PAM は `TUPLTYPE` と `MAXVAL` に応じて変換する。

- `BLACKANDWHITE`: `Gray8`
- `GRAYSCALE`: `Gray8` または `Gray16`
- `RGB`: `Rgb8` または `Rgb16`
- `BLACKANDWHITE_ALPHA`: `GrayAlpha8`
- `GRAYSCALE_ALPHA`: `GrayAlpha8` または `GrayAlpha16`
- `RGB_ALPHA`: `Rgba8` または `Rgba16`

PGM / PPM / PAM の `maxval` が target pixel format の最大値と異なる場合は、target bit depth の full range に正規化する。

## encode

PNM encode では、出力する subformat を明示的に指定する。

- P1: ASCII PBM
- P2: ASCII PGM
- P3: ASCII PPM
- P4: binary PBM
- P5: binary PGM
- P6: binary PPM

PAM encode では、出力する tuple type を明示的に指定する。

encode 時は、指定した format / tuple type と `ImageView` の pixel format が一致する場合のみ書き出す。
意味が曖昧な暗黙変換は行わない。

native encode では、native representation が保持する `maxval` と sample 値を書き出す。
