use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The shared adapter takes an immediate semaphore permit and respond maps errors, applies no-store, and bounds serialized JSON to one MiB.",
    source: trace::Source {
        path: "crates/player-http/src/lib.rs",
        needle: "fn respond<T: Serialize>(result: Result<T, Error>) -> Response {"
    },
    gap: Some(
        "The size boundary uses strings rather than a maximum real page. No mixed search/detail/ratings exhaustion test is mapped."
    ),
});
