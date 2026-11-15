use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The gateway exports annotated matchmaking paths and two cookie security schemes through its offline OpenAPI accessor.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: Some(
        "This test checks presence rather than exact equality of the gateway method/path set. The completeness catalog separately reconciles generated operations. Gateway has no runtime /openapi.json route."
    ),
});
