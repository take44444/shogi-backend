use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Player route-derived OpenAPI 3.1 has package identity, exact method/path set, query/error constraints and public response shapes.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn api_router() -> OpenApiRouter<App> {"
    },
    gap: None,
});
