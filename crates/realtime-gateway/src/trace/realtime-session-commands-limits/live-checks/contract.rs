use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "command_worker rechecks session and current game access before invoking upstream command using session.user.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn command_worker("
    },
    gap: Some(
        "No independent recheck-before-response race assertion or timed upstream mutation outcome test."
    ),
});
