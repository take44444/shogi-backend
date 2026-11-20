# コンポーネント境界とテスト対象を生成する

コンポーネント間の Rust API 呼出は、コンパイラの情報から抽出する。
テストが何を対象にするかは `targets` 属性で明示する。
API を呼んだ事実からテストの目的を推測しない。

## 解析用 Rust を準備する

製品の通常ビルドとは別に、固定した解析用ツールチェーンを使う。

```sh
rustup toolchain install nightly-2026-11-18 --profile minimal --component rustc-dev,llvm-tools,rust-src
```

解析器は [tools/traceability](../../tools/traceability/Cargo.toml) にある。
ツールチェーンの固定は同ディレクトリの `rust-toolchain.toml` にある。

## テストの対象を指定する

対象を指定する package の `Cargo.toml` に開発用依存を追加する。

```toml
[dev-dependencies]
traceability = { package = "traceability-attributes", path = "../../tools/traceability-attributes" }
```

テスト属性の前に、対象の関数またはメソッドへの Rust の参照を記述する。

```rust
#[traceability::targets(shogi_domain::Game::play)]
#[test]
fn rejected_move_preserves_position() {
    // 対象の振る舞いを検証する。
}
```

複数の対象はカンマで区切る。
trait のインターフェースを対象にするテストでは、dyn 互換な trait のメソッドを `<dyn Trait>::method` として指定できる。
具体的な実装を対象にするテストには、実装型のメソッドを指定する。
実装型のテストを trait 境界のテストに対応付けない。

属性の参照先はコンパイラが解決する。
存在しない項目やアクセスできない項目はコンパイルに失敗する。
名前の一致、文字列 ID、別の契約登録ファイルで対応を作らない。

セットアップに使っただけの constructor を、テスト対象として記述しない。
通信だけを検証するテストには、便宜的なローカル API を割り当てない。
対象を指定していないテストも一覧には残る。
対象の指定はテスト作者の意図であり、その assertion が契約を十分に検証することの証明ではない。
指定した定義 ID が通常ビルドにも存在することを検査する。
テスト専用の項目や、マクロ・`cfg` により通常ビルドとの対応が取れない項目を指定すると、生成に失敗する。
`ambiguous compiler API identity` が出た場合は、診断に示された生成コードに識別可能なソース位置を与えるか、無名スコープ内の定義を通常のソースコードへ移す。
対象に指定しない曖昧な定義は `compilation-occurrence` として収集する。
viewer の API 詳細には観測した compiler unit と理由が表示される。
これらの定義は異なる unit 間で対応付けず、契約テストの対象にも使わない。

## 生成結果を検査する

```sh
python3 scripts/traceability.py check
```

対象の workspace を解析用コンパイラでビルドし、実際の Cargo テスト一覧と抽出したテストを照合する。
テスト本体は実行しないため、この操作には PostgreSQL と Valkey は不要である。
コンパイル設定と未解決の呼出を生成結果に記録する。
選択した target と feature 設定の外にある `cfg` 専用コードまで解析したとは扱わない。

HTTP、gRPC、メッセージの送受信を対応付ける関係は生成しない。
HTTP adapter から application への通常のローカル Rust 呼出は含める。
`dyn Trait` の呼出先は trait の宣言で止め、注入された具体型まで追わない。

## HTML viewer を開く

```sh
python3 scripts/traceability.py viewer
```

`target/traceability-viewer.html` をブラウザーで開く。
出力先を変更するには `--output /tmp/traceability.html` を指定する。
コンポーネントから API への呼出と、属性で指定したテスト対象を確認する。
取り込んだソースと、その位置を viewer 内で開ける。

## テストを実行する

```sh
scripts/test-postgres.sh python3 scripts/traceability.py run
scripts/test-postgres.sh python3 scripts/traceability.py run --kind targeted
```

`--kind targeted` は対象属性を持つテストを選ぶ。
`--kind untargeted` は対象属性のないテストを選ぶ。
省略時は一覧中の全テストを実行する。
実行結果と静的な対象指定を区別して確認する。
解析時に照合したテスト実行ファイルを、各 package のディレクトリで実行する。
package 単体で再ビルドせず、workspace 内で有効になった feature を保持する。
Cargo が実際に渡す package 情報、build script の環境変数、動的ライブラリの探索先も保持する。
これらの値は実行用にだけ使い、生成する JSON や HTML には保存しない。
実行環境の取得には一時的な Cargo runner を使うため、独自の target runner を設定した環境は対象外とする。

生成モデルと抽出範囲は[設計](../design/code-derived-boundaries.md)を参照する。
