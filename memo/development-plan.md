# Development Plan

## 現在の到達点

初期の土台作りと Netpbm の基本形式対応は完了している。

- `Image` / `ImageView` / `PixelFormat`
- `ImageError`
- endian 読み書き helper
- Netpbm 共通 parser / helper
- PBM P1 / P4
- PGM P2 / P5
- PPM P3 / P6
- BMP 24-bit uncompressed bottom-up
- 各形式の roundtrip example

次の大きな目標は、Netpbm 系フォーマットについて仕様上の残りを整理して実装し、やり残しを明確にしたうえで初回リリースへ進むこと。

## ブランチ方針

開発は `develop` を中心に進め、機能ごとに小さなブランチを切る。

ブランチ名は Conventional Commits に近い短い prefix を使う。

```text
docs/netpbm-complete-support-plan
fix/netpbm-parser-spec
feat/netpbm-native-image
feat/netpbm-normalization
feat/netpbm-16bit
feat/netpbm-multi-image
docs/pam-support-plan
feat/pam-codec
chore/prepare-initial-release
```

## Netpbm 完全対応に向けた方針

初回リリース前に、PBM / PGM / PPM / PAM の対応範囲を一段広げる。

対象:

- PBM P1 / P4
- PGM P2 / P5
- PPM P3 / P6
- PGM / PPM の `maxval` 1..65535
- 8-bit と 16-bit の sample
- Netpbm の `maxval` を保持する native image 型
- native image から汎用 `Image` への正規化変換
- 複数画像を連結した Netpbm stream
- PAM P7

ただし、色空間変換や gamma 補正は行わない。PGM / PPM 仕様では BT.709 gamma transfer function が記述されているが、このクレートでは sample 値の読み書きを扱い、値の色空間解釈は利用側に任せる。

## 1. docs/netpbm-complete-support-plan

Netpbm 完全対応に向けて、仕様と実装順を整理する。

目的:

- 現在の `u8` / `maxval = 255` 前提から、Netpbm 仕様全体を見据えた計画へ更新する
- PBM / PGM / PPM / PAM の差分を整理する
- `PixelFormat` と public API に影響する判断点を明確にする
- Netpbm native image と汎用 `Image` の役割分担を決める

想定する作業範囲:

- `memo/development-plan.md` を更新する
- `memo/codex-project-overview.md` を更新する
- 実装には触れない

判断が必要な点:

- multi-image stream API をどう設計するか
- PAM P7 の詳細対応範囲をどの時点で固めるか

## 2. fix/netpbm-parser-spec

Netpbm parser を仕様に寄せる。

目的:

- 後続の native image 対応、16-bit 対応、multi-image 対応の土台を安定させる
- 現在の parser の仕様漏れを先に潰す

想定する作業範囲:

- whitespace として space, TAB, CR, LF, VT, FF を扱う
- コメントを `#` から次の CR または LF の直前までとして扱う
- `maxval` を `1..65535` として読み、0 と 65536 以上をエラーにする
- raster 長計算を overflow しない共通 helper に寄せる
- binary PGM / PPM / PBM で必要なraster長を正確に消費する
- trailing data を「次画像の可能性があるデータ」として扱えるよう、parser位置を正確に管理する

注意点:

- P1 / P2 / P3 は plain 形式で、仕様上は単一画像形式として扱われる
- P4 / P5 / P6 は raw 形式で、複数画像 stream の構成要素になり得る
- P1 の raster 後には whitespace で始まる junk を許容する仕様がある

## 3. feat/netpbm-native-image

Netpbm の `maxval` やsubformatを保持できる native image 型を追加する。

方針:

