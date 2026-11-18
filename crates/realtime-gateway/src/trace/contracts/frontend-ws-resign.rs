use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "frontend-ws-resign",
    consumer: "frontend",
    provider: "realtime-gateway",
    interface: trace::Interface::WebSocket,
    statement: "認証済みの参加者から resign コマンドを受け付け、厳密な入力・識別条件に従って受理・拒否・成否不明を返す。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "enum WireCommand {"
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/realtime-gateway/src/game.rs",
        needle: "enum WireCommand {"
    }),
    binding: None,
    requires: &[
        trace::Dependency {
            contract: "game-grpc/rpc-resign",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "user-http/gateway-session",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        }
    ],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
