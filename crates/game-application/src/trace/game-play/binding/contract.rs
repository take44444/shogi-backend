use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "アクターは受理済み記録の検索より先に参加者への所属を確認し、呼び出し元が選んだ先後ではなく、不変の Participants から操作する側を導出する。",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {"
    },
    gap: None,
});
