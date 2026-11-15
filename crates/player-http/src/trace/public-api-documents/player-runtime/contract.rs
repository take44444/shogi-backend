use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The player document route returns the same generated OpenAPI as player_http::openapi without reading storage.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "pub fn openapi() -> utoipa::openapi::OpenApi {"
    },
    gap: None,
});
