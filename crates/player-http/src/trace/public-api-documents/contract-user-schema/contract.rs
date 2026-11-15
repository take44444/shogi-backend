use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "User route-derived OpenAPI has service identity, the exact six operations, cookie security alternatives and request/response schemas.",
    source: trace::Source {
        path: "crates/user-http/src/lib.rs",
        needle: "fn api_router() -> OpenApiRouter<App> {"
    },
    gap: Some(
        "The runtime rejection sample does not cover every response family or all success/error headers. The schema checks do not establish provider availability."
    ),
});
