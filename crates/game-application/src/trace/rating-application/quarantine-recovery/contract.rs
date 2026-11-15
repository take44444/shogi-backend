use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The durable fail/recovery operations enforce transient backoff and attempt budget while retaining reservations; recovery resets quarantine without repairing state.",
    source: trace::Source {
        path: "crates/game-persistence/src/rating.rs",
        needle: "    async fn retry_quarantined("
    },
    gap: Some(
        "Recovery tests assert outcomes, increasing generations, retained reservations and the need to repair corrupt state. They do not compare a complete before/after snapshot of frozen inputs and both ratings during recovery."
    ),
});
