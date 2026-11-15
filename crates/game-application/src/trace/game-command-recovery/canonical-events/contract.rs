use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "StagedGameChange validates sequence bounds/continuity and action/result combinations, then freezes normal move, move-plus-checkmate or resignation event order.",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub fn new(game_id: GameId, receipt: Receipt) -> Result<Self, ApplicationError> {"
    },
    gap: None,
});
