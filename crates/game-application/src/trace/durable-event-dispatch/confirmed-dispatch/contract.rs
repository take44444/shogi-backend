use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "OutboxPublisher publishes before marking. The Valkey adapter accepts only valid nonnegative PUBLISH integers and invalidates a cancelled or timed-out connection.",
    source: trace::Source {
        path: "crates/game-application/src/events.rs",
        needle: "pub async fn publish_next(&mut self) -> Result<bool, DispatchError> {"
    },
    gap: Some(
        "Transport tests use a controlled TCP RESP fixture and fake repository, not a deployed Valkey instance or subscriber."
    )
});
