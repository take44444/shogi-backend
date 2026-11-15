# 機能の保証とテストの対応

この一覧は `python3 scripts/traceability.py render` で生成する。

実装を正とし、各機能の保証、コンポーネント契約、実在するテストと未検証事項を結ぶ。成功状態は保存しない。

| 機能 | 公開操作と入口 | 保証 | 契約 | テスト宣言 | 元データ |
| --- | --- | --- | --- | --- | --- |
| [Google によるブラウザーログイン](traceability/account-google-login.md) | user-http: GET /auth/google/callback<br>user-http: GET /auth/google/start | 4 | 6 | 11 | [crates/user-http/src/trace/account-google-login/feature.rs](../../crates/user-http/src/trace/account-google-login/feature.rs) |
| [自分のプロフィール取得と表示名変更](traceability/account-profile.md) | user-http: GET /me<br>user-http: PATCH /me | 4 | 6 | 6 | [crates/user-http/src/trace/account-profile/feature.rs](../../crates/user-http/src/trace/account-profile/feature.rs) |
| [ブラウザーセッションの認証と失効](traceability/account-session.md) | user-http: POST /auth/logout | 4 | 6 | 8 | [crates/user-http/src/trace/account-session/feature.rs](../../crates/user-http/src/trace/account-session/feature.rs) |
| [対局イベントの永続化と順序付き配信](traceability/durable-event-dispatch.md) | Outbox 配信プロセス<br>永続 Outbox の一件配信 | 5 | 6 | 16 | [crates/game-application/src/trace/durable-event-dispatch/feature.rs](../../crates/game-application/src/trace/durable-event-dispatch/feature.rs) |
| [対局操作の再試行・永続化・復旧](traceability/game-command-recovery.md) | 対局アクターと操作の復旧 | 5 | 7 | 24 | [crates/game-application/src/trace/game-command-recovery/feature.rs](../../crates/game-application/src/trace/game-command-recovery/feature.rs) |
| [着手・将棋ルール・詰み・投了](traceability/game-play.md) | GameService.MakeMove<br>GameService.Resign | 5 | 8 | 29 | [crates/game-application/src/trace/game-play/feature.rs](../../crates/game-application/src/trace/game-play/feature.rs) |
| [対局スナップショットと閲覧制御](traceability/game-snapshots-access.md) | GameService.CheckViewAccess<br>GameService.GetSnapshot<br>GameService.GetSnapshotForViewer | 3 | 8 | 11 | [crates/game-application/src/trace/game-snapshots-access/feature.rs](../../crates/game-application/src/trace/game-snapshots-access/feature.rs) |
| [レート差に基づく対戦選択と対局作成](traceability/matchmaking-pairing.md) | 待機チケットの周期的マッチング | 4 | 4 | 10 | [crates/game-application/src/trace/matchmaking-pairing/feature.rs](../../crates/game-application/src/trace/matchmaking-pairing/feature.rs) |
| [マッチメイキングの参加・状態取得・継続・キャンセル](traceability/matchmaking-tickets.md) | GameService.CancelMatchmaking<br>GameService.GetMatchmaking<br>GameService.GetMatchmakingTicket<br>GameService.HeartbeatMatchmaking<br>GameService.JoinMatchmaking<br>realtime-gateway: DELETE /matchmaking/tickets/{ticket_id}<br>realtime-gateway: GET /matchmaking/me<br>realtime-gateway: GET /matchmaking/tickets/{ticket_id}<br>realtime-gateway: POST /matchmaking/tickets/{ticket_id}/heartbeat<br>realtime-gateway: PUT /matchmaking/tickets/{ticket_id} | 4 | 8 | 11 | [crates/game-application/src/trace/matchmaking-tickets/feature.rs](../../crates/game-application/src/trace/matchmaking-tickets/feature.rs) |
| [公開 API 仕様の取得とオフライン生成](traceability/public-api-documents.md) | player-http: GET /openapi.json<br>user-http: GET /openapi.json<br>オフラインのマッチメイキング API 仕様 | 5 | 6 | 10 | [crates/player-http/src/trace/public-api-documents/feature.rs](../../crates/player-http/src/trace/public-api-documents/feature.rs) |
| [終了済み公開対局の履歴取得](traceability/public-record-detail.md) | player-http: GET /game-records/{id} | 4 | 5 | 13 | [crates/player-http/src/trace/public-record-detail/feature.rs](../../crates/player-http/src/trace/public-record-detail/feature.rs) |
| [終了済み公開対局の検索](traceability/public-record-search.md) | player-http: GET /game-records | 4 | 5 | 8 | [crates/player-http/src/trace/public-record-search/feature.rs](../../crates/player-http/src/trace/public-record-search/feature.rs) |
| [レート更新ジョブの実行と隔離からの復旧](traceability/rating-application.md) | レートジョブのポーリングと隔離復旧 CLI<br>レートジョブの処理 | 5 | 7 | 12 | [crates/game-application/src/trace/rating-application/feature.rs](../../crates/game-application/src/trace/rating-application/feature.rs) |
| [対局の初期局面と確定イベントのリアルタイム配信](traceability/realtime-game-feed.md) | スナップショットと通知の接続処理 | 4 | 5 | 8 | [crates/realtime-gateway/src/trace/realtime-game-feed/feature.rs](../../crates/realtime-gateway/src/trace/realtime-game-feed/feature.rs) |
| [WebSocket の認証・対局操作・資源制限](traceability/realtime-session-commands-limits.md) | WebSocket move と resign の入力<br>認証済み WebSocket 接続 | 5 | 8 | 10 | [crates/realtime-gateway/src/trace/realtime-session-commands-limits/feature.rs](../../crates/realtime-gateway/src/trace/realtime-session-commands-limits/feature.rs) |
| [サービスの起動設定・スキーマ互換性・DB 分離](traceability/startup-schema-isolation.md) | ユーザー DB の初期化とスキーマ検査<br>ユーザーサービス起動<br>リアルタイムゲートウェイ起動<br>公開データサービス起動<br>公開データ読み取り接続<br>対局 DB の初期化とスキーマ検査 | 5 | 9 | 23 | [crates/game-persistence/src/trace/startup-schema-isolation/feature.rs](../../crates/game-persistence/src/trace/startup-schema-isolation/feature.rs) |
| [ユーザー別の確定済みレート取得](traceability/user-ratings.md) | player-http: GET /users/{user_id}/ratings | 6 | 9 | 8 | [crates/player-http/src/trace/user-ratings/feature.rs](../../crates/player-http/src/trace/user-ratings/feature.rs) |

