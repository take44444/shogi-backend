use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "The PostgreSQL reader filters public completed metadata, applies all requested filters, and seeks strictly after the prior finish-time and binary-ID pair.",
    source: trace::Source {
        path: "crates/game-record-persistence/src/lib.rs",
        needle: "async fn search(&self, search: &Search) -> Result<Page, Error> {"
    },
    gap: None,
});
