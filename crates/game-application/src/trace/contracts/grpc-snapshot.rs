use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "grpc-snapshot",
    consumer: "game-grpc",
    provider: "game-application",
    interface: trace::Interface::RustApi,
    statement: "信頼済み呼出元に永続状態を反映した対局スナップショットを返す。閲覧者の認証はこの内部 API の保証に含めない。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot(&self, id: &GameId) -> Result<Snapshot, ApplicationError> {"
    },
    call: Some(trace::Source {
        path: "crates/game-grpc/src/service.rs",
        needle: "async fn get_snapshot("
    }),
    implementation: Some(trace::Source {
        path: "crates/game-application/src/lib.rs",
        needle: "pub async fn snapshot(&self, id: &GameId) -> Result<Snapshot, ApplicationError> {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-application/game-store",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
