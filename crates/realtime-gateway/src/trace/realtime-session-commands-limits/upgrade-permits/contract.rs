use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Upgrade authenticates/access-checks, checks shutdown before/after upstream calls, acquires total and per-user permits and configures frame/message limits; disconnect Drop releases capacity.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn upgrade("
    },
    gap: Some(
        "No dedicated race assertion covers shutdown during an upstream handshake. Capacity tests do not exhaust every in-flight handshake state."
    ),
});
