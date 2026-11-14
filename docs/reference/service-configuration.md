# サービス設定の参照仕様

以下の設定はローカルのバイナリーに適用する。
Docker Compose のコンテナー設定は [compose.yaml](../../compose.yaml) で別に指定する。

## 棋譜サービス

`player-server` は、認証なしで公開棋譜と確定済みレートを返す。
スキーマバージョン 7 を要求し、スキーマの初期化や更新は行わない。

| 変数 | 既定値 | 意味 |
| --- | --- | --- |
| `PLAYER_BIND_ADDR` | `127.0.0.1:8081` | HTTP 待ち受けアドレス |
| `PLAYER_GAME_DATABASE_URL` | `postgres://shogi_reader:shogi_reader@localhost:5432/shogi` | SELECT 専用の認証情報を含む PostgreSQL URL |

読み取りロールには `schema_version`、`games`、`journal`、`rating_states` の SELECT 権限が必要になる。
テーブルの所有権や書き込み権限は持たせない。
[スキーマ初期化処理](../../crates/game-persistence/src/schema.rs)は、`shogi_reader` があれば対象テーブルへの権限を付与する。
[初期設定 SQL](../../scripts/init-postgres.sql) はそのロールを作成し、データベースへの接続とスキーマの使用を許可する。
読み取りロールは `rated_games`、`rating_slots`、`rating_jobs`、`rating_applications` の SELECT 権限を持たない。

## ユーザーサービス

`user-server` は専用の PostgreSQL データベースにアカウントを保存する。

| 変数 | 必須 | 既定値と制約 |
| --- | --- | --- |
| `GOOGLE_CLIENT_ID` | はい | Google のウェブクライアント ID |
| `GOOGLE_CLIENT_SECRET` | はい | Google のウェブクライアントシークレット |
| `USER_PUBLIC_ORIGIN` | はい | 認証情報、パス、クエリ、フラグメントを持たない HTTPS オリジン。コールバック URL と更新操作に許可する Origin を決める。 |
| `USER_DATABASE_URL` | はい | 専用の `users` データベースの PostgreSQL URL。データベースは事前作成が必要。 |
| `USER_BIND_ADDR` | いいえ | `127.0.0.1:8080`。内部 HTTP 待ち受けアドレス。 |
| `USER_ALLOW_LOOPBACK_HTTP` | いいえ | `false`。`true` は `localhost`、`127.0.0.1`、`[::1]` の HTTP 公開オリジンを許可する。 |
| `USER_SESSION_SECONDS` | いいえ | `86400`。セッションの絶対有効期間。`60` から `2592000` 秒まで。 |

起動時は、待ち受けを開始する前に Google のディスカバリー情報と JWKS を取得する。
コード交換でもディスカバリー情報と署名鍵を再取得する。
送信リクエストは HTTPS を使い、リダイレクトを追跡しない。
接続タイムアウトは 5 秒、各リクエストのタイムアウトは 10 秒となる。

データベースロールには、起動のたびにデータベースの `CREATE` 権限と、`public` 内でオブジェクトを作成する権限が必要になる。
初期化と検証は[アカウント保存層の参照仕様](../../crates/user-persistence/README.md#起動時のスキーマ処理)に記述する。

## レートワーカー

`rating-worker-server` は `game-server` と同じ対局データベースを使う。
ポーリング前にスキーマバージョン 7 を初期化または検証し、Ctrl-C で停止する。

| 変数 | 既定値 | 意味 |
| --- | --- | --- |
| `DATABASE_URL` | `postgres://shogi:shogi@localhost:5432/shogi` | 対局データベースへの書き込み接続 |
| `RATING_WORKER_ID` | `worker-<process-id>` | Unicode の空白を含まない、1 から 128 UTF-8 バイトまでのプロセス識別子。同時実行するワーカーには異なる名前が必要。 |
| `RATING_LEASE_MS` | `30000` | ジョブ処理権の有効期間。正のミリ秒値。 |
| `RATING_RETRY_LIMIT` | `5` | 正の最大取得回数。期限切れの処理権取得も数える。 |
| `RATING_BACKOFF_MS` | `1000` | 一時的な失敗の再試行前に待つ非負の時間。単位はミリ秒。 |
| `RATING_POLL_MS` | `250` | 正のポーリング間隔。単位はミリ秒。 |

## リアルタイムゲートウェイ

`realtime-gateway-server` はデータベースの認証情報を持たない。
認証には `user-server`、対局操作と閲覧権限には `game-server` を呼び出す。

| 変数 | 既定値 | 意味 |
| --- | --- | --- |
| `GATEWAY_BIND_ADDR` | `127.0.0.1:8082` | HTTP 待ち受けアドレス |
| `GATEWAY_PUBLIC_ORIGIN` | 必須 | 認証情報、パス、クエリ、フラグメントを持たない HTTPS ブラウザーオリジン |
| `GATEWAY_ALLOW_LOOPBACK_HTTP` | `false` | `true` は `localhost`、`127.0.0.1`、`[::1]` の HTTP ブラウザーオリジンを許可する。 |
| `USER_SERVICE_URL` | 必須 | 固定の内部ユーザー HTTP サービスのオリジン |
| `GAME_SERVICE_URL` | 必須 | 固定の内部対局 gRPC 接続先 |
| `VALKEY_URL` | 必須 | `redis://` で指定する Valkey 接続先 |
| `GATEWAY_MAX_CONNECTIONS` | `256` | 同時接続数。1 から 100000 まで。 |
| `GATEWAY_MAX_USER_CONNECTIONS` | `8` | ユーザーごとの接続数。1 から 1000 まで。 |
| `GATEWAY_QUEUE_CAPACITY` | `64` | 接続ごとに保持する通知と送信メッセージのキュー容量。1 から 4096 まで。 |
| `GATEWAY_MAX_INPUT_BYTES` | `4096` | 受信メッセージの容量。1 から 1048576 バイトまで。 |
| `GATEWAY_MAX_SNAPSHOT_BYTES` | `262144` | シリアライズしたスナップショットの容量。1 から 4194304 バイトまで。 |
| `GATEWAY_MESSAGES_PER_SECOND` | `10` | 接続ごとの 1 秒あたりの受信メッセージ数。1 から 1000 まで。 |

HTTP 開発環境は `user_session` Cookie、HTTPS は `__Host-user_session` Cookie を使う。
セッション Cookie を共有するには、ユーザーサービスとゲートウェイのブラウザー向けホストが一致する必要がある。
