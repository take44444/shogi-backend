# game-grpc の参照仕様

生成された `shogi.v1.GameService` API は平文の gRPC を使う。
スキーマは [`proto/game.proto`](../../proto/game.proto) に定義する。
同梱の `protoc` が Rust のクライアントとサーバーを生成する。
`GameRpc::new(GameService, Arc<dyn MatchmakingRepository>)` が対局操作を振り分ける。
ユーザー管理は独立した HTTP サービスが担当し、gRPC メソッドは持たない。

## 閲覧権限

`GetSnapshotForViewer(game_id, viewer_user_id)` は、公開状態、参加者、履歴を同じデータベーススナップショットから取得し、局面を復元する。
`CheckViewAccess(game_id, viewer_user_id)` は履歴を読まずに、現在の公開状態と参加者を確認する。

公開対局は認証済みユーザーが閲覧でき、非公開対局は参加者だけが閲覧できる。
存在しない対局と閲覧できない対局は、いずれも `NotFound` を返す。
信頼された呼び出し元が閲覧者を認証してから `viewer_user_id` を渡す。
gRPC の待ち受け自体は呼び出し元を認証しない。

## リクエストとレスポンス

| メソッド | 必須のリクエストフィールド | レスポンス |
|---|---|---|
| `JoinMatchmaking` | 認証済み `user_id`、UUID の `ticket_id`、`pool` | チケットと初回受理フラグ |
| `GetMatchmaking` | 認証済み `user_id` | 有効なチケット、または参加なし |
| `GetMatchmakingTicket` | 認証済み `user_id`、UUID の `ticket_id` | 保存済みチケット |
| `HeartbeatMatchmaking` | 認証済み `user_id`、UUID の `ticket_id`、正の `sequence` | 現在のチケット |
| `CancelMatchmaking` | 認証済み `user_id`、UUID の `ticket_id` | 参加前のキャンセルも含む現在のチケット |
| `GetSnapshot` | `game_id` | `Snapshot` |
| `GetSnapshotForViewer` | `game_id`、認証済みとして申告する `viewer_user_id` | `Snapshot`。存在しない場合や閲覧できない場合は `NotFound`。 |
| `CheckViewAccess` | `game_id`、認証済みとして申告する `viewer_user_id` | 空の成功応答。存在しない場合や閲覧できない場合は `NotFound`。 |
| `MakeMove` | `game_id`、`request_id`、`expected_seq`、`user_id`、`usi` | `Acceptance` |
| `Resign` | `game_id`、`request_id`、`expected_seq`、`user_id` | `Acceptance` |

ID は空白を含まない 1 から 128 バイトまでの値となる。
`expected_seq` が protobuf の `optional` なのは、未指定と有効な 0 を区別するためだけである。
両更新メソッドは、シーケンス番号の未指定を拒否する。`MakeMove` は厳密な USI 着手表記を受け付ける。
ユーザー ID は呼び出し元の申告値となる。
この API は認証を行わず、信頼できる内部呼び出し元を対象とする。
マッチメイキングが対戦相手、対局 ID、先後を決定する。
HTTP ユーザーサービスは、これらの gRPC 呼び出しを認証しない。

クライアントは参加者として対応付けられたユーザー ID を渡し、サーバーが先後を決定する。
出力には引き続き `Player` 列挙型を使う。
予約済みフィールドと通信上のフィールド番号は [`game.proto`](../../proto/game.proto) に定義する。

`Snapshot` は、駒のある升の盤面、先後合わせて 14 種類の持ち駒枚数、手番、王手状態、任意の結果、
合法な USI 着手、参加者 ID、シーケンス番号を含む。
終了済みのスナップショットは局面と手番を保持するが、合法手は空となる。
`Acceptance` は受理したシーケンス番号、王手状態、任意の結果を含む。
同一内容の再試行は、再起動後や終局後も元の受理結果を返す。

## エラーと再試行

| アプリケーションの結果 | gRPC コード |
|---|---|
| 不正な ID や着手、シーケンス番号の欠落、不正なハートビート番号 | `InvalidArgument` |
| 未知の対局、または閲覧権限のないユーザーによる閲覧 API の呼び出し | `NotFound` |
| ユーザーが参加者ではない | `PermissionDenied` |
| チケットのプール競合、または受理済みリクエスト ID の内容の競合 | `AlreadyExists` |
| 古いシーケンス番号 | `Aborted`。`actual-seq` メタデータを含む。 |
| 手番違い、不正な着手、終了済み対局 | `FailedPrecondition` |
| 保存層の障害、または結果不明 | `Unavailable` |
| 永続履歴の破損、または非互換スキーマ | `DataLoss` |
| 内部不変条件の違反、ルール実装の失敗、シーケンス番号の上限到達 | `Internal` |

`Unavailable` またはレスポンスの消失後は、同じリクエスト ID と同じ内容で再試行する。
キュー投入後のキャンセルは、永続化する操作を取り消さない。

マッチメイキングの拒否は RPC としては成功し、`refused` 状態と保存済みの理由を返す。
HTTP ゲートウェイは参加拒否を 409 に変換する。
所有者の範囲内に存在しないチケットは `NotFound` を返す。
同じ UUID を別のプールで使うと `AlreadyExists` を返す。
参加結果が不明な場合は同じ UUID とプールで再試行するか、チケットを取得する。
本番用の RPC とアプリケーションメソッドには、任意の参加者を指定して対局を作成する機能はない。

## サンプルとテスト

`play` サンプルは着手と投了を確認する。
`persistence_proof` は `seed`、`resume`、`finished` を別々に実行し、protobuf 成果物で再起動後の保持を検証する。
実行コマンドは[ルートの README](../../README.md) に記述する。

アダプターのテストは変換と TCP リクエストを対象とする。
実行コマンドは[ワークスペースの検証手順](../../README.md#ワークスペースを検証する)に記述する。
