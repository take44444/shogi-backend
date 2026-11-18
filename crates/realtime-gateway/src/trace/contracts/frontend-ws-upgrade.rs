use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "frontend-ws-upgrade",
    consumer: "frontend",
    provider: "realtime-gateway",
    interface: trace::Interface::WebSocket,
    statement: "認証済みの閲覧者に初期局面と順序付き更新を返し、許可された参加者のコマンドを処理する。接続・入力・出力の枠と期限を適用し、アクセス喪失・キュー溢れ・停止では接続を閉じる。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: ".route(\"/ws/games/{game_id}\", get(upgrade))"
    }),
    binding: None,
    requires: &[
        trace::Dependency {
            contract: "game-grpc/rpc-make-move",
            reason: "接続から受け付ける着手コマンドの受理結果を得る。"
        },
        trace::Dependency {
            contract: "game-grpc/rpc-resign",
            reason: "接続から受け付ける投了コマンドの受理結果を得る。"
        },
        trace::Dependency {
            contract: "user-http/gateway-session",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "game-grpc/rpc-get-snapshot-for-viewer",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "game-grpc/rpc-check-view-access",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "game-messaging/gateway-subscription",
            reason: "初期局面の取得と並行して通知を購読し、以後の表示更新と再同期判定へ渡す。"
        }
    ],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
