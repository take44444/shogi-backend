use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Mutation refreshes state, applies replay before expected sequence checks, stages a cloned candidate and supplied events, and publishes in-memory candidate only after commit confirmation.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn apply(&mut self, pending: MutationRequest) -> Result<Acceptance, ApplicationError> {"
    },
    gap: None,
});
