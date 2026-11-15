use crate::trace_support as trace;
trace::contract!(trace::Contract {
    statement: "PostgreSQL commit_change compares expected version and atomically saves metadata, receipt, frozen supplied events and terminal dependent records; no partial success is exposed.",
    source: trace::Source {
        path: "crates/game-persistence/src/lib.rs",
        needle: "async fn commit_change(&self, change: &StagedGameChange) -> Result<(), Error> {"
    },
    gap: None,
});
