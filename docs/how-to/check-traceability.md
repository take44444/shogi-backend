# 機能の保証とテストの対応を更新する

HTML viewer は Rust の宣言と実在するテストから生成する。
保証やテスト一覧を手書きの文書に複写しない。
保証、契約、検証範囲、根拠、未検証事項の説明は日本語で書く。
機能・契約の ID、列挙値、関数名、パスなどのコード上の識別子は維持する。
[永続イベント配信の定義](../../crates/game-application/src/trace/durable-event-dispatch/feature.rs)と[共通の型とマクロ](../../scripts/traceability_model.rs)を例にする。

1. 機能の `src/trace/<feature>/feature.rs` または `tests/trace/<feature>/feature.rs` を更新する。
   `src` と `tests` は Cargo package の直下とする。
   同じ機能の `feature.rs` は workspace 全体で一つにし、保証、処理の流れ、限界を宣言する。
   `<feature>/<contract>/contract.rs` にコンポーネントの契約、実装の `source`、未検証事項の `gap` を宣言する。
   宣言は `#[cfg(test)]` の Rust モジュールとしてコンパイルする。
2. テストを契約ディレクトリに配置し、通常の Rust 子モジュールとして読み込む。
   同じ契約と準備処理を使うケースは `tests.rs` にまとめる。
   同じ契約を別の Cargo target やシナリオから検証する場合は、契約ディレクトリ内で名前のあるファイルに分ける。
   一つの契約だけで使う準備処理はテストと同じモジュールに置く。
   複数の契約で共有する準備処理は、主となる機能の `support.rs` に置き、契約のテストを子モジュールとして読み込む。
   複数の独立した準備処理がある場合は `support/<scenario>.rs` に分ける。
   共通の準備処理を別機能の契約でも使う場合は、その契約へのモジュールパスを明示する。
   support ファイルには `#![cfg(test)]` を置く。
   非公開要素を検証するテストは、対象の実装モジュールから `#[cfg(test)]` と `#[path]` で接続する。
   Feature・Contract 宣言の読み込みは、テスト専用の `trace_metadata` モジュールにまとめる。
   移動するテストに相対パスの `include_str!` などがあれば、同じファイルを指すように直す。
   package、Cargo target、完全なテスト名、ソース位置、主契約を一覧へ登録する必要はない。
   到達しない宣言やテストファイルは検査で拒否する。
3. 通常の `#[test]` と `#[tokio::test]` は、所属する契約の `unreviewed` として自動追加される。
   所属は、その実装を呼ぶことや 検証の十分さを保証しない。
   `unreviewed` しかない契約には `gap` が必要となる。
   レビューした根拠は、元のテストを `trace::case!` で包み、`Case` に `scope`、`reason`、`relationship` を宣言する。
   `asserts` は記載した事例を直接検証する関係、`exercises` は処理を通る関係を表す。
4. 一つのテストが複数の契約や機能に関わる場合は、`Case.also` の `Evidence` に追加する。
   追加先の `feature`、`contract`、その機能での `scope`、関係、理由を明示する。
   同じ機能における同じテストの `scope` は統一する。
   テスト本体は複製しない。`run` は共有されたテストを一度だけ実行する。
5. 製品の契約を検証しないテストは、個別に理由を宣言する。
   SQL 記述方針やテスト用 DB の検査には `trace::infrastructure!("category", "reason"; <test item>)` を使う。
   別プロセスから起動する補助テストには `trace::helper!("reason"; <test item>)` を使う。
   基盤・補助テストの分類と理由は、各テストの Rust 宣言で確認する。
   未分類のテストをまとめて除外する仕組みは設けない。
6. HTTP・gRPC 操作の所属は `Feature.surfaces`、その他の処理入口は `Feature.entrypoints` に宣言する。
   OpenAPI 出力と proto の操作一覧に照合するため、追加・削除や所有者の重複は検査で拒否する。
   別の JSON 台帳への登録は不要。
   Cargo example は名前に関係なく、ソース直下に一つの `example!(Example { kind, reason })` 宣言を置く。
   [OpenAPI 出力の例](../../crates/player-http/examples/player-openapi.rs)のように共通モデルを読み込み、`ExampleKind::OpenApi` または `ExampleKind::Program` と理由を指定する。
   全 example をコンパイルし、`OpenApi` の example だけを実行して出力を照合する。
   `Program` は実行しない。未宣言・重複・条件付きの宣言は拒否する。
