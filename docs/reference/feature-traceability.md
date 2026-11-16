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
Cargo が列挙する全サンプルプログラムに、型検査される Example 宣言と理由を必須とする。OpenApi 宣言のサンプルだけを実行し、出力された操作と Surface 宣言の完全一致を検査する。Program 宣言はコンパイルのみ行う。proto/ の全サービス・RPC も探索して Surface 宣言と照合する。
check は OpenAPI 出力プログラムと操作の追加・削除、gRPC 操作の追加・削除、未割当の所有者、台帳から参照されない機能、参照元データと生成文書の不足・余分を検出する。
新しいバックグラウンド処理や WebSocket メッセージを自動発見すること、機能の分割が適切かを判定することはできない。台帳の入口とソース参照はレビューで保つ。
テスト宣言数は機能内の件数であり、同じ Cargo テストが複数機能で共有されることがある。run はパッケージ・ターゲット・完全なテスト名が同じものを一度だけ実行する。
「事例を直接検証」は記載した事例を直接確かめる関係、「処理を通る」は対象の処理を実行する関係を示す。位置の一致とテストの成功は、保証の意味や網羅性の証明ではない。

更新方法は [仕様とテストの対応を更新する](../how-to/check-traceability.md) を参照する。

## ワークスペース全体のテスト分類

コンパイル済みテスト 196 件。機能に関連付けた固有テスト 190 件、基盤の検査 5 件、テスト実行補助 1 件。共有テストは機能ごとの宣言数に重複して現れるが、この合計では一度だけ数える。

標準ホストと Cargo の既定の機能フラグを使用する。test = false のターゲットは Cargo の指定どおり一覧から除く。required-features を持つターゲットは現在の規則では明示的に拒否する。テストがないターゲットも一覧照合の対象にする。

- `game-persistence / test / outbox / outbox::publisher_lock_probe` (テスト実行補助) [crates/game-persistence/tests/trace/durable-event-dispatch/support.rs](../../crates/game-persistence/tests/trace/durable-event-dispatch/support.rs#L89): 配信ロックの駆動テストからだけ呼び出す、子プロセス用の補助テスト。
- `game-persistence / test / sql_policy / policy_accepts_builders_bound_catalog_queries_and_test_fixtures` (基盤の検査 / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L497): 許可するテスト用データとクエリービルダーの例で SQL 方針の解析器を検証する。リポジトリ開発用の検査である。
- `game-persistence / test / sql_policy / policy_distinguishes_test_functions_from_production_sql` (基盤の検査 / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L519): テスト専用の準備用 SQL を本番コードの診断対象から除き、通常の関数と不正なテスト属性の検査を維持する。
- `game-persistence / test / sql_policy / policy_rejects_application_sql_and_interpolation` (基盤の検査 / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L471): 禁止するソース例に対する SQL 方針の診断を検証する。リポジトリ開発用の検査である。
- `game-persistence / test / sql_policy / production_sql_uses_builders_or_reviewed_postgres_operations` (基盤の検査 / sql-policy) [crates/game-persistence/tests/sql_policy.rs](../../crates/game-persistence/tests/sql_policy.rs#L448): リポジトリの本番ソースに対する SQL 構築方針を検査する。製品の契約を検証するものではない。
- `pg-test-support / lib / (lib) / tests::test_databases_have_independent_names_and_contents` (基盤の検査 / test-support) [crates/pg-test-support/src/lib.rs](../../crates/pg-test-support/src/lib.rs#L101): テスト実行用に提供する、独立した一時 PostgreSQL データベースを検証する。

## Cargo のサンプルプログラムの分類

- `game-grpc / outbox_proof` (プログラム（コンパイルのみ）) [crates/game-grpc/examples/outbox_proof.rs](../../crates/game-grpc/examples/outbox_proof.rs): サービスの再起動を含め、gRPC、Valkey、Docker Composeを通したOutboxの復旧処理を実行する。
- `game-grpc / persistence_proof` (プログラム（コンパイルのみ）) [crates/game-grpc/examples/persistence_proof.rs](../../crates/game-grpc/examples/persistence_proof.rs): gRPCサーバーの再起動をまたいで永続化の処理を実行し、指定ディレクトリに検証用ファイルを出力する。
- `game-grpc / play` (プログラム（コンパイルのみ）) [crates/game-grpc/examples/play.rs](../../crates/game-grpc/examples/play.rs): 稼働中のgRPCサービスを通して対局を進め、着手と投了の応答を確認する。
- `game-grpc / rating_proof` (プログラム（コンパイルのみ）) [crates/game-grpc/examples/rating_proof.rs](../../crates/game-grpc/examples/rating_proof.rs): 稼働中のgRPCサービスを通して対局を終え、レーティングの検証に使う。
- `player-http / player-openapi` (OpenAPI 仕様の出力) [crates/player-http/examples/player-openapi.rs](../../crates/player-http/examples/player-openapi.rs): 公開対局 HTTP の OpenAPI 文書を JSON として出力する。
- `realtime-gateway / matchmaking-openapi` (OpenAPI 仕様の出力) [crates/realtime-gateway/examples/matchmaking-openapi.rs](../../crates/realtime-gateway/examples/matchmaking-openapi.rs): マッチメイキング用HTTP APIのOpenAPI文書をJSONとして出力する。
- `user-http / user-openapi` (OpenAPI 仕様の出力) [crates/user-http/examples/user-openapi.rs](../../crates/user-http/examples/user-openapi.rs): アカウント HTTP の OpenAPI 文書を JSON として出力する。
