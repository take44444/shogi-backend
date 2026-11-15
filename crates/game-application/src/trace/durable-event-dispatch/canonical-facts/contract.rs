use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "StagedGameChange validates sequence bounds and action/result consistency and freezes move-then-finish or resignation-only-finish facts.",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub fn new(game_id: GameId, receipt: Receipt) -> Result<Self, ApplicationError> {"
    },
    gap: None
});