## 一覧の検査範囲

入口、保証、操作の所有者はコンパイルされる Feature、Contract、Case、Evidence、Surface 宣言から取得する。
Cargo が列挙する全 example に、型検査される Example 宣言と理由を必須とする。OpenApi 宣言の example だけを実行し、出力された操作と Surface 宣言の完全一致を検査する。Program 宣言はコンパイルのみ行う。proto/ の全 service・RPC も探索して Surface 宣言と照合する。
check は OpenAPI exporter と操作の追加・削除、gRPC 操作の追加・削除、未割当の所有者、台帳から参照されない機能、参照元データと生成文書の不足・余分を検出する。
新しいバックグラウンド処理や WebSocket メッセージを自動発見すること、機能の分割が適切かを判定することはできない。台帳の入口と source はレビューで保つ。
テスト宣言数は機能内の件数であり、同じ Cargo テストが複数機能で共有されることがある。run は package・target・完全なテスト名が同じものを一度だけ実行する。
asserts は記載した事例への直接の assertion、exercises は処理を通る関係を示す。位置の一致とテストの成功は、保証の意味や網羅性の証明ではない。

更新方法は [仕様とテストの対応を更新する](../how-to/check-traceability.md) を参照する。

## 全 workspace テストの分類

コンパイル済みテスト 196 件。機能に関連付けた固有テスト 190 件、インフラ 5 件、テスト補助 1 件。共有テストは機能ごとの宣言数に重複して現れるが、この合計では一度だけ数える。

標準ホストとデフォルト Cargo feature を使用する。test = false の target は Cargo の指定どおり一覧から除く。required-features を持つ target は現在のポリシーでは明示的に拒否する。空の target も一覧照合の対象にする。

- `game-persistence / test / outbox / publisher_lock_probe` (helper) [crates/game-persistence/tests/outbox.rs](../../crates/game-persistence/tests/outbox.rs#L87): Child-process harness invoked only by the publisher lock driver.
- `game-persistence / test / sql_policy / policy_accepts_builders_bound_catalog_queries_and_test_fixtures` (infrastructure / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L493): Checks the SQL policy analyzer against permitted fixture and query-builder examples; it is a repository development control.
- `game-persistence / test / sql_policy / policy_distinguishes_test_functions_from_production_sql` (infrastructure / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L515): Keeps test-only fixture SQL out of production diagnostics while retaining checks for ordinary functions and invalid test attributes.
- `game-persistence / test / sql_policy / policy_rejects_application_sql_and_interpolation` (infrastructure / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L467): Checks SQL policy diagnostics for forbidden source examples; it is a repository development control.
- `game-persistence / test / sql_policy / production_sql_uses_builders_or_reviewed_postgres_operations` (infrastructure / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L444): Scans repository production sources for the SQL construction policy; it does not assert a product contract.
- `pg-test-support / lib / (lib) / tests::test_databases_have_independent_names_and_contents` (infrastructure / test-support) [crates/pg-test-support/src/lib.rs](../../crates/pg-test-support/src/lib.rs#L101): Validates isolated temporary PostgreSQL databases supplied to the test harness.

## Cargo example の分類

- `game-grpc / outbox_proof` (Program) [crates/game-grpc/examples/outbox_proof.rs](../../crates/game-grpc/examples/outbox_proof.rs): Exercises outbox recovery through gRPC, Valkey and Docker Compose, including service restarts.
- `game-grpc / persistence_proof` (Program) [crates/game-grpc/examples/persistence_proof.rs](../../crates/game-grpc/examples/persistence_proof.rs): Exercises persistence across gRPC server restarts and writes proof files to a supplied directory.
- `game-grpc / play` (Program) [crates/game-grpc/examples/play.rs](../../crates/game-grpc/examples/play.rs): Plays a game through a running gRPC service and checks move and resignation responses.
- `game-grpc / rating_proof` (Program) [crates/game-grpc/examples/rating_proof.rs](../../crates/game-grpc/examples/rating_proof.rs): Finishes a game through a running gRPC service to support rating verification.
- `player-http / player-openapi` (OpenApi) [crates/player-http/examples/player-openapi.rs](../../crates/player-http/examples/player-openapi.rs): Prints the public player HTTP OpenAPI document as JSON.
- `realtime-gateway / matchmaking-openapi` (OpenApi) [crates/realtime-gateway/examples/matchmaking-openapi.rs](../../crates/realtime-gateway/examples/matchmaking-openapi.rs): Prints the matchmaking HTTP OpenAPI document as JSON.
- `user-http / user-openapi` (OpenApi) [crates/user-http/examples/user-openapi.rs](../../crates/user-http/examples/user-openapi.rs): Prints the account HTTP OpenAPI document as JSON.
