# 機能の保証とテストの対応

この一覧は `python3 scripts/traceability.py render` で生成する。

実装を正とし、各機能の保証、コンポーネント契約、実在するテストと未検証事項を結ぶ。成功状態は保存しない。

| 機能 | 公開操作と入口 | 保証 | 契約 | テスト宣言 | 元データ |
| --- | --- | --- | --- | --- | --- |
| [Google によるブラウザーログイン](traceability/account-google-login.md) | user-http: GET /auth/google/callback<br>user-http: GET /auth/google/start | 4 | 6 | 9 | [crates/user-http/src/account-google-login.trace.json](../../crates/user-http/src/account-google-login.trace.json) |
| [自分のプロフィール取得と表示名変更](traceability/account-profile.md) | user-http: GET /me<br>user-http: PATCH /me | 4 | 6 | 6 | [crates/user-http/src/account-profile.trace.json](../../crates/user-http/src/account-profile.trace.json) |
| [ブラウザーセッションの認証と失効](traceability/account-session.md) | user-http: POST /auth/logout | 4 | 6 | 8 | [crates/user-http/src/account-session.trace.json](../../crates/user-http/src/account-session.trace.json) |
| [対局イベントの永続化と順序付き配信](traceability/durable-event-dispatch.md) | Outbox 配信プロセス<br>永続 Outbox の一件配信 | 5 | 6 | 16 | [crates/game-application/src/durable-event-dispatch.trace.json](../../crates/game-application/src/durable-event-dispatch.trace.json) |
| [対局操作の再試行・永続化・復旧](traceability/game-command-recovery.md) | 対局アクターと操作の復旧 | 5 | 7 | 19 | [crates/game-application/src/game-command-recovery.trace.json](../../crates/game-application/src/game-command-recovery.trace.json) |
| [着手・将棋ルール・詰み・投了](traceability/game-play.md) | GameService.MakeMove<br>GameService.Resign | 5 | 8 | 23 | [crates/game-application/src/game-play.trace.json](../../crates/game-application/src/game-play.trace.json) |
| [対局スナップショットと閲覧制御](traceability/game-snapshots-access.md) | GameService.CheckViewAccess<br>GameService.GetSnapshot<br>GameService.GetSnapshotForViewer | 3 | 8 | 8 | [crates/game-application/src/game-snapshots-access.trace.json](../../crates/game-application/src/game-snapshots-access.trace.json) |
| [レート差に基づく対戦選択と対局作成](traceability/matchmaking-pairing.md) | 待機チケットの周期的マッチング | 4 | 4 | 9 | [crates/game-application/src/matchmaking-pairing.trace.json](../../crates/game-application/src/matchmaking-pairing.trace.json) |
| [マッチメイキングの参加・状態取得・継続・キャンセル](traceability/matchmaking-tickets.md) | GameService.CancelMatchmaking<br>GameService.GetMatchmaking<br>GameService.GetMatchmakingTicket<br>GameService.HeartbeatMatchmaking<br>GameService.JoinMatchmaking<br>realtime-gateway: DELETE /matchmaking/tickets/{ticket_id}<br>realtime-gateway: GET /matchmaking/me<br>realtime-gateway: GET /matchmaking/tickets/{ticket_id}<br>realtime-gateway: POST /matchmaking/tickets/{ticket_id}/heartbeat<br>realtime-gateway: PUT /matchmaking/tickets/{ticket_id} | 4 | 8 | 9 | [crates/game-application/src/matchmaking-tickets.trace.json](../../crates/game-application/src/matchmaking-tickets.trace.json) |
| [公開 API 仕様の取得とオフライン生成](traceability/public-api-documents.md) | player-http: GET /openapi.json<br>user-http: GET /openapi.json<br>オフラインのマッチメイキング API 仕様 | 5 | 6 | 10 | [crates/player-http/src/public-api-documents.trace.json](../../crates/player-http/src/public-api-documents.trace.json) |
| [終了済み公開対局の履歴取得](traceability/public-record-detail.md) | player-http: GET /game-records/{id} | 4 | 5 | 9 | [crates/player-http/src/public-record-detail.trace.json](../../crates/player-http/src/public-record-detail.trace.json) |
| [終了済み公開対局の検索](traceability/public-record-search.md) | player-http: GET /game-records | 4 | 5 | 7 | [crates/player-http/src/public-record-search.trace.json](../../crates/player-http/src/public-record-search.trace.json) |
| [レート更新ジョブの実行と隔離からの復旧](traceability/rating-application.md) | レートジョブのポーリングと隔離復旧 CLI<br>レートジョブの処理 | 5 | 7 | 12 | [crates/game-application/src/rating-application.trace.json](../../crates/game-application/src/rating-application.trace.json) |
| [対局の初期局面と確定イベントのリアルタイム配信](traceability/realtime-game-feed.md) | スナップショットと通知の接続処理 | 4 | 5 | 6 | [crates/realtime-gateway/src/realtime-game-feed.trace.json](../../crates/realtime-gateway/src/realtime-game-feed.trace.json) |
| [WebSocket の認証・対局操作・資源制限](traceability/realtime-session-commands-limits.md) | WebSocket move と resign の入力<br>認証済み WebSocket 接続 | 5 | 8 | 10 | [crates/realtime-gateway/src/realtime-session-commands-limits.trace.json](../../crates/realtime-gateway/src/realtime-session-commands-limits.trace.json) |
| [サービスの起動設定・スキーマ互換性・DB 分離](traceability/startup-schema-isolation.md) | ユーザー DB の初期化とスキーマ検査<br>ユーザーサービス起動<br>リアルタイムゲートウェイ起動<br>公開データサービス起動<br>公開データ読み取り接続<br>対局 DB の初期化とスキーマ検査 | 5 | 9 | 14 | [crates/game-persistence/src/startup-schema-isolation.trace.json](../../crates/game-persistence/src/startup-schema-isolation.trace.json) |
| [ユーザー別の確定済みレート取得](traceability/user-ratings.md) | player-http: GET /users/{user_id}/ratings | 6 | 9 | 8 | [crates/player-http/src/ratings.trace.json](../../crates/player-http/src/ratings.trace.json) |

## 一覧の検査範囲

[操作と入口の台帳](../../scripts/traceability-surfaces.json) は crates/ と apps/ の既存 examples/*openapi.rs から生成する OpenAPI 操作、指定した gRPC service の rpc 名、ソース位置付きの処理入口を機能に割り当てる。
check は OpenAPI exporter と操作の追加・削除、gRPC 操作の追加・削除、未割当の所有者、台帳から参照されない機能、参照元データと生成文書の不足・余分を検出する。
新しいバックグラウンド処理や WebSocket メッセージを自動発見すること、機能の分割が適切かを判定することはできない。台帳の入口と source はレビューで保つ。
テスト宣言数は機能内の件数であり、同じ Cargo テストが複数機能で共有されることがある。run は package・target・完全なテスト名が同じものを一度だけ実行する。
asserts は記載した事例への直接の assertion、exercises は処理を通る関係を示す。位置の一致とテストの成功は、保証の意味や網羅性の証明ではない。

更新方法は [仕様とテストの対応を更新する](../how-to/check-traceability.md) を参照する。
