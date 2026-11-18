use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "grpc-command",
    consumer: "game-grpc",
    provider: "game-application",
    interface: trace::Interface::RustApi,
    statement: "固定参加者から先後を導出し、着手・投了を受理する。同一操作の再試行は元の受理結果を返し、確定前の状態を公開しない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn make_move("
    },
    call: Some(trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn make_move("
    }),
    implementation: Some(trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn make_move("
    }),
    binding: None,
    requires: &[
        trace::Dependency {
            contract: "shogi-domain/application-play",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "shogi-domain/application-resign",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        },
        trace::Dependency {
            contract: "game-application/game-store",
            reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
        }
    ],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
