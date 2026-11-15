use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The reader uses one repeatable-read transaction and validates a complete contiguous structural history before returning Detail.",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn detail(&self, id: &GameId) -> Result<Detail, Error> {"
    },
    gap: Some(
        "These are selected corruption mutations rather than an exhaustive matrix. Structural validation deliberately does not establish shogi move legality."
    ),
});
