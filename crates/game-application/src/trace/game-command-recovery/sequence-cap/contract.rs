use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "next_sequence rejects advancing i64::MAX; stale expected sequence is checked before attempting an exhausted advance.",
    source: trace::Source {
        path: "crates/game-application/src/actor.rs",
        needle: "fn next_sequence(seq: u64) -> Result<u64, ApplicationError> {"
    },
    gap: Some(
        "Tests synthesize maximum state; they do not prove the rules engine supports billions of actual moves."
    ),
});
