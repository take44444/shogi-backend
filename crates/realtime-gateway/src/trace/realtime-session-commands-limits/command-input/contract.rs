use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The reader enforces frame/rate limits, strict command parsing, one in-flight command permit and a bounded queue. Parser rejects unknown fields and noncanonical decimal sequences/USI.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "async fn reader("
    },
    gap: Some(
        "No parser table enumerates malformed decimal strings, oversized numeric values or all unknown fields. No dedicated busy/in-flight permit or command queue/output-overflow assertion."
    ),
});