7. リポジトリのルートで検査ツールのテストと、宣言・ソース参照・Cargo のテスト一覧の検査を実行する。

   ```sh
   python3 scripts/test-traceability.py
   python3 scripts/test-directory-traceability.py
   python3 scripts/test-traceability-viewer.py
   python3 scripts/traceability.py check
   ```

8. 実テストを実行する。

   ```sh
   scripts/test-postgres.sh python3 scripts/traceability.py run
   scripts/test-postgres.sh cargo test --locked --workspace
   ```

`check` は、宣言、ソース参照、操作の所属、Cargo のコンパイル済みテスト一覧を検査する。
`check` と `run` は表示用ファイルを読み書きしない。
テスト本体の成功を確認するコマンドは `run` または `cargo test`。

## ブラウザーで保証とソースを確認する

リポジトリのルートで HTML を生成する。

```sh
python3 scripts/traceability.py viewer
```

`target/traceability-viewer.html` をブラウザーで開く。
サーバーや追加のパッケージは不要で、`file://` でも利用できる。
出力先を変える場合は `python3 scripts/traceability.py viewer --output /tmp/traceability.html` を使う。
`--output` は `viewer` 専用となる。
`--root` を指定した場合、既定の出力先はそのルートの `target/traceability-viewer.html` となる。
入力ソース、追跡済みファイル、ツールのコード、Git の管理ファイルへの上書きは拒否する。

機能を検索し、保証・契約・テストや、それらを結ぶ関係を選択する。
詳細には処理の流れ、根拠、検証範囲、未検証事項、限界を表示する。
契約を選ぶと、その契約のテストをグラフに展開する。
選択項目は URL のハッシュに記録され、再読込とブラウザーの戻る・進むで復元する。
共有する場合は、同じ HTML とハッシュを渡す。
Tab と Enter で項目を選択できる。
ソース画面は **閉じる** または Escape で閉じる。

ソース画面は生成時点に取り込んだ内容と参照行を表示する。
取り込み時の HEAD とファイルの生バイトが一致し、対応する GitHub origin がある場合だけ、そのコミットに固定したリンクを表示する。
変更済み・新規のファイルや Git のない環境でも、取り込んだソースを参照できる。
ファイル群全体の同時点性は保証しない。
ソースを変更したら HTML を再生成する。
HTML には参照されたソースの全文を含むため、共有先を確認する。

`viewer` は構造・操作の所属・Cargo のテスト一覧を検査してから生成する。
テスト本体は実行せず、テストの成功や検証の十分さも示さない。
Rust CI の `traceability-viewer` artifact からも HTML をダウンロードして開ける。

## 検査の対象

分類の対象は、workspace の各 package にあるデフォルト構成の lib、bin、integration test target。
新しい package や target も列挙し、未分類のコンパイル済みテストを拒否する。
テストがない target も検査対象として記録する。
`test = false` の target は理由付きで記録し、`required-features` を持つ target は構成を推測せず拒否する。
基盤・補助テスト、除外 target、example の分類と理由は収集・検査する。
これらの分類をまとめた表示は viewer に含まれないため、理由は Rust 宣言と Cargo.toml で確認する。
ディレクトリ規約、ファイルの到達、同一テストの重複、宣言とコンパイル結果の不一致も検査する。

[収集ツール](../../tools/traceability/Cargo.toml) は独立した Rust workspace で、syn による構文解析を使う。
本体の依存関係には追加しない。
検査はホスト環境のデフォルト Cargo 構成で行い、別 target・feature・cfg の組合せを網羅するものではない。
例や doctest、Python・shell の検査は、この Cargo テスト分類の対象外とし、それぞれの CI 検査を継続する。
保証の意味、テストとの関連の妥当性、検証の十分さは引き続きレビューする。

Python 3.11 以上と Rust のビルド環境を使う。
実テストを実行する最後の手順には Docker Compose も必要となる。
既存の検証サービスを使う場合の設定は[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)を参照する。
