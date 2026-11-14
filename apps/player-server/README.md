# player-server の起動手順

終了済みの公開棋譜を取得する、認証不要の HTTP API を起動する。
サービスは `player-http` と読み取り専用の `PostgresGameRecords` アダプターを使う。
初期化済みの対局データベースと、SELECT 専用の認証情報が必要になる。

フィルター、レスポンス、ページ分割、エラーは [HTTP API の参照仕様](../../crates/player-http/README.md)に記述する。

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
初期化済みの対局データベースに SELECT 専用でアクセスできるロールを使うこと。
既定値と必要な権限は[設定と権限の参照仕様](../../docs/reference/service-configuration.md#棋譜サービス)に記述する。

## Docker Compose で起動する

`records` プロファイルを指定してサービスを起動する。

```sh
docker compose --profile records up --build -d
```

Compose は、書き込みサービスとは別の認証情報で同じ対局データベースに接続する。
HTTP はホストのループバックアドレスで公開し、既定のポートは 8081 となる。
ホスト側ポートを変更する場合は `PLAYER_SERVER_PORT` を設定する。
書き込みサービスがスキーマを初期化している間は、再起動ポリシーに従って起動を再試行する。

## 動作を検証する

リポジトリのルートで、読み取りサービスと HTTP アダプターのテストを実行する。

```sh
./scripts/test-postgres.sh cargo test --locked -p game-record-persistence -p player-http -p player-server
```

サーバーを起動せずに OpenAPI を出力する場合は、[API ドキュメントの生成手順](../../README.md#api-ドキュメントを生成する)に従うこと。

## 確定済みレートを取得する

公開レート取得エンドポイントにユーザー ID を指定する。

```sh
curl 'http://127.0.0.1:8081/users/USER_ID/ratings'
```

レスポンスは両プールを含む。
フィールドと、レート適用済み対局がないユーザーの扱いは
[HTTP レート API の契約](../../crates/player-http/README.md#ユーザー別レートの取得)に記述する。
