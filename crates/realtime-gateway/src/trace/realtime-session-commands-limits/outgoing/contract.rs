use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "writer rechecks session/access before every frame and bounds send/close waits; bootstrap rejects a snapshot exceeding configured bytes and uses bounded output/notification channels.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn writer("
    },
    gap: Some(
        "No slow-browser backlog, output write deadline, exhausted notification queue or snapshot upstream-timeout assertion. Snapshot limit is enforced in run_connection, which this evidence exercises."
    ),
});
