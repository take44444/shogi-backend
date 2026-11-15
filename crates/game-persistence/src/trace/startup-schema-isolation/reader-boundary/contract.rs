use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgresGameRecords::connect validates an existing schema without initialization, limits its pool to four, and sets read-only transactions plus two-second acquire, statement and lock timeouts.",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "    pub async fn connect(url: &str) -> Result<Self, Error> {"
    },
    gap: Some(
        "The lock_timeout setting is asserted, but a separate real lock-wait timeout scenario is not mapped."
    ),
});
