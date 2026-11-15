use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The pump forwards subscription results through a bounded queue and stops on failure or queue exhaustion; run_connection treats exhausted/lost notifications as terminal.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn pump("
    },
    gap: Some(
        "No selected assertion directly fills the gateway pump queue. Slow-browser output backlog and writer timeout lack dedicated assertions."
    ),
});
