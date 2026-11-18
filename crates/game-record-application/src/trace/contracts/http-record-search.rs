use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "http-record-search",
    consumer: "player-http",
    provider: "game-record-application",
    interface: trace::Interface::RustApi,
    statement: "GameRecords::search は型付き Search を RecordRepository に渡し、その結果を返す。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    call: Some(trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn search("
    }),
    implementation: Some(trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn search(&self, search: &Search) -> Result<Page, Error> {"
    }),
    binding: None,
    requires: &[trace::Dependency {
        contract: "game-record-application/record-search-store",
        reason: "提供元はこの直接の依存先を利用して、上位の応答と状態の保証を満たす。"
    }],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
