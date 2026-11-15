use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The generated matchmaking OpenAPI includes the five owner-scoped operations and cookie authentication alternatives.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/matchmaking.rs",
        needle: "pub(crate) fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
