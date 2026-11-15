# 機能の保証とテストの対応を更新する

[機能一覧](../reference/feature-traceability.md)と各機能のページは自動生成する。
手書きの文書に仕様やテスト一覧を複写せず、元の `*.trace.json` を更新する。
形式の例は[レート取得の定義](../../crates/player-http/src/ratings.trace.json)を参照する。

1. 変更した実装の近くにある `*.trace.json` を編集し、保証と契約の根拠を実装・テストに結び付ける。
   source は実装コードの位置に合わせ、根拠のない箇所は `gap` に記録する。
2. API や処理入口を追加・変更した場合は[操作と入口の台帳](../../scripts/traceability-surfaces.json)も更新する。
3. リポジトリのルートで再生成する。

   ```sh
   python3 scripts/traceability.py render
   ```

4. 生成差分を読み、各テストの assertion が保証のどの事例を確認するか、未検証事項を隠していないかをレビューする。
5. 検査ツールのテストと、参照・生成物の検査を実行する。

   ```sh
   python3 scripts/test-traceability.py
   python3 scripts/traceability.py check
   ```

6. 登録した実テストを実行する。

   ```sh
   scripts/test-postgres.sh python3 scripts/traceability.py run
   ```

Python 3.11 以上と Rust のビルド環境を使う。最後の手順には Docker Compose も必要となる。
既存の検証サービスを使う場合の設定は[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)を参照する。
