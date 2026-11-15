use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Only an unexpired Waiting ticket with a larger sequence gets a fresh database-time expiry; all terminal states retain their stored state.",
    source: trace::Source {
        path: "crates/game-persistence/src/matchmaking.rs",
        needle: "async fn heartbeat("
    },
    gap: Some(
        "No selected assertion enumerates all older sequences, all non-Waiting states or exact wall-clock timing boundaries."
    ),
});
