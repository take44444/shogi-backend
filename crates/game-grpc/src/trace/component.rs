use crate::trace_support as trace;
trace::component!(trace::Component {
    description: "game-grpc コンポーネント",
    notes: &[
        "game-play/rpc-errors。アプリケーションとドメインのエラーを安定した gRPC 区分へ変換する。通常の拒否は FailedPrecondition、非参加者は PermissionDenied、古いシーケンスは actual-seq 付き Aborted、ルール処理や基盤の内部エラーは Internal とする。",
        "game-play/rpc-move。MakeMove は対局・リクエスト・ユーザー ID を検証し、expected_seq を必須とし、USI を解析して型付きのアプリケーション操作を渡す。",
        "game-play/rpc-resign。Resign は対局・リクエスト・ユーザー ID と必須の予想シーケンスを検証し、ゲートウェイで認証済みの参加者の申告をアプリケーションへ委譲する。",
        "game-snapshots-access/grpc-shape。スナップショットの変換には、参加者の割り当て、シーケンス、駒がある升目、両者のすべての持ち駒、手番・王手・結果、合法 USI 手を含める。 未検証範囲。選択した通信テストは、盤面と持ち駒の完全な順序や、すべての駒・列挙型の変換分岐を比較しない。",
        "game-snapshots-access/grpc-viewer。GetSnapshotForViewer と CheckViewAccess は対局・閲覧者 ID を検証し、呼び出し元の閲覧者の申告を渡してアプリケーションエラーを変換するが、その申告自体は認証しない。 未検証範囲。閲覧者向けの両 RPC で、不正な閲覧者 ID と対局 ID の境界をそれぞれ検証してはいない。",
        "matchmaking-tickets/rpc-boundary。五つの組合せ RPC は所有者と UUID を解析し、対応するプールと正のハートビートを検証する。所有者に限定した操作を委譲し、チケット状態を直列化する。 未検証範囲。すべての RPC で不正な所有者と UUID の事例を列挙してはいない。"
    ],
    external: &[],
    operations: &[
        trace::Operation {
            contract: "game-grpc/rpc-make-move",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "MakeMove",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-resign",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "Resign",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-get-snapshot",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "GetSnapshot",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-get-snapshot-for-viewer",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "GetSnapshotForViewer",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-check-view-access",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "CheckViewAccess",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-join-matchmaking",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "JoinMatchmaking",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-get-matchmaking",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "GetMatchmaking",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-heartbeat-matchmaking",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "HeartbeatMatchmaking",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-cancel-matchmaking",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "CancelMatchmaking",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        },
        trace::Operation {
            contract: "game-grpc/rpc-ready-matchmaking",
            audience: trace::Audience::Internal,
            surface: trace::Surface::Grpc {
                operation: "ReadyMatchmaking",
                path: "proto/game.proto",
                service: "GameService"
            },
            reason: "フロントエンドはゲートウェイを経由し、このサービス RPC を直接呼び出さない。"
        }
    ],
    entrypoints: &[]
});
