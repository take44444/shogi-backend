use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "GameRecords::detail は型付き GameId をリポジトリに渡し、その結果を返す。",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    },
    gap: Some("任意の ID と結果が変更なしで転送されることを検証する独立したテストはない。"),
});
