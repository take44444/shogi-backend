use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Ordinary snapshot requests refresh durable version before serving actor state; storage failure cannot fall back to the cached position.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "async fn refresh(&mut self) -> Result<(), ApplicationError> {"
    },
    gap: None,
});
