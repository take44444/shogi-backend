use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "run_connection subscribes and pumps before viewer snapshot, validates and queues snapshot first, initializes cursor at its sequence and drives queued notifications through that cursor.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn run_connection("
    },
    gap: Some(
        "No live broker disconnect/reconnect end-to-end assertion proves every subscription-loss path; reconnect does not replay a durable event log."
    ),
});
