use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "復元処理は標準初期局面から操作を再現し、受理記録のシーケンス、一意性、参加者の正当性、受理結果、最終メタデータを検証する。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn restore(record: GameRecord) -> Result<Self, ApplicationError> {"
    },
    gap: None,
});
