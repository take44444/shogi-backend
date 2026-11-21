# 将棋バックエンド

対局の gRPC API、公開棋譜とレートの HTTP API、Google ログイン、ブラウザー向けリアルタイムゲートウェイを提供する。

コンポーネント境界の呼出と、テストが対象にする API は[HTML viewer の手順](docs/how-to/check-traceability.md)に従って確認する。
採用理由と将来の設計案は[ドキュメント一覧](docs/README.md)を参照すること。

以下のコマンドは Rust、Cargo、Docker、Docker Compose がある環境で、リポジトリのルートから実行する。
rustup は [rust-toolchain.toml](rust-toolchain.toml) で指定した Rust と Cargo を使う。
ビルドには同梱の `protoc` を使う。

## ローカルで対局を実行する

データベースとロールを事前に作成する。
Compose の PostgreSQL は[初期設定 SQL](scripts/init-postgres.sql)でローカル用のデータベースとロールを作成する。
別の PostgreSQL を使う場合、書き込みロールにはデータベースの `CREATE` 権限と、対象スキーマ内でオブジェクトを作成する権限を付与する。
比較用スキーマを作るため、初回以降の起動でも `CREATE` が必要になる。
旧版のデータベースを保持し、対応していないスキーマには別の空のデータベースを用意すること。
互換性と初期化処理は[永続化コンポーネントの公開 API](crates/game-persistence/src/lib.rs)から確認できる。

```sh
docker compose up -d --wait postgres
cargo run --locked -p game-server
```

`game-server listening on 127.0.0.1:50051` が出たら、別のターミナルでサンプル対局を実行する。

```sh
cargo run --locked -p game-grpc --example play
```

最後の行は `black wins at sequence 6` となる。
接続先とユーザー名の接頭辞を変える場合は、次のように指定する。

```sh
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' cargo run --locked -p game-server -- 127.0.0.1:50052
cargo run --locked -p game-grpc --example play -- http://127.0.0.1:50052 my-game
```

対局 gRPC API は呼び出し元のユーザー ID を信頼するため、信頼できる内部ネットワークに配置する。
ブラウザーから使う場合は[ゲートウェイの起動手順](apps/realtime-gateway/README.md)に従うこと。

## Docker Compose で起動する

対局サーバー、レートワーカー、Outbox 配信プロセス、Valkey、PostgreSQL を起動する。

```sh
docker compose up --build -d
```

公開棋譜サービスを追加する場合は `records` プロファイルを使う。
問い合わせの例と読み取りロールの準備は [player-server の手順](apps/player-server/README.md)を参照すること。

```sh
docker compose --profile records up --build -d
```

Google ログインには [user-server の設定手順](apps/user-server/README.md)と `auth` プロファイルを使う。
ブラウザーからマッチメイキングと WebSocket を使う場合は[ゲートウェイの手順](apps/realtime-gateway/README.md)に従い、`realtime` プロファイルを使う。
マッチメイキングからレート更新までの操作例は[レートワーカーの手順](apps/rating-worker-server/README.md#実際の対局で確認する)を参照すること。

設定テンプレートは [.env.example](.env.example)、コンテナーの設定と公開ポートは [compose.yaml](compose.yaml)に置く。
データを保持してサービスを停止するには `docker compose down` を実行する。
本番環境には専用の認証情報、TLS、バックアップ、権限を制限したロールを用意する。
アプリケーションはデータベースとロールを作成しない。

## コンテナー再作成後のデータ保持を検証する

上記の手順で Compose を起動する。
未使用のユーザー名の接頭辞と検証用ディレクトリを指定すること。
以下は既定の gRPC ポートを使う。
起動と再起動の後は、`docker compose logs game-server` に `game-server listening on 0.0.0.0:50051` が出るまで待つ。

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

各段階の間で同じ `DATABASE_URL` を使ってサーバーを再起動すれば、ローカル実行でも検証できる。

## 確定済みの対局イベントを配信する

同じデータベースでローカルの配信プロセスを使う場合は、先に Compose の配信プロセスを停止する。
同じ Outbox を所有できる配信プロセスは 1 つだけである。

```sh
docker compose stop outbox-publisher
docker compose up -d --wait valkey
DATABASE_URL='postgres://shogi:shogi@localhost:5432/shogi' VALKEY_URL='redis://127.0.0.1:6379' cargo run --locked -p outbox-publisher
```

依存サービスの障害で配信プロセスが終了した場合は、依存サービスの復旧後に同じコマンドで再起動する。

配信の保証と実装は[アプリケーションの配信処理](crates/game-application/src/events.rs)、
通知形式は[メッセージのエンコーダー](crates/game-messaging/src/lib.rs)、採用理由は[対局イベントの配信と永続処理](docs/design/event-processing.md)を参照すること。

使い捨ての Compose プロジェクトで、停止と再起動を含む配信全体の検証を実行する。

```sh
./scripts/prove-outbox.sh
```

ポートを変える場合は `GAME_SERVER_PORT`、`VALKEY_PORT`、`POSTGRES_PORT` を設定する。
後処理は検証用プロジェクトのコンテナーとデータベースボリュームを削除する。

## API ドキュメントを生成する

サーバーを起動せずに、棋譜、ユーザー、マッチメイキング API の OpenAPI JSON を出力する。

```sh
bash scripts/generate-openapi.sh
```

出力先は `target/openapi/player.json`、`target/openapi/user.json`、`target/openapi/matchmaking.json` となる。
任意の第 1 引数で出力ディレクトリを変更できる。
起動中のユーザーサービスと棋譜サービスは `GET /openapi.json` でも仕様を返す。

Rust API ドキュメントは rustdoc で生成する。

```sh
cargo doc --locked --workspace --no-deps
cargo doc --locked -p game-application --no-deps --open
```

HTML は `target/doc/` に出力される。
非公開モジュールを確認する場合は `--document-private-items` を追加する。
gRPC のメッセージと操作は [proto/game.proto](proto/game.proto) を参照すること。
プロトコルコメントは、生成された `game_grpc::proto` の rustdoc にも含まれる。

## ワークスペースを検証する

[Rust CI](.github/workflows/rust.yaml) と同じ検査を実行する。

```sh
python3 scripts/check-boundaries.py
python3 scripts/test-check-boundaries.py
cargo fmt --all -- --check
cargo clippy --locked --workspace --all-targets -- -D warnings
./scripts/test-postgres.sh
python3 scripts/test-traceability.py
python3 scripts/test-traceability-viewer.py
python3 scripts/traceability.py check
python3 scripts/check-contract-interfaces.py
python3 scripts/test-deployment.py
```

`test-postgres.sh` は検証用の PostgreSQL と Valkey を起動し、テスト後に削除する。
既存サービスを使う場合は `TEST_DATABASE_URL` と `TEST_VALKEY_URL` を設定する。
未指定のサービスだけを起動する。
データベース接続ロールには `CREATEDB` と `CREATEROLE` が必要になる。
ホスト側ポートは `POSTGRES_PORT` と `VALKEY_PORT` で変更できる。

パッケージ単位のテストと Rust ドキュメントの検証は次のように実行する。
解析用 Rust の準備とテスト対象属性の書き方は[トレーサビリティの手順](docs/how-to/check-traceability.md)を参照すること。

```sh
cargo test --locked -p shogi-domain
./scripts/test-postgres.sh cargo test --locked -p game-persistence
RUSTDOCFLAGS='-D warnings' cargo doc --locked --workspace --no-deps
cargo test --locked --workspace --doc
```
