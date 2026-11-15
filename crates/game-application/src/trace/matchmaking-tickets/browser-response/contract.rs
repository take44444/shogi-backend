use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Ticket responses preserve server state and distinguish newly created, replayed, refused and already-matched cancellation outcomes; router middleware adds no-store.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "fn ticket_response("
    },
    gap: Some(
        "no-store is asserted for representative successes and Origin failures, not every response class; all DTO corruption and upstream error mappings lack dedicated assertions."
    ),
});
