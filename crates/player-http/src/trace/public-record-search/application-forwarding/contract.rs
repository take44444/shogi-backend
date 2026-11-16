use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GameRecords::search は型付き Search を RecordRepository に渡し、その結果を返す。",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    gap: Some(
        "任意の Search とリポジトリの結果が変更なしで転送されることを検証する独立したテストはない。"
    ),
});
