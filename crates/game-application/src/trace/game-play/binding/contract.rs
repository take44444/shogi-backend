use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The actor checks participant membership before accepted-receipt lookup and derives command side from immutable Participants rather than a caller side choice.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {"
    },
    gap: None,
});
