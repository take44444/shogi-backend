use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "読み取り処理は一つの反復可能読み取りトランザクションを使い、連続した履歴全体の構造を検証してから Detail を返す。",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    },
    gap: Some(
        "破損の変更事例を選んで確認しており、すべての組み合わせは網羅しない。構造の検証では、意図的に将棋の着手の合法性は確認しない。"
    ),
});
