# リアルタイムゲートウェイを起動する

コマンドはリポジトリのルートで実行する。
マッチメイキングと対局 WebSocket は[ゲートウェイ API の参照仕様](../../apps/realtime-gateway/README.md)に記述する。
環境変数と上限は[設定の参照仕様](../reference/service-configuration.md#リアルタイムゲートウェイ)に記述する。

## Docker Compose で起動する

次の順で認証情報を設定し、サービスを起動する。

1. リポジトリのルートにある `.env` に `GOOGLE_CLIENT_ID` と `GOOGLE_CLIENT_SECRET` を設定する。
   [Google クライアントの設定手順](../../apps/user-server/README.md#google-ログインを設定する)に従うこと。
   リダイレクト URI には `http://localhost:8082/auth/google/callback` を使う。
2. `realtime` プロファイルでサービスを起動する。

   ```sh
   docker compose --profile realtime up --build -d
   ```

3. `http://localhost:8082/auth/google/start` を開いてログインする。
   成功すると `/me` に移動し、プロフィールを JSON で返す。
4. `/matchmaking/*` と `/ws/games/*` に同じブラウザーオリジンから接続する。

ブラウザーの URL を変更する場合は、`REALTIME_PUBLIC_ORIGIN` と `REALTIME_PORT` を一緒に設定する。
Google のリダイレクト URI も合わせて変更すること。
この手順では `realtime` プロファイルだけを使う。
`auth` プロファイルは、ユーザーサービスに別のオリジンを設定する。

## ローカルのバイナリーを起動する

次の順で依存サービス、ゲートウェイ、プロキシを起動する。

1. `user-server`、`game-server`、Valkey、Outbox 配信プロセスを起動する。
   `user-server` には `USER_PUBLIC_ORIGIN=http://localhost:8082` と
   `USER_ALLOW_LOOPBACK_HTTP=true` を設定する。
2. ゲートウェイを起動する。

   ```sh
   GATEWAY_BIND_ADDR=127.0.0.1:8083 \
   GATEWAY_PUBLIC_ORIGIN=http://localhost:8082 \
   GATEWAY_ALLOW_LOOPBACK_HTTP=true \
   USER_SERVICE_URL=http://127.0.0.1:8080 \
   GAME_SERVICE_URL=http://127.0.0.1:50051 \
   VALKEY_URL=redis://127.0.0.1:6379 \
   cargo run --locked -p realtime-gateway-server
   ```

   プロセスは `realtime-gateway-server listening on 127.0.0.1:8083` と出力する。

3. リバースプロキシを `localhost:8082` で待ち受けさせる。
   `/matchmaking/*` と `/ws/games/*` を `127.0.0.1:8083` に転送する。
   その他のパスは `127.0.0.1:8080` に転送する。
   [deploy/realtime.Caddyfile](../../deploy/realtime.Caddyfile) を参考に、
   コンテナーのアドレスをこれらのローカルアドレスに置き換える。

## HTTPS の背後に配置する

認証、マッチメイキング、WebSocket を同じオリジンで提供するリバースプロキシで HTTPS を終端する。
`user-server`、`game-server`、ゲートウェイ、Valkey は内部ネットワークに置く。
ゲートウェイとユーザーサービスの公開オリジンを HTTPS に設定し、両方のループバック HTTP フラグを無効にする。
