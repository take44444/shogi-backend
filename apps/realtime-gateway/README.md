# リアルタイムゲートウェイを起動する

コマンドはリポジトリのルートで実行する。
マッチメイキングの HTTP 仕様は[OpenAPI の生成手順](../../README.md#api-ドキュメントを生成する)、
WebSocket の形式は[プロトコル実装](../../crates/realtime-gateway/src/game.rs)を参照すること。
認証、接続、資源制限の実装とテストは[HTML viewer の手順](../../docs/how-to/check-traceability.md#html-viewer-を開く)に従って確認する。
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

## Google アカウントなしで公開経路を検証する

検証用の認証プロバイダーを使い、公開入口からログイン、マッチング、着手、相手への通知、再接続、投了、ログアウトを自動検証する。
Node.js、npm、Chromium の実行に必要なシステムライブラリーと、`!override` に対応した Docker Compose を用意する。
ブラウザーの依存関係は最初に一度インストールする。
検証には変更のないコミット済みの専用チェックアウトを使い、実行終了まで編集や HEAD の変更を行わない。
ビルド前と後片付け後に確認し、ステージ済み・未ステージ・未追跡の変更や HEAD の変更があれば検証は失敗する。
`target/` など Git が無視する出力は許容する。詳細は[チェックアウトの要件](../../.agents/skills/verify/SKILL.md#launch)を参照すること。

```sh
npm ci --prefix tools/traceability-viewer-tests
./tools/traceability-viewer-tests/node_modules/.bin/playwright install chromium
./scripts/verify-app.py gateway
```

実行ごとに専用の Compose プロジェクトと空のデータベースを作り、ループバックの公開入口を使う。
検証専用の `verification-user-server` example が Alice と Bob のログインを受け付け、通常のアカウント処理がセッション Cookie を発行する。
専用イメージと [Compose 上書き設定](../../compose.verification.yaml)で認証サービスだけを置き換える。
通常の `user-server` とその配布イメージには検証用プロバイダーを含めない。
Google の認証情報は不要となり、実際の Google 連携はこのコマンドの検証対象に含まれない。

結果と操作記録は、コマンドが表示する `target/verification/shogi-verify-<id>/` に残る。
成功・失敗のどちらでも専用のコンテナーとデータを削除し、証跡を保持する。
確認項目と個別の認証拒否テストは[検証マップ](../../.agents/skills/verify/features/browser-realtime.md)を参照すること。

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

操作結果と更新通知の扱いは[ゲートウェイの公開 API](../../crates/realtime-gateway/src/lib.rs)、
初期局面と通知の同期は[ゲートウェイの対局処理](../../crates/realtime-gateway/src/game.rs)を参照すること。

## 操作結果が不明な場合や切断後に復旧する

操作は 1 件ずつ送り、送信前にリクエスト全体をクライアントに保存する。上の例では `pendingCommand` を保持する。
受理応答が消失した場合や `outcome_unknown` を受け取った場合は、元の値と同じ内容で再送する。
結果を回復するために新しい `request_id` を発行しない。

`stale_seq` または `resync_required` を受け取った場合は再接続し、新しい `snapshot` から局面を更新する。
切断後の再接続には指数バックオフと無作為な待機時間のばらつきを使う。
再接続で取得した局面の番号と、応答不明の操作を再試行するときの元の `expected_seq` は区別して保持する。

`GATEWAY_QUEUE_CAPACITY` は、接続ごとの通知バッファと送信キューそれぞれの上限である。
通知はスナップショット取得中も Valkey 購読内のバッファ 1 段だけに保持する。
購読確認の応答は通知枠を使わない。以前のゲートウェイ内の追加バッファは廃止したため、吸収できる通知の集中は小さくなる。
通知バッファの超過後は `subscription_lost` による再同期を要求する。
送信キューが満杯の場合も切断し、`resync_required` の送信は可能な場合だけ試みる。

## HTTPS の背後に配置する

認証、マッチメイキング、WebSocket を同じオリジンで提供するリバースプロキシで HTTPS を終端する。
`user-server`、`game-server`、ゲートウェイ、Valkey は内部ネットワークに置く。
ゲートウェイとユーザーサービスの公開オリジンを HTTPS に設定し、両方のループバック HTTP フラグを無効にする。

## 対局時計

スナップショットには`time_control`、`clock`、`server_now_ms`が含まれる。
値は整数ミリ秒であり、時刻はUTCエポックからの経過ミリ秒である。
`clock.state`が`running`なら、`main_remaining_ms`は先手、後手の順に手番開始時の持ち時間を表す。
`turn_started_at_ms`と`deadline_at_ms`で進行中の手番の経過と期限を示す。
`stopped`なら`byoyomi_remaining_ms`と`stopped_at_ms`を含む最終値を保持する。

表示には`server_now_ms`とローカルの単調時計から求めた時刻差を使う。
着手と終局のイベントには確定後の`clock`と過去の`evaluated_at_ms`が含まれる。
イベントが遅れて届いても、その判定時刻を現在時刻として使わない。
`accepted`の時計は元の受理時点の値であり、完全一致再送でも変わらない。
再接続時は新しいスナップショットで時刻差と局面を更新する。

`seq`は着手、投了、システムによる時間切れを含む状態遷移の数である。
時間切れの終局理由は`timeout`、通知の`origin`は`system`になる。
期限は`判定時刻 >= 期限`で失効する。
切断やサーバー停止でも時計は進む。
時計表示は0で止め、勝敗にはスナップショットまたは終局イベントの保存済み結果を使う。
