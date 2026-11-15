# 機能の保証とテストの対応を更新する

[機能一覧](../reference/feature-traceability.md) から、各機能の保証、コンポーネント契約、実在するテストと未検証事項を確認できる。
現行の仕様は実装とテストを正とし、参照資料はソースに隣接する `*.trace.json` から生成する。

1. `crates/` または `apps/` の実装に隣接する `*.trace.json` を編集する。
   `version` は `1` とし、`feature`、`guarantees`、`contracts`、`tests`、`limitations` を持たせる。
   `feature.id` は全機能で一意にする。その他の ID はファイル内で一意にする。
   `feature.flow` に、入口から各コンポーネントを経由して結果が成立する順序と条件を書く。
   一つの要素は空でない一行の文章にする。
2. 保証から必要な契約、契約から根拠となるテストを ID で参照する。
   `asserts` には直接確認する事例、`exercises` には通る処理を `reason` に書く。
   未検証の部分は契約の `gap` に残す。テストがない契約には明示した `gap` が必要となる。
   すべての契約に明示した `gap` があれば、`tests` を空にできる。
3. 実装とテストの `source` にリポジトリ相対パスと、一意に見つかる `needle` を指定する。
   テストには Cargo の `package`、`target`、完全なテスト名と `scope` を指定する。
   `target` は `{"kind":"lib"}` または `{"kind":"test","name":"対象名"}` とする。
   テストの `needle` には、そのテストの関数名を含める。
4. [操作と入口の台帳](../../scripts/traceability-surfaces.json) を更新する。
   `crates/*/examples/*openapi.rs` と `apps/*/examples/*openapi.rs` の exporter はすべて登録する。
   その OpenAPI 操作と、指定した gRPC service の全 rpc 名を既存の `feature.id` に割り当てる。
   WebSocket の入力、周期処理、ワーカー、スキーマ検査と起動処理も `entrypoints` の名前とソース位置で割り当てる。
   どの機能も一つ以上の操作または入口を所有する必要がある。
5. リポジトリのルートで参照資料を再生成する。

   ```sh
   python3 scripts/traceability.py render
   ```

6. 生成差分で保証の条件、関連の理由、テストの範囲、未検証事項を確認する。
   出力は `docs/reference/traceability/<feature.id>.md` と機能一覧になる。
   `render` は検証に成功した後で文書を書き、不要になった機能文書と旧レート取得文書を削除する。
7. 構造、実在するテスト名、生成文書の内容とファイル集合、検査スクリプトの制御例を確認する。

   ```sh
   python3 scripts/traceability.py check
   python3 scripts/test-traceability.py
   ```

8. 対応する全テストを実行する場合は、Docker Compose が使える環境で次を実行する。

   ```sh
   scripts/test-postgres.sh python3 scripts/traceability.py run
   ```

Python 3.11 以上と Rust のビルド環境が必要となる。
`render` と `check` は既存の OpenAPI example を実行し、Cargo でテスト一覧を取得する。DB 接続は使わない。
`run` は文書の一致も検査し、共有される package・target・完全なテスト名を一度だけ実行する。
成功扱いにするのは、選択した 1 件の成功、失敗 0 件、無視 0 件を確認した場合だけとなる。
`check` はテストを実行しない。CI では既存の workspace テストの後に構造と文書の一致を検査する。

API 操作の追加・削除、台帳が参照する機能の欠落、孤立した機能、空の必須関連、不明な参照先、孤立した契約やテストは検査に失敗する。
バックグラウンド処理や WebSocket メッセージの新設、機能の分割、孤立を残さない関連の削除、根拠の過大評価はレビューで確認する。
位置とテスト名の実在やテストの成功は、対応の意味や網羅性を証明しない。
