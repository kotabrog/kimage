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
- PNM P1..P6 上位API
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
feat/pnm-api
chore/pam-p7-planning
feat/pam-codec
chore/prepare-initial-release
```

## Netpbm 完全対応に向けた方針

初回リリース前に、PBM / PGM / PPM / PAM の対応範囲を一段広げる。

対象:

- PBM P1 / P4
- PGM P2 / P5
- PPM P3 / P6
- PGM / PPM の `maxval` 1..=65535
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
- `maxval` を `1..=65535` として読み、0 と 65536 以上をエラーにする
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
- 必要なら `pbm::Decoder`, `pgm::Decoder`, `ppm::Decoder` 型を追加し、1枚ずつ読み進められるようにする
- raw形式ではraster直後に次画像が続くため、余分なデータを単純なエラーにしない
- plain形式 P1 / P2 / P3 は仕様上1ファイル1画像として扱い、multi-image APIでは複数画像streamの対象にしない
- plain形式で複数画像のような入力が来た場合は、仕様重視でエラーにする

想定するテスト:

- P4 / P5 / P6 の2画像連結
- native image の `maxval` が画像ごとに保持されること
- 途中で壊れた2枚目のエラー
- 空入力
- 1枚だけの入力
- P1 / P2 / P3 の複数画像風入力をエラーにすること

## 7. feat/netpbm-image-conversion

`NetpbmImage` と汎用 `Image` の変換 API を公開する。

方針:

- 既存の private helper を整理し、`NetpbmImage -> Image` の公開 API を追加する
- `NetpbmImage -> Image` は既存 `decode` / `decode_ascii` と同じ正規化規則を使う
- PBM は `0 = white`, `1 = black` を `Gray8` の `255 = white`, `0 = black` に変換する
- PGM / PPM の `maxval < 256` は `Gray8` / `Rgb8` に正規化する
- PGM / PPM の `maxval >= 256` は `Gray16` / `Rgb16` に正規化する
- 変換時の丸めは既存通り `(sample * target_max + maxval / 2) / maxval` を使う
- `Image` または `ImageView` から `NetpbmImage` への変換も追加する
- `Gray8` / `Rgb8` からは `maxval = 255` の PGM / PPM に変換する
- `Gray16` / `Rgb16` からは `maxval = 65535` の PGM / PPM に変換する
- PBM への変換は threshold 方針が絡むため、必要性が明確なら専用関数として追加する

API候補:

```rust
impl TryFrom<NetpbmImage> for Image
```

```rust
impl NetpbmImage {
    pub fn to_image(&self) -> Result<Image>;
}
```

```rust
pub fn image_view_to_pgm_native(image: ImageView<'_>) -> Result<NetpbmImage>;
pub fn image_view_to_ppm_native(image: ImageView<'_>) -> Result<NetpbmImage>;
pub fn image_view_to_pam_native(
    image: ImageView<'_>,
    tuple_type: PamEncodeTupleType,
) -> Result<PamImage>;
```

想定するテスト:

- PBM native を `Gray8` に変換すること
- PGM / PPM native `maxval < 256` を `Gray8` / `Rgb8` に正規化すること
- PGM / PPM native `maxval >= 256` を `Gray16` / `Rgb16` に正規化すること
- `Gray8` / `Rgb8` を `maxval = 255` の native image に変換すること
- `Gray16` / `Rgb16` を `maxval = 65535` の native image に変換すること
- unsupported pixel format をエラーにすること

## 8. feat/pnm-api

P1..P6 を magic number で自動判別する上位APIを追加する。

方針:

- 形式別APIは `pbm.rs` / `pgm.rs` / `ppm.rs` に残す
- `pnm.rs` は、呼び出し側が事前にsubformatを判定したくない場合の入口にする
- `pnm::decode_native` は P1..P6 を magic number で自動判別して1枚読む
- `pnm::decode` は `pnm::decode_native` の結果を正規化済み `Image` に変換する
- `pnm::decode_all_native` は最初のmagic numberでsubformatを決め、そのsubformatのstreamとして読む
- `pnm::decode_all_native` は異なるsubformatの混在streamを標準対応しない
- 必要なら `pnm::Decoder` 型を追加し、1枚ずつ読み進められるようにする
- `pnm::encode` は `PnmEncodeFormat` で P1..P6 を明示指定する
- `pnm::encode_all` / `pnm::encode_all_native` は P4 / P5 / P6 の binary multi-image stream のみ扱う
- P1 / P2 / P3 は仕様上1ファイル1画像として扱い、multi-image APIでは `UnsupportedFormat` にする

想定するテスト:

- P1..P6 を magic number で自動判別して読めること
- `pnm::decode` が正規化済み `Image` を返すこと
- `pnm::decode_native` が `NetpbmImage` の適切な variant を返すこと
- `pnm::decode_all_native` が最初のmagic numberでsubformatを決めること
- `pnm::decode_all_native` が異なるsubformatの混在streamをエラーにすること
- `pnm::encode` が P1..P6 を明示指定して書けること
- `pnm::encode_all` が P4 / P5 / P6 の multi-image stream を書けること
- unsupported magic number をエラーにすること

## 9. chore/pam-p7-planning

PAM P7 の詳細計画を整理する。

PAM は PBM / PGM / PPM とはヘッダ構造が大きく違い、alpha 付き tuple type や任意の `DEPTH` も絡むため、実装直前に改めて対応範囲を決める。

方針:

- PAM は `NetpbmImage` には混ぜず、別途 `PamImage` を追加する
- native API は PAM raster として妥当な範囲を広めに扱う
- `TUPLTYPE` が未指定または未知でも、`decode_native` では保持できるようにする
- 汎用 `Image` への変換は、意味が明確で `PixelFormat` に対応できる tuple type のみ対応する
- PAM の multi-image stream は初回から対応する
- `DEPTH` が tuple type の期待値より大きい入力は、初期実装では受け入れずエラーにする

検討する項目:

- `PamImage` / `PamTupleType` の public API
- `decode_native` / `decode_all_native` / `encode_native` / `encode_all_native` の仕様
- `decode` / `encode` で `Image` / `ImageView` と相互変換する対応範囲
- alpha 付き tuple type のために追加する `PixelFormat`
- unknown `TUPLTYPE` の保持方法

決定事項:

- `PamImage` は `width`, `height`, `depth`, `maxval`, `tuple_type`, `data` を持つ native 保持型にする
- `PamTupleType` は公式 tuple type と `Other(String)` を表現する
- `TUPLTYPE` なしは native では許可し、汎用 `Image` への変換では `UnsupportedFormat` とする
- `WIDTH`, `HEIGHT`, `DEPTH`, `MAXVAL`, `ENDHDR` は必須
- `MAXVAL` は `1..=65535`
- file raster は `maxval < 256` なら1 byte/sample、`maxval >= 256` なら2 bytes/sample big-endian
- native `data` は `maxval >= 256` の sample を little-endian `u16` として保持する
- `BLACKANDWHITE` は PAM 仕様通り `0 = black`, `1 = white` として扱う
- `GrayAlpha8`, `GrayAlpha16`, `Rgba16` を追加し、alpha 付き tuple type も汎用 `Image` に変換できるようにする
- `decode` は `BLACKANDWHITE`, `GRAYSCALE`, `RGB`, `BLACKANDWHITE_ALPHA`, `GRAYSCALE_ALPHA`, `RGB_ALPHA` を汎用 `Image` に変換する
- `encode` は `PamEncodeTupleType` で tuple type を明示指定し、`ImageView` の `PixelFormat` と合う場合のみ書き出す

## 10. feat/pam-codec

PAM P7 を追加する。

候補:

- `TUPLTYPE BLACKANDWHITE`
- `TUPLTYPE GRAYSCALE`
- `TUPLTYPE RGB`
- `TUPLTYPE BLACKANDWHITE_ALPHA`
- `TUPLTYPE GRAYSCALE_ALPHA`
- `TUPLTYPE RGB_ALPHA`

方針:

- `pam.rs` を追加する
- `PamImage` と `PamTupleType` を追加する
- `WIDTH`, `HEIGHT`, `DEPTH`, `MAXVAL`, `ENDHDR` を必須として扱う
- `TUPLTYPE` は任意として扱い、native では未指定を保持する
- 複数 `TUPLTYPE` は仕様通り空白区切りで連結する
- `MAXVAL` は `1..=65535`
- raster sample は maxval に応じた最小byte数で、ファイル上 big-endian として読む
- native `data` では 16-bit sample を little-endian として保持する
- PBM相当の `BLACKANDWHITE` は PAM では `0 = black`, `1 = white` であり、PBM の `0 = white`, `1 = black` と逆である点に注意する
- unknown `TUPLTYPE` は `decode_native` で保持し、`decode` では `UnsupportedFormat` にする
- `decode_all_native` / `encode_all_native` で PAM multi-image stream を扱う

想定するテスト:

- GRAYSCALE / RGB の native decode / encode
- BLACKANDWHITE の native decode / encode
- RGB_ALPHA + `maxval < 256` の `Rgba8` decode / encode
- BLACKANDWHITE_ALPHA の `GrayAlpha8` decode / encode
- GRAYSCALE_ALPHA の `GrayAlpha8` / `GrayAlpha16` decode / encode
- RGB_ALPHA + `maxval >= 256` の `Rgba16` decode / encode
- `maxval >= 256` の 16-bit sample が内部 little-endian で保持されること
- `TUPLTYPE` なしを native で読めること
- unknown `TUPLTYPE` を native で保持できること
- `decode` が unknown `TUPLTYPE` を `UnsupportedFormat` にすること
- multi-image stream の decode / encode
- 必須header不足
- 重複header
- 不明header
- `ENDHDR` 不足
- tuple type と `DEPTH` が一致しない場合
- `MAXVAL` が 0 または 65536 以上のケース
- short raster data
- sample 値が `MAXVAL` を超えるケース
- tuple type 指定と `ImageView` の `PixelFormat` が一致しない場合

API候補:

```rust
pub struct PamImage {
    pub width: u32,
    pub height: u32,
    pub depth: u32,
    pub maxval: u16,
    pub tuple_type: Option<PamTupleType>,
    pub data: Vec<u8>,
}

pub enum PamTupleType {
    BlackAndWhite,
    Grayscale,
    Rgb,
    BlackAndWhiteAlpha,
    GrayscaleAlpha,
    RgbAlpha,
    Other(String),
}
```

```rust
pam::decode(...)
pam::decode_all(...)
pam::decode_native(...)
pam::decode_all_native(...)

pam::encode(..., PamEncodeTupleType)
pam::encode_all(..., PamEncodeTupleType)
pam::encode_native(...)
pam::encode_all_native(...)
```

## 11. chore/prepare-initial-release

Netpbm の対応範囲を反映したうえで、初回の仮リリースとして `main` へマージできる状態に整える。

想定する作業範囲:

- README の対応表を更新する
- examples の一覧を更新する
- `memo/` の計画を更新する
- public API の名前と公開範囲を見直す
- `make ci` を通す
- `develop` から `main` への PR を準備する

この段階では、PNG にはまだ進まない。

詳細方針:

- 新機能追加ではなく、説明・計画・公開APIの整理に集中する
- README は利用者が現在の対応範囲を把握しやすい形にする
- `memo/` は「完了済み」と「初回リリース後」を分け、次の作業が読み取れる状態にする
- public API は大きく作り替えず、初回リリース前に破壊的変更すべきものがないか軽く棚卸しする
- examples と CI を実行し、README に載せている内容が実際に動くことを確認する

README で確認・整理する項目:

- 対応形式を表または読みやすいセクションで整理する
- PBM / PGM / PPM / PNM / PAM / BMP の supported / unsupported を最新実装に合わせる
- 通常 API と native API の違いを短く説明する
- `Image` / `ImageView` / `PixelFormat` の役割を最小限説明する
- examples の一覧を出力ファイル名付きで整理する
- PNM は P1..P6 の上位API、PAM は P7 専用APIであることを明確にする

public API の確認項目:

- `pub mod io` を公開 API として残すか判断する
- `codecs::netpbm` は private のまま、`NetpbmImage` と変換関数だけ re-export する形でよいか確認する
- `PnmEncodeFormat` / `PamEncodeTupleType` の命名を初回リリース前に確認する
- `decode_all` / `decode_all_native` / `encode_all` / `encode_all_native` の命名が形式間で揃っているか確認する
- `ImageError` の variant と message が利用者に伝わるものになっているか確認する

public API の個別対応候補:

- `io` module は内部 endian helper として扱い、crate root から公開しない
- `decode_all` の有無を形式間で揃える
  - `pam` は `decode_all` / `decode_all_native` を持つ
  - `pbm` / `pgm` / `ppm` / `pnm` にも正規化済み `decode_all` を追加し、形式間で揃える
- `ImageError::Io` は `std::io::Error` を保持し、`Error::source()` から元の IO error を参照できる形にする
  - `Clone` / `Eq` は実装しない
  - 非IOエラーのテストや比較用途のため、`PartialEq` は手書きで維持する
- `image_view_to_pbm_native`, `image_view_to_pgm_native`, `image_view_to_ppm_native`, `image_view_to_pam_native` に名前を揃える
  - `ImageView` から native image 型へ変換する helper として公開する
  - 将来的に `TryFrom` などの変換APIを追加する余地は残す
- `Image` / `ImageView` の public field 方針を確認する
  - 現状は小さい画像バッファ型として扱いやすい
  - 不変条件をより強く守る設計にするなら getter 中心も候補だが、初回リリースでは現状維持を基本にする

リポジトリ・crate metadata の確認項目:

- `Cargo.toml` の `description`, `repository`, `readme`, `license` を確認する
- `license = "MIT OR Apache-2.0"` に合わせ、`LICENSE-MIT` / `LICENSE-APACHE` の追加を検討する
- crates.io publish を急がない場合、`keywords` / `categories` は必須にしない

動作確認:

- README に載せている examples を一通り実行する
- `pamtopng` や `pamsplit` など外部コマンドがない環境でも example が失敗しないことを確認する
- 最後に `make ci` を通す
- 必要に応じて `cargo package --list` で crate に含まれるファイルを確認する

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
