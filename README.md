# 将棋バックエンド

対局操作を永続化する gRPC API、公開棋譜の HTTP API、Google ログインを提供するバックエンド。
ユーザー向け HTTP サービスは対局サービスから分離している。
PostgreSQL は受理済みの操作、イベント、マッチメイキングのチケット、レート更新ジョブを保存する。
独立したワーカーが Valkey への一時通知とレート更新を実行する。

リアルタイムゲートウェイは、認証済みブラウザー向けに着手、投了、対局イベント受信、観戦用の WebSocket を提供する。
ブラウザーの接続設定は[ゲートウェイの起動手順](docs/how-to/run-realtime-gateway.md)、
リクエストとメッセージは[ブラウザー API の参照仕様](apps/realtime-gateway/README.md)に記述する。

対局 gRPC API は、呼び出し元が渡すユーザー ID を信頼する。信頼できる内部ネットワークに配置すること。
ユーザーサービスは対局 gRPC リクエストを認証しない。

以下のコマンドには Rust、Cargo、Docker、Docker Compose が必要になる。
リポジトリのルートで実行すること。ビルドには同梱の `protoc` を使う。

## ローカルで対局を実行する

このリリースはスキーマバージョン 7 に対応する。
古いデータベースは保持し、別の空のデータベースを用意すること。
起動処理は古いスキーマを拒否し、データの移行や削除は行わない。

