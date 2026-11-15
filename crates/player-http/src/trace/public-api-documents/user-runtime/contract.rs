use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The user document route returns offline OpenAPI without storage or identity-provider calls and applies private response headers.",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
