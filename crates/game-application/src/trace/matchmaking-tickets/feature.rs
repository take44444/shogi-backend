use crate::trace_support as trace;
trace::feature!(trace::Feature {
    title: "マッチメイキングの参加・状態取得・継続・準備確認・キャンセル",
    source: trace::Source {
        path: "crates/game-application/src/matchmaking.rs",
        needle: "pub enum TicketState {"
    },
    flow: &[
        "ブラウザーの現在参加状態・参加・ハートビート・準備完了・取消の各ハンドラーは、Cookie を認証し、所有者を導出してから対応する GameService RPC を呼ぶ。",
        "GameRpc は型付きの所有者・UUID・プール・ハートビートを検証し、JoinMatchmaking、GetMatchmaking、HeartbeatMatchmaking、ReadyMatchmaking、CancelMatchmaking を永続リポジトリへ委譲する。",
        "PostgreSQL トランザクションは参加受け入れを直列化し、古い待機状態を失効させ、過去のチケット・拒否・取消記録を保持する。組合せ処理は別に追跡する。"
    ],
    guarantees: &[
        trace::Guarantee {
            id: "owner-admission",
            statement: "所有者は対応するどちらのレーティングプールにも参加を要求できるが、プール全体で待機中・準備待ち・対局中の参加は一件に限られる。サーバーは参加時のレーティングを固定し、UUID による再試行の識別を保持する。",
            contracts: &["admission", "rpc-boundary", "browser-owner"]
        },
        trace::Guarantee {
            id: "durable-history",
            statement: "現在参加の取得と、ハートビートや元の操作の再試行による保存済み結果の取得は所有者に限定する。同一の再試行は保存済み結果を保持し、プールの変更は競合する。参加拒否と参加前の取消は永続化される。",
            contracts: &[
                "admission",
                "current",
                "heartbeat",
                "cancel",
                "rpc-boundary"
            ]
        },
        trace::Guarantee {
            id: "waiting-lifecycle",
            statement: "待機は、参加受け入れ時またはシーケンスが厳密に大きい最新の正のハートビートから 30 秒続く。同じか古いハートビートでは延長も復活もしない。取消は待機を終了させるが、組合せ済みの対局は取り消せない。",
            contracts: &["heartbeat", "cancel"]
        },
        trace::Guarantee {
            id: "browser-contract",
            statement: "ブラウザーの五つの組合せ操作は有効なセッションを必須とし、Host と変更時の Origin・same-site を確認する。呼び出し元の user_id を拒否し、安定したチケット・エラー JSON を返して no-store を適用する。OpenAPI は五つすべての操作を記述する。",
            contracts: &["browser-owner", "browser-response", "browser-schema"]
        }
    ],
    limitations: &[
        "gRPC は呼び出し元の識別情報の申告を受け入れ、ユーザーを認証しない。認証済み所有者の導出はブラウザーゲートウェイが担う。",
        "選択したテストは、ハートビートの全順序、不正な全フィールド、no-store の全応答区分、接続先の失敗の全分岐を網羅しない。",
        "リンクとテスト成功だけでは意味上の完全性は成立しない。保証、根拠の強さ、分割の変更にはレビューが必要となる。"
    ],
    entrypoints: &[],
    surfaces: &[
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "GET /matchmaking/me"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "PUT /matchmaking/tickets/{ticket_id}"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "DELETE /matchmaking/tickets/{ticket_id}"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "POST /matchmaking/tickets/{ticket_id}/heartbeat"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "JoinMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "GetMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "HeartbeatMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "CancelMatchmaking"
        },
        trace::Surface::Grpc {
            path: "proto/game.proto",
            service: "GameService",
            operation: "ReadyMatchmaking"
        },
        trace::Surface::OpenApi {
            package: "realtime-gateway",
            example: "matchmaking-openapi",
            operation: "POST /matchmaking/tickets/{ticket_id}/ready"
        }
    ],
});
