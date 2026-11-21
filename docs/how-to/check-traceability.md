# コンポーネント境界とテスト対象を生成する

## 解析用 Rust を準備する

[解析用ツールチェーンの設定](../../tools/traceability/rust-toolchain.toml)に従って Rust を準備する。
次のコマンドは固定したツールチェーンを選択し、未導入ならインストールする。

```sh
cd tools/traceability
rustc --version
```

以降のコマンドはリポジトリルートで実行する。

## テストの対象を指定する

対象を指定する package の `Cargo.toml` に開発用依存を追加する。
`path` はその package からの相対パスに合わせる。

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
セットアップに使っただけの constructor を、テスト対象として記述しない。
通信だけを検証するテストには、便宜的なローカル API を割り当てない。

通常ビルドに存在する API を対象にする。
テスト専用の項目や、マクロ・`cfg` により通常ビルドとの対応が取れない項目を指定すると、生成に失敗する。
対象の指定は作者の意図であり、契約を十分に検証することの証明ではない。

## 生成結果を検査する

```sh
python3 scripts/traceability.py check
```

テスト本体は実行しないため、PostgreSQL と Valkey は不要である。

## HTML viewer を開く

```sh
python3 scripts/traceability.py viewer
```

`target/traceability-viewer.html` をブラウザーで開く。
出力先を変更するには `--output /tmp/traceability.html` を指定する。
package と呼出の矢印を選び、API から対象テストやソースを確認する。
対象指定の有無とテストの実行結果は区別する。

## Viewer のブラウザーテストを実行する

Node.js と Python 3 を準備して実行する。

```sh
cd tools/traceability-viewer-tests
npm ci
npx playwright install chromium
npm test
```

## テストを実行する

PostgreSQL と Valkey を使える環境で実行する。
DB の準備には[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)のラッパーを使う。

```sh
scripts/test-postgres.sh python3 scripts/traceability.py run
scripts/test-postgres.sh python3 scripts/traceability.py run --kind targeted
```

`--kind targeted` は対象属性を持つテストを選ぶ。
`--kind untargeted` は対象属性のないテストを選ぶ。
省略時は一覧中の全テストを実行する。
独自の Cargo target runner を設定している場合は、設定を外して実行する。