サーバーは接続を受け付ける前に、スキーマバージョン 7 を初期化または検証する。
データベースとロールは事前に作成すること。
スキーマ検証には、初回以降の起動でもデータベースの `CREATE` 権限が必要になる。
必要な権限と非互換スキーマの扱いは[保存層の参照仕様](crates/game-persistence/README.md#起動時のスキーマ処理)に記述する。

PostgreSQL を起動し、続けて対局サーバーを起動する。

```sh
docker compose up -d --wait postgres
cargo run --locked -p game-server
```

サーバーは `game-server listening on 127.0.0.1:50051` と出力する。
別のターミナルでサンプルクライアントを実行する。

```sh
cargo run --locked -p game-grpc --example play
```

クライアントはチケットを 2 件登録し、対戦相手の決定を待つ。
その後、`7g7f 3c3d 8h2b+ 3a2b B*4e` の順に指し、後手が投了する。
駒取り、成り、駒打ち、スナップショット、同一内容での再試行を検証する。
最後の行は `black wins at sequence 6` となる。
サーバーは対局 ID を生成し、先後を決定する。任意の引数で新しいユーザー名の接頭辞を指定できる。

既定の接続先は `127.0.0.1:50051`、データベース接続設定は
`DATABASE_URL=postgres://shogi:shogi@localhost:5432/shogi` となる。
アドレスを位置引数で指定すると `GAME_SERVER_ADDR` より優先される。

```sh
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' cargo run --locked -p game-server -- 127.0.0.1:50052
cargo run --locked -p game-grpc --example play -- http://127.0.0.1:50052 my-game
```

## 終了済みの公開棋譜を取得する

対局サーバーがデータベースを初期化した後、棋譜サービスを起動する。

```sh
cargo run --locked -p player-server
```

別のターミナルから問い合わせる。
期間は対象対局を含む UTC マイクロ秒の範囲に置き換え、終了済み対局の ID を指定する。

```sh
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&limit=30'
curl 'http://127.0.0.1:8081/game-records/COMMITTED_GAME_UUID'
```

棋譜サービスは SELECT 専用の認証情報を使い、ユーザーサービスを必要としない。
起動手順は [player-server](apps/player-server/README.md)、
検索パラメーター、レスポンス、制限は [player-http](crates/player-http/README.md) に記述する。
Google ログインとプロフィール操作は[ユーザーサービスの手順](apps/user-server/README.md)を参照すること。

## Docker Compose で起動する

対局サーバー、レートワーカー、Outbox 配信プロセス、Valkey、PostgreSQL を起動する。

```sh
docker compose up --build -d
```

PostgreSQL の初期設定は `shogi` と `users` データベース、`shogi_reader` ロールを作成する。
両データベースは名前付きボリューム `pg-data` を使う。
公開ポートはホストのループバックアドレスにバインドする。
ホスト側ポートを変更する場合は `GAME_SERVER_PORT`、`POSTGRES_PORT`、`VALKEY_PORT` を設定する。

公開棋譜サービスを追加する場合は `records` プロファイルを使う。

```sh
docker compose --profile records up --build -d
```

ユーザーサービスを起動する場合は `auth` プロファイルを使う。
[ユーザーサービスの手順](apps/user-server/README.md#docker-compose-で起動する)に従って Google の認証情報を設定する。
ブラウザーからマッチメイキングと WebSocket を使う場合は、
[ゲートウェイの起動手順](docs/how-to/run-realtime-gateway.md)に従うこと。
この手順では `realtime` プロファイルと `http://localhost:8082` を使う。

データベースのデータを保持してサービスを停止するには `docker compose down` を実行する。

## コンテナー再作成後のデータ保持を検証する

上記の手順で Compose を起動する。未使用のユーザー名の接頭辞と検証用ディレクトリを指定すること。
以下のコマンドは既定の gRPC ポートを使う。
起動または再起動のたびに、`docker compose logs game-server` に
`game-server listening on 0.0.0.0:50051` が出るまで待ってからクライアントを実行する。

1. 対局を開始し、受理記録を保存する。

   ```sh
   cargo run --locked -p game-grpc --example persistence_proof -- seed http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
   ```

2. 対局サーバーのコンテナーを再作成する。

   ```sh
   docker compose up -d --force-recreate game-server
   ```

3. サーバーの起動完了後に対局を再開する。

   ```sh
   cargo run --locked -p game-grpc --example persistence_proof -- resume http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
   ```

4. 対局サーバーを再起動する。

   ```sh
   docker compose restart game-server
   ```

5. サーバーの起動完了後に終了済み対局を検証する。

   ```sh
   cargo run --locked -p game-grpc --example persistence_proof -- finished http://127.0.0.1:50051 restart-proof /tmp/shogi-proof
   ```

クライアントは protobuf のスナップショットと受理記録を保存し、再起動後に比較する。
対局を継続し、終局後の再起動でも元の受理記録が取得できることを確認する。
各段階の間で同じ `DATABASE_URL` を使ってサーバーを再起動すれば、ローカル実行でも検証できる。

## 確定済みの対局イベントを配信する

対局サーバーは、受理した操作とイベントを原子的にコミットする。
配信プロセスやブローカーが停止していても操作を受理する。
同じデータベースに対してローカルで配信プロセスを起動する場合は、先に Compose 側の配信プロセスを停止すること。
同じ Outbox を同時に所有できる配信プロセスは 1 つだけである。その後、Valkey を起動する。

```sh
docker compose up -d --wait valkey
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' VALKEY_URL='redis://127.0.0.1:6379' cargo run --locked -p outbox-publisher
```

チャネル、JSON フィールド、配信保証は[通知の参照仕様](docs/reference/game-notifications.md)に記述する。
配信プロセスの復旧と通知の重複は[Outbox の所有権](crates/game-persistence/README.md#outbox-の所有権)に記述する。

使い捨ての Compose プロジェクトで、配信全体の検証を実行する。

```sh
./scripts/prove-outbox.sh
```

この検証は通知、同一内容での再試行、配信プロセスの停止、サーバー再作成、Valkey の障害と復旧を確認する。
購読者がいない場合の通知受け入れも確認する。
既定のホスト側ポートは 55051、56379、55435 となる。
変更する場合は `GAME_SERVER_PORT`、`VALKEY_PORT`、`POSTGRES_PORT` を設定する。
後処理は、検証用プロジェクトのコンテナーとデータベースボリュームを削除する。

## API ドキュメントを生成する

棋譜、ユーザー、マッチメイキング API の OpenAPI 3.1 JSON を出力する。
サーバーの起動、データベース設定、Google の認証情報は必要ない。

```sh
bash scripts/generate-openapi.sh
```

出力先は `target/openapi/player.json`、`target/openapi/user.json`、`target/openapi/matchmaking.json` となる。
任意の第 1 引数で出力ディレクトリを変更できる。
起動中の HTTP サービスは `GET /openapi.json` でも仕様を返す。

Rust API ドキュメントは rustdoc で生成する。

```sh
cargo doc --locked --workspace --no-deps
cargo doc --locked -p game-application --no-deps --open
```

HTML は `target/doc/` 以下に出力される。
非公開モジュールを確認する場合は `--document-private-items` を追加する。
`proto/game.proto` のプロトコルコメントは、生成された `game_grpc::proto` の参照仕様に含まれる。

## ワークスペースを検証する

[Rust CI](.github/workflows/rust.yaml) と同じ検査を実行する。

```sh
python3 scripts/check-boundaries.py
python3 scripts/test-check-boundaries.py
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
./scripts/test-postgres.sh
python3 scripts/check-contract-interfaces.py
python3 scripts/test-deployment.py
```

依存境界の検査は、ワークスペース内の依存経路と、直接指定した外部依存を調べる。
そのテストは、禁止された設定が失敗することを確認する。
インターフェースの検査は `tests/contracts/` のサンプルをコンパイルする。
デプロイ構成の検査は使い捨ての PostgreSQL を起動し、実際の権限を確認する。

`test-postgres.sh` は検証専用の PostgreSQL と Valkey を Compose で起動し、テスト後に削除する。
既定のホスト側ポートは 55433 と 56333 となる。
変更する場合は `POSTGRES_PORT` と `VALKEY_PORT` を設定する。
`TEST_DATABASE_URL` または `TEST_VALKEY_URL` が設定されていれば、指定されたサービスを使い、未指定のサービスだけを起動する。
データベース接続ロールには `CREATEDB` と `CREATEROLE` が必要になる。
各データベーステストは専用データベースを作成し、終了後に削除する。

パッケージ単位でテストする場合は `-p` を指定する。
PostgreSQL または Valkey を使うパッケージは検証用スクリプトで実行する。
Rust ドキュメントを検証する場合は、rustdoc の警告をエラーとして扱い、ドキュメントテストも実行する。

```sh
cargo test --locked -p shogi-domain
./scripts/test-postgres.sh cargo test --locked -p game-persistence
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
cargo test --locked --workspace --doc
```

Compose の認証情報はローカル開発用の既定値である。
本番環境では PostgreSQL の認証情報、TLS 設定、バックアップ、権限を制限したロールを用意する。
[読み取り用の権限](docs/reference/service-configuration.md#棋譜サービス)に従ってデータベースとロールを作成すること。
アプリケーションはデータベースもロールも作成しない。

[ドキュメント一覧](docs/README.md)には採用済みの判断、未完成の機能、将来の設計案をまとめる。
クレートの参照仕様には、[対局ルール](crates/shogi-domain/README.md)、
[アプリケーションの契約](crates/game-application/README.md)、
[保存層](crates/game-persistence/README.md)、[gRPC API](crates/game-grpc/README.md)を記述する。

## マッチメイキングとレート更新を確認する

`game-server`、`rating-worker-server`、`player-server` を同じデータベースに接続して起動する。
[レートワーカーの手順](apps/rating-worker-server/README.md#実際の対局で確認する)に従い、
ユーザーを組み合わせ、レート対象の対局を終了して、確定済みレートを取得する。

認証済みブラウザーから参加する場合は、[マッチメイキング HTTP API](apps/realtime-gateway/README.md#マッチメイキング-http-api-の契約)に従うこと。
新しい参加要求には新しいチケット UUID を使う。
待機中は 10 秒ごとに、前回より大きいシーケンス番号のハートビートを送る。
新しいシーケンス番号を受信しないまま 30 秒経過すると、待機は期限切れになる。
どちらのプールも時計や時間切れの判定は実装していない。
