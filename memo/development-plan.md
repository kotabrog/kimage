# Development Plan

## 現在の到達点

初期の土台作りは完了している。

- `Image` / `ImageView` / `PixelFormat`
- `ImageError`
- endian 読み書き helper
- PPM P6 `Rgb8` / `maxval = 255`
- PGM P5 `Gray8` / `maxval = 255`
- BMP 24-bit uncompressed bottom-up
- 各形式の roundtrip example

次の大きな目標は、Netpbm 系フォーマットの基本対応を揃えたうえで、初回の仮リリースとして `main` へマージできる状態にすること。

## ブランチ方針

開発は `develop` を中心に進め、機能ごとに小さなブランチを切る。

ブランチ名は Conventional Commits に近い短い prefix を使う。

```text
refactor/netpbm-parser
feat/ascii-netpbm
feat/pbm-codec
chore/prepare-initial-release
```

## 初回リリースまでの方針

初回リリースでは、Netpbm 系の基本形式を `u8` ベースで一通り扱えることを目標にする。

対象:

- PBM P1 / P4
- PGM P2 / P5
- PPM P3 / P6

初回リリースでは、以下は未対応として明記する。

- 16-bit PPM/PGM samples
- `maxval > 255`
- 複数画像を連結した Netpbm stream
- PAM
- PNG/JPEG/TIFF などの本格フォーマット

## 1. refactor/netpbm-parser

PPM / PGM で重複している Netpbm ヘッダ処理を共通化する。

目的:

- PPM/PGM/PBM の実装を同じ parser に乗せる
- ASCII 形式と binary 形式の追加前に、ヘッダ処理の重複を減らす
- コメント、空白、CRLF、magic、width、height、maxval の扱いを一箇所に集める

想定する作業範囲:

- `src/codecs/netpbm.rs` などの共通モジュールを追加
- PPM / PGM の private `HeaderParser` を共通 parser に置き換える
- 既存の PPM / PGM テストを維持する
- 挙動変更を最小にする

注意点:

- PBM には `maxval` がないため、header parser は `maxval` あり/なしを扱える設計にする
- raster separator の扱いで先頭 pixel byte を捨てないようにする

## 2. feat/ascii-netpbm

ASCII Netpbm 形式を追加する。

対象:

- PPM P3
- PGM P2

方針:

- P3 は `Rgb8` に decode する
- P2 は `Gray8` に decode する
- encode も P3 / P2 を提供する
- 初期対応は `maxval = 255` のみ
- sample 値が `maxval` を超える場合はエラーにする

想定するテスト:

- P3 decode / encode
- P2 decode / encode
- コメント付きヘッダ
- 複数空白、改行、CRLF
- sample 値不足
- sample 値過多
- sample 値が `maxval` を超えるケース
- roundtrip

## 3. feat/pbm-codec

PBM を追加する。

対象:

- PBM P1 ASCII bitmap
- PBM P4 binary bitmap

内部表現:

- 初回リリースでは `PixelFormat::Gray8` に展開する
- PBM の white/black は、ひとまず `0` と `255` の `Gray8` として扱う
- `PixelFormat::Bitmap1` の追加は後回しにする

方針:

- P1 decode / encode
- P4 decode / encode
- P4 の bit packing / unpacking に対応する
- 行末の余り bit を正しく扱う

想定するテスト:

- P1 decode / encode
- P4 decode / encode
- width が 8 の倍数でないケース
- コメント付きヘッダ
- short bitmap data
- invalid bitmap token
- roundtrip

## 4. chore/prepare-initial-release

初回の仮リリースとして `main` へマージできる状態に整える。

想定する作業範囲:

- README の対応表を更新する
- examples の一覧を更新する
- `memo/` の計画を更新する
- public API の名前と公開範囲を見直す
- `make ci` を通す
- `develop` から `main` への PR を準備する

この段階では、PNG にはまだ進まない。

## 初回リリース後の候補

Netpbm 基本対応が固まった後に、PNG の最小 encoder へ進む。

PNG に入る前に必要になる候補:

- `checksum.rs`
  - CRC32
  - Adler-32
- `bitstream.rs`
- `zlib.rs`
- `deflate.rs`

PNG の最初の目標は、filter type 0 と deflate stored block のみで有効な PNG を保存すること。
