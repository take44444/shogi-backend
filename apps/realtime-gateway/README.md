# リアルタイムゲートウェイを起動する

コマンドはリポジトリのルートで実行する。
マッチメイキングの HTTP 仕様は[OpenAPI の生成手順](../../README.md#api-ドキュメントを生成する)、
WebSocket の形式は[プロトコル実装](../../crates/realtime-gateway/src/game.rs)を参照すること。
認証、接続、資源制限の実装とテストは[HTML viewer の手順](../../docs/how-to/check-traceability.md#ブラウザーで保証とソースを確認する)に従って確認する。
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
参加前に UUID とプールを保存する。
503 や応答の消失後は、同じ UUID とプールで参加要求を再送する。
新しい参加要求を始める場合だけ、新しい UUID を発行する。

待機中は 5 秒を目安に、前回より大きいシーケンス番号でハートビートを送る。
`POST /matchmaking/tickets/{ticket_id}/heartbeat` に `{"sequence":1}` の形式で送る。
送信前にシーケンス番号を保存し、応答が不明なら番号を変えずに再送する。
応答を受け取ってから、次のハートビート用の番号を増やす。
応答のチケットの `state` で、待機・マッチ成立・対局開始・終了を判定する。
終端状態になったらハートビートを止める。
リクエストの形式は OpenAPI、間隔の検証課題は[マッチメイキングの設計メモ](../../docs/design/matchmaking.md#初期値と負荷検証)を参照する。

画面を開き直したときは、`GET /matchmaking/me` で現在の参加を取得する。
待機中なら、返された `heartbeat_sequence` の次の番号から継続する。
ただし、応答が不明な保存済み操作があれば、先にその操作を同じ内容で再送する。
現在参加のチケット UUID が保存済みの操作と一致するか確認する。
`idle` は現在の参加がないことだけを示し、以前の操作の結果は示さない。
ハートビートの 404 も、応答不明の参加要求が今後コミットされない保証にはならない。
参加・取消・準備完了の応答が不明な場合は、それぞれ元の操作を同じ UUID と内容で再試行する。

## マッチ成立後に準備完了を送る

ハートビートの応答、または再接続時の現在参加で `matched` を確認する。
対局に入る準備ができたら、自分のチケット UUID で次を実行する。

```js
const ticketId = 'YOUR_TICKET_UUID';
const response = await fetch(`/matchmaking/tickets/${encodeURIComponent(ticketId)}/ready`, {method: 'POST'});
console.log(await response.json());
```

応答を失った場合も同じ UUID で再送する。
`ready` は自分の準備確認、`ready_deadline` は両者共通のデータベース時刻による期限である。
期限は成立から 15 秒で、再送やハートビートでは延長しない。
`matched` の `game_id` は予約された ID であり、まだ対局 API では使えない。
準備完了の応答を確認し、まだ `matched` ならハートビートの応答で `playing` になるまで待つ。
期限切れは `voided` になり、勝敗やレーティングに影響しない。新しい UUID で再参加する。

## ブラウザーから対局に接続する

ログイン後、同じオリジンのブラウザーコンソールで実行する。
`playing` のチケットで取得した対局 ID に置き換える。

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

操作結果と更新通知の扱いは[対局操作の定義](../../crates/realtime-gateway/src/trace/realtime-session-commands-limits/feature.rs)、
初期局面と通知の同期は[リアルタイム配信の定義](../../crates/realtime-gateway/src/trace/realtime-game-feed/feature.rs)を参照すること。

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
