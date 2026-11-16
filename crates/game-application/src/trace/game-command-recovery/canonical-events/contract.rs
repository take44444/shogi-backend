use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "StagedGameChange はシーケンスの境界と連続性、操作と結果の組み合わせを検証し、通常着手、着手と詰み、投了のイベント順を固定する。",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub fn new(game_id: GameId, receipt: Receipt) -> Result<Self, ApplicationError> {"
    },
    gap: None,
});
