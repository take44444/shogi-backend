# レートワーカーを起動する

`docker compose up --build -d` で PostgreSQL、対局サーバー、独立したレートワーカーを起動する。
認証不要の棋譜 HTTP サーバーも起動する場合は `--profile records` を追加する。

ローカルのバイナリーを使う場合は、`game-server` と同じ対局データベースを `DATABASE_URL` に指定する。
`cargo run --locked -p rating-worker-server` で起動し、Ctrl-C で停止する。

ポーリング、ジョブの取得、再試行の設定は[起動処理](src/main.rs)で確認する。
ワーカーを同時に複数起動する場合は、異なる `RATING_WORKER_ID` を指定する。
仕様、実装、テストの対応は[レート更新のトレーサビリティ](../../docs/reference/traceability/rating-application.md)で確認する。

## 隔離された対局の処理を再試行する

書き込み権限を持つ認証情報を使い、`rating_jobs.last_error`、
`rated_games` に保存した変更不可の開始時レート状態、対応する確定済みの対局結果を確認する。
保存済みの入力が破損している場合は、再試行の前に修復すること。再試行は入力を自動修復しない。

対局 ID を明示して運用コマンドを実行する。

```sh
cargo run --locked -p rating-worker-server -- retry-quarantined GAME_ID
```

Compose では `docker compose run --rm rating-worker-server retry-quarantined GAME_ID` を実行する。
`Retried` が出力されたことを確認し、ワーカーの適用結果を確認する。
復旧のために `rating_slots` の予約を削除しないこと。

## 実際の対局で確認する

`game-server`、`rating-worker-server`、`player-server` を同じデータベースに接続して起動する。
実行ごとに新しい接頭辞を使う。

```sh
cargo run --locked -p game-grpc --example rating_proof -- http://127.0.0.1:50051 rating-demo
curl http://127.0.0.1:8081/users/rating-demo-one/ratings
curl http://127.0.0.1:8081/users/rating-demo-two/ratings
```

[サンプル](../../crates/game-grpc/examples/rating_proof.rs)が出力した先後と結果を確認する。
ワーカーの適用後に両ユーザーのレートを再取得し、勝者のレート上昇と敗者の低下を確認する。
`ten_minute_30` の対局数が両者とも 1 であり、もう一方のプールが未対局のままであることも確認する。
