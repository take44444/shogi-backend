use crate::trace_support::{self as trace, Contract};
trace::contract!(Contract {
    statement: "PostgresOutbox uses one nonreconnecting advisory-locked session for ordered reads and exact-row idempotent marking, rejects unsupported frozen encodings, and leaves no open transaction during transport wait.",
    source: trace::Source {
        path: "crates/game-persistence/src/outbox.rs",
        needle: "impl OutboxRepository for PostgresOutbox {"
    },
    gap: Some(
        "The delivery-boundary fixture observes database state and subsequent writes. It is not an all-components test stalled on a real broker."
    )
});
