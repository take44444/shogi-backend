# player-server の起動手順

公開棋譜と確定済みレートを取得する HTTP API を起動する。
初期化済みの対局データベースと、SELECT 専用の認証情報が必要になる。

API の仕様を出力するには、[API ドキュメントの生成手順](../../README.md#api-ドキュメントを生成する)に従う。
仕様、実装、テストの対応は[機能トレーサビリティ](../../docs/reference/feature-traceability.md)から確認する。

## ローカルで起動する

先に `game-server` を起動し、対局スキーマを初期化する。
その後、リポジトリのルートで読み取りサービスを起動する。

```sh
cargo run --locked -p player-server
```

サーバーは `player-server listening on 127.0.0.1:8081` と出力する。
別のターミナルで終了済み対局を取得する。
期間を対象対局を含む UTC マイクロ秒の範囲に置き換え、対局 ID を指定する。

```sh
curl 'http://127.0.0.1:8081/game-records?from=1793577600000000&to=1793664000000000&limit=30'
curl 'http://127.0.0.1:8081/game-records/COMPLETED_GAME_ID'
```

## 読み取りサービスを設定する

ローカルの既定値を変更する場合は `PLAYER_BIND_ADDR` または `PLAYER_GAME_DATABASE_URL` を設定する。
設定例は [.env.example](../../.env.example)、既定値は[起動処理](src/main.rs)で確認する。

対局データベースへの接続と `public` スキーマの使用を許可した、読み取り専用ロールを用意する。
SELECT の対象は[スキーマ初期化処理](../../crates/game-persistence/src/schema.rs)で確認し、テーブルの所有権や書き込み権限は持たせない。
ローカル環境では[初期設定 SQL](../../scripts/init-postgres.sql)で `shogi_reader` を作成し、
`game-server` の起動時に対象テーブルへの SELECT 権限を付与する。

## Docker Compose で起動する

`records` プロファイルを指定してサービスを起動する。

```sh
docker compose --profile records up --build -d
```

ホスト側ポートを変更する場合は `PLAYER_SERVER_PORT` を設定する。
接続先とポート公開の設定は [compose.yaml](../../compose.yaml) で確認する。

## 動作を検証する

リポジトリのルートで、読み取りサービスと HTTP アダプターのテストを実行する。

```sh
./scripts/test-postgres.sh cargo test --locked -p game-record-persistence -p player-http -p player-server
```

## 確定済みレートを取得する

公開レート取得エンドポイントにユーザー ID を指定する。

```sh
curl 'http://127.0.0.1:8081/users/USER_ID/ratings'
```

レート更新まで確認するには、[実際の対局で確認する手順](../rating-worker-server/README.md#実際の対局で確認する)に従う。
