use crate::trace_support as trace;
trace::contract!(trace::Contract {
    id: "postgres-detail",
    consumer: "game-record-persistence",
    provider: "postgres",
    interface: trace::Interface::External,
    statement: "読み取り専用 repeatable-read トランザクションの中では、メタデータとジャーナルを同じ DB スナップショットから取得できる。",
    assumptions: &["呼出元はインターフェースの型・認証・識別条件を満たす。"],
    limitations: &["契約の分解は動作の合成証明ではない。"],
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    },
    call: Some(trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    }),
    implementation: None,
    binding: None,
    requires: &[],
    gap: Some(
        "記載した境界の代表事例を追跡する。内部テストは直接の契約根拠に含めず、未検証の入力・障害・並行順序は残る。"
    )
});
