use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "rpc-get-snapshot",
    consumer: "trusted-service-client",
    provider: "game-grpc",
    interface: trace::Interface::Grpc,
    statement: "GetSnapshot は入力を検証し、型付きの要求を直接の依存先へ渡し、結果を安定した gRPC 応答またはエラーに変換する。呼出元の識別情報は信頼された申告値として扱う。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn get_snapshot("
    },
    call: None,
    implementation: Some(trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn get_snapshot("
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-application/grpc-snapshot",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
