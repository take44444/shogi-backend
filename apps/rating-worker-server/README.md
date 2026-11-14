# レートワーカーを起動する

`docker compose up --build -d` で PostgreSQL、対局サーバー、独立したレートワーカーを起動する。
認証不要の棋譜 HTTP サーバーも起動する場合は `--profile records` を追加する。

ローカルのバイナリーを使う場合は、`game-server` と同じ対局データベースを `DATABASE_URL` に指定する。
`cargo run --locked -p rating-worker-server` で起動し、Ctrl-C で停止する。

ポーリング、ジョブの取得、再試行の設定を変更する場合は、
[レートワーカーの設定](../../docs/reference/service-configuration.md#レートワーカー)にある環境変数を指定する。

## 隔離された対局の処理を再試行する

書き込み権限を持つ認証情報を使い、`rating_jobs.last_error`、
`rated_games` に保存した変更不可の開始時レート状態、対応する確定済みの対局結果を確認する。
保存済みの入力が破損している場合は、再試行の前に修復すること。再試行は入力を自動修復しない。

対局 ID を明示して運用コマンドを実行する。

```sh
cargo run --locked -p rating-worker-server -- retry-quarantined GAME_ID
```

Compose では `docker compose run --rm rating-worker-server retry-quarantined GAME_ID` を実行する。
`Retried` は、試行回数の上限と世代を更新してジョブを待機状態に戻したことを示す。
`NotQuarantined` と `NotFound` は保存状態を変更しない。
隔離中も再試行中も、レート更新用の予約は占有されたままとなる。
レート適用前に同じプールで次の対局を開始しないよう、マッチメイキングが予約を参照する。
復旧のために予約を削除しないこと。

## 実際の対局で確認する

`game-server`、`rating-worker-server`、`player-server` を同じデータベースに接続して起動する。
実行ごとに新しい接頭辞を使う。

```sh
cargo run --locked -p game-grpc --example rating_proof -- http://127.0.0.1:50051 rating-demo
curl http://127.0.0.1:8081/users/rating-demo-one/ratings
```

サンプルはチケットを 2 件登録して対戦相手の決定を待ち、確定した対局と無作為に決めた先後を出力してから、後手が投了する。
ワーカーの適用後、`ten_minute_30` のレートは先手のユーザーが 1516、後手のユーザーが 1484 となる。
両ユーザーのレート適用済み対局数は 1 となる。もう一方のプールはレートが null、対局数が 0 のままとなる。
