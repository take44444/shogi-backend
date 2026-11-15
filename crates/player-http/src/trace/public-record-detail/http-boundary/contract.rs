use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The detail handler validates the path ID, uses the shared permit and maps repository failures to generic no-store HTTP responses.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "async fn detail(State(app): State<App>, path: Result<Path<String>, PathRejection>) -> Response {"
    },
    gap: Some(
        "The real-router fixture does not create a hidden record; its hidden behavior is supported by the repository test and shared error mapping. Shared admission and byte limits are mapped in public-record-search."
    ),
});
