use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "StagedGameChange はシーケンスの境界と操作・結果の整合性を検証し、着手に続く終局、または投了だけの終局のイベントを固定する。",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub fn new(game_id: GameId, receipt: Receipt) -> Result<Self, ApplicationError> {"
    },
    gap: None
});
