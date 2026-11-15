use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "browser_headers requires one exact Origin and Host, permits absent fetch-site or same-origin only and finds exactly one supported shogi.realtime.v1 offer.",
    source: trace::Source {
        path: "crates/realtime-gateway/src/lib.rs",
        needle: "fn browser_headers(headers: &HeaderMap, origin: &str) -> bool {"
    },
    gap: Some(
        "Not every Host, duplicate protocol and header normalization combination is explicitly asserted by this selected test."
    ),
});
