use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Current and ticket operations expire stale state transactionally and query the authenticated owner; current returns only waiting/playing participation.",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn current(&self, user: &UserId) -> Result<Participation, MError> {"
    },
    gap: None,
});
