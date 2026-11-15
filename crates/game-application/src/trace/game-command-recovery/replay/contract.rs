use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Replay first checks participant membership then compares the complete MutationRequest under the game-scoped accepted ID and returns its original Acceptance.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn replay(&self, request: &MutationRequest) -> Result<Option<Acceptance>, ApplicationError> {"
    },
    gap: None,
});
