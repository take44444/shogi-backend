use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "Search validates interval, limit and ply bounds, and decodes a cursor tied to the filter fingerprint rather than page size.",
    source: trace::Source {
        path: "crates/game-record-application/src/lib.rs",
        needle: "pub fn new(filters: Filters, limit: u32, cursor: Option<&str>) -> Result<Self, Error> {"
    },
    gap: Some(
        "The mapped cases do not mutate every fingerprint field or exhaust cursor byte and UTF-8 boundaries."
    ),
});