- 既存の `Image` は正規化済みの汎用画像バッファとして維持する
- Netpbm用に `NetpbmImage` のような保持型を追加する
- PBM / PGM / PPM は enum variant で分け、PBM に不要な `maxval` を `Option` で持たせない
- PGM / PPM は `maxval: u16` を保持する
- sample 値は正規化せず、ファイル上の値域 `0..maxval` を保持する
- PBM native data は `0 = white`, `1 = black` のNetpbm仕様値として保持する
- PGM / PPM の `maxval < 256` は 1 byte/sample として保持する
- PGM / PPM の `maxval >= 256` は 2 bytes/sample の little-endian として保持する
- file I/O境界では、Netpbm binary 16-bit sample の big-endian と内部 little-endian を変換する

想定するテスト:

- P1 / P4 の native decode
- P2 / P3 / P5 / P6 の `maxval = 1`, `15`, `100`, `255`
- native decode が `maxval` を保持すること
- native encode が元の `maxval` を出力すること
- sample 値が `maxval` を超えるケース
- PBM native data が `0 = white`, `1 = black` を保持すること

API候補:

```rust
pub enum NetpbmImage {
    Pbm {
        width: u32,
        height: u32,
        data: Vec<u8>,
    },
    Pgm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
    Ppm {
        width: u32,
        height: u32,
        maxval: u16,
        data: Vec<u8>,
    },
}
```

公開API候補:

```rust
pbm::decode_native(...)
pgm::decode_native(...)
ppm::decode_native(...)

pbm::encode_native(...)
pgm::encode_native(...)
ppm::encode_native(...)
```

## 4. feat/netpbm-normalization

Netpbm native image から汎用 `Image` へ正規化できるようにする。

方針:

- `decode` / `decode_ascii` は、native decode の結果を正規化して `Image` を返すAPIとして維持する
- `NetpbmImage` から `Image` へ変換する helper を追加する
- `Gray8` / `Rgb8` へ正規化する場合は `0..255` へスケーリングする
- `Gray16` / `Rgb16` へ正規化する場合は `0..65535` へスケーリングする
- PBM は `Gray8` または `Gray16` に変換し、`0 = white`, `1 = black` を通常の grayscale 値へ変換する
- 色空間変換や gamma 補正は行わない

想定するテスト:

- PGM / PPM native `maxval = 100` を `Gray8` / `Rgb8` へ正規化する
- PGM / PPM native `maxval = 1000` を `Gray16` / `Rgb16` へ正規化する
- PBM native を `Gray8` / `Gray16` へ正規化する
- 正規化の丸め

注意点:

- 正規化の丸め規則を固定する必要がある
- 推奨は `(sample * target_max + maxval / 2) / maxval` による最近傍丸め

## 5. feat/netpbm-16bit

PGM / PPM の 16-bit sample を扱えるようにする。

方針:

- `maxval >= 256` の binary PGM / PPM は 2 bytes/sample として読む
- ファイル上の2 bytes/sampleは big-endian として読む
- native image では元sample値を little-endian の `u16` として保持する
- 正規化済み `Image` 用に `PixelFormat::Gray16` / `PixelFormat::Rgb16` を追加する
- `Image` の `data: Vec<u8>` は維持する
- `Gray16` / `Rgb16` の内部表現は little-endian に統一する
- `decode_native` は元sample値を保持し、`decode` は正規化済み `Image` を返す

想定するテスト:

- P5 16-bit decode / encode
- P6 16-bit decode / encode
- P2 / P3 の `maxval >= 256`
- `maxval = 256`, `65535`
- short raster data
- sample 値が `maxval` を超えるケース

## 6. feat/netpbm-multi-image

複数画像を連結した Netpbm stream を扱えるようにする。

方針:

- 既存の `decode` / `decode_ascii` は単一画像を読むAPIとして維持する
- `pbm::decode_all_native`, `pgm::decode_all_native`, `ppm::decode_all_native` を追加する
- 必要に応じて、正規化済み `Image` を返す `decode_all` も追加する
- 形式別の `decode_all` は仕様に沿って同一subformatのstreamだけを扱う
- 上位APIとして `pnm::decode_native` / `pnm::decode_all_native` も追加する
- `pnm::decode_native` は P1..P6 を magic number で自動判別して1枚読む
- `pnm::decode_all_native` は最初のmagic numberでsubformatを決め、そのsubformatのstreamとして読む
- `pnm::decode_all_native` は異なるsubformatの混在streamを標準対応しない
- 必要なら `pbm::Decoder`, `pgm::Decoder`, `ppm::Decoder`, `pnm::Decoder` 型を追加し、1枚ずつ読み進められるようにする
- raw形式ではraster直後に次画像が続くため、余分なデータを単純なエラーにしない
- plain形式 P1 / P2 / P3 は仕様上1ファイル1画像として扱い、multi-image APIでは複数画像streamの対象にしない
- plain形式で複数画像のような入力が来た場合は、仕様重視でエラーにする

想定するテスト:

- P4 / P5 / P6 の2画像連結
- native image の `maxval` が画像ごとに保持されること
- `pnm::decode_all_native` が最初のmagic numberでsubformatを決めること
- `pnm::decode_all_native` が異なるsubformatの混在streamをエラーにすること
- 途中で壊れた2枚目のエラー
- 空入力
- 1枚だけの入力
- P1 / P2 / P3 の複数画像風入力をエラーにすること

## 7. docs/pam-support-plan

PAM P7 の詳細計画を別途整理する。

PAM は PBM / PGM / PPM とはヘッダ構造が大きく違い、alpha 付き tuple type や任意の `DEPTH` も絡むため、実装直前に改めて対応範囲を決める。

方針:

- 可能であれば PAM P7 は広く対応する
- ただし、詳細な対応範囲は PGM / PPM の 16-bit 対応と multi-image API が固まった後に決める
- 特に alpha 付き tuple type は `PixelFormat` の追加方針と合わせて判断する

検討する項目:

- 対応する `TUPLTYPE`
- `DEPTH` と `PixelFormat` の対応表
- `TUPLTYPE` なしのPAMをどこまで受け入れるか
- alpha 付き tuple type の内部表現
- PAM の multi-image stream 対応

## 8. feat/pam-codec

PAM P7 を追加する。

候補:

- `TUPLTYPE BLACKANDWHITE`
- `TUPLTYPE GRAYSCALE`
- `TUPLTYPE RGB`
- `TUPLTYPE GRAYSCALE_ALPHA`
- `TUPLTYPE RGB_ALPHA`

方針:

- `WIDTH`, `HEIGHT`, `DEPTH`, `MAXVAL`, `ENDHDR` を必須として扱う
- `TUPLTYPE` は任意だが、対応 tuple type の判定に使う
- `MAXVAL` は `1..65535`
- raster sample は maxval に応じた最小byte数で、big-endianとして読む
- PBM相当の `BLACKANDWHITE` は PAM では `0 = black`, `1 = white` であり、PBM の `0 = white`, `1 = black` と逆である点に注意する

想定するテスト:

- GRAYSCALE / RGB の decode / encode
- BLACKANDWHITE の decode
- 必須header不足
- 重複header
- 不明header
- `ENDHDR` 不足
- alpha付き tuple type を対応する場合は `GrayAlpha8` / `Rgba8` などの表現テスト

## 9. chore/prepare-initial-release

Netpbm の対応範囲を反映したうえで、初回の仮リリースとして `main` へマージできる状態に整える。

想定する作業範囲:

- README の対応表を更新する
- examples の一覧を更新する
- `memo/` の計画を更新する
- public API の名前と公開範囲を見直す
- `make ci` を通す
- `develop` から `main` への PR を準備する

この段階では、PNG にはまだ進まない。

## 初回リリース後の候補

Netpbm の対応範囲が固まった後に、PNG の最小 encoder へ進む。

PNG に入る前に必要になる候補:

- `checksum.rs`
  - CRC32
  - Adler-32
- `bitstream.rs`
- `zlib.rs`
- `deflate.rs`

PNG の最初の目標は、filter type 0 と deflate stored block のみで有効な PNG を保存すること。
