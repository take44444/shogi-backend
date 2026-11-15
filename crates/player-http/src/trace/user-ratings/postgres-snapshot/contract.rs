use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL 読み取り側は指定ユーザーの確定済み rating_states を読み、存在しないプールを Unrated とし、保存値を RatingState で検証する。",
    source: trace::Source {
        path: "crates/game-record-persistence/src/ratings.rs",
        needle: "impl RatingRepository for PostgresGameRecords {"
    },
    gap: Some(
        "読み取り側での破損値拒否、別ユーザーの値が混ざらないこと、未知ユーザーの両プールの初期値を個別に確認する assertion はない。"
    ),
});
