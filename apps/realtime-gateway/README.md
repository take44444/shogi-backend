# リアルタイムゲートウェイを起動する

コマンドはリポジトリのルートで実行する。
マッチメイキングの HTTP 仕様は[OpenAPI の生成手順](../../README.md#api-ドキュメントを生成する)、
WebSocket の形式は[プロトコル実装](../../crates/realtime-gateway/src/game.rs)を参照すること。
認証、接続、資源制限の実装とテストは[機能の対応表](../../docs/reference/traceability/realtime-session-commands-limits.md)からたどる。
環境変数と上限は [GatewayConfig](../../crates/realtime-gateway/src/lib.rs) に定義する。

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
セッション Cookie を共有するには、ユーザーサービスとゲートウェイのブラウザー向けホストを一致させる。
ブラウザーの URL では `localhost` と `127.0.0.1` を混在させない。
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

## マッチメイキングの待機を継続する

参加要求に使ったチケット UUID を、結果が確定するまでクライアントに保持する。
503 や応答の消失後は、同じ UUID と内容で再試行するか、そのチケットの状態を取得する。
新しい参加要求を始める場合だけ、新しい UUID を発行する。

待機中は 10 秒を目安に、前回より大きいシーケンス番号でハートビートを送る。
同じハートビートを再試行する場合は、番号を変えずに再送する。
リクエストの形式は OpenAPI、間隔の検証課題は[マッチメイキングの設計メモ](../../docs/design/matchmaking.md#初期値と負荷検証)を参照する。

## ブラウザーから対局に接続する

ログイン後、同じオリジンのブラウザーコンソールで実行する。
マッチメイキングで取得した対局 ID に置き換える。

```js
const gameId = 'MATCHED_GAME_ID';
const scheme = location.protocol === 'https:' ? 'wss:' : 'ws:';
const socket = new WebSocket(`${scheme}//${location.host}/ws/games/${encodeURIComponent(gameId)}`, 'shogi.realtime.v1');
socket.onmessage = event => console.log(JSON.parse(event.data));
```

最初の `snapshot` を受け取り、自分が先手の初期局面なら着手を送る。

```js
const pendingCommand = {type: 'move', request_id: crypto.randomUUID(), expected_seq: '0', usi: '7g7f'};
socket.send(JSON.stringify(pendingCommand));
```

操作結果と更新通知の扱いは[対局操作の対応表](../../docs/reference/traceability/realtime-session-commands-limits.md)、
初期局面と通知の同期は[リアルタイム配信の対応表](../../docs/reference/traceability/realtime-game-feed.md)を参照すること。

## 操作結果が不明な場合や切断後に復旧する

操作は 1 件ずつ送り、送信前にリクエスト全体をクライアントに保存する。上の例では `pendingCommand` を保持する。
受理応答が消失した場合や `outcome_unknown` を受け取った場合は、元の値と同じ内容で再送する。
結果を回復するために新しい `request_id` を発行しない。

`stale_seq` または `resync_required` を受け取った場合は再接続し、新しい `snapshot` から局面を更新する。
切断後の再接続には指数バックオフと無作為な待機時間のばらつきを使う。
再接続で取得した局面の番号と、応答不明の操作を再試行するときの元の `expected_seq` は区別して保持する。

## HTTPS の背後に配置する

認証、マッチメイキング、WebSocket を同じオリジンで提供するリバースプロキシで HTTPS を終端する。
`user-server`、`game-server`、ゲートウェイ、Valkey は内部ネットワークに置く。
ゲートウェイとユーザーサービスの公開オリジンを HTTPS に設定し、両方のループバック HTTP フラグを無効にする。
